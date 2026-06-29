//! SQLite-backed accounts + device registry.
//!
//! A tiny data layer so the backend can own users and devices (Phase: accounts).
//! Operations are synchronous (rusqlite) behind a `Mutex`; callers keep them off
//! the hot path. Password *hashing* (the slow part) happens in the handlers, not
//! here.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode};

/// Shared SQLite handle.
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

/// What a store operation can go wrong with.
#[derive(Debug)]
pub enum StoreError {
    /// A uniqueness constraint failed (e.g. email already registered).
    Duplicate,
    /// Any other database error.
    Db(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Duplicate => write!(f, "already exists"),
            StoreError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

fn map_err(e: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(err, _) = &e {
        if err.code == ErrorCode::ConstraintViolation {
            return StoreError::Duplicate;
        }
    }
    StoreError::Db(e.to_string())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A user account.
pub struct User {
    pub id: i64,
    /// Short, shareable account ID (9 digits) shown in the profile/QR.
    pub short_id: String,
    pub email: String,
    pub password_hash: String,
}

/// A registered device (an agent), owned by a user.
pub struct Device {
    pub id: String,
    pub name: String,
    /// Owning user id. Retained for ownership-scoped queries/future use.
    #[allow(dead_code)]
    pub owner: i64,
    /// Whether an unattended-access password is set on this device.
    pub has_password: bool,
}

/// Generate a random 9-digit short account ID (string, may have leading zeros).
fn gen_short_id() -> String {
    use rand::Rng;
    format!("{:09}", rand::thread_rng().gen_range(0..1_000_000_000u64))
}

impl Store {
    /// Open (creating if needed) the SQLite database and run migrations.
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(map_err)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS users (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 email         TEXT NOT NULL UNIQUE COLLATE NOCASE,
                 password_hash TEXT NOT NULL,
                 created_at    INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS devices (
                 id         TEXT PRIMARY KEY,
                 name       TEXT NOT NULL,
                 owner      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                 created_at INTEGER NOT NULL
             );",
        )
        .map_err(map_err)?;
        // Migrations for existing DBs (ignore "duplicate column" on re-run):
        //  - users.short_id: a short, shareable account ID (AnyDesk-style).
        //  - devices.access_hash: optional unattended-access password (Argon2).
        let _ = conn.execute("ALTER TABLE users ADD COLUMN short_id TEXT", []);
        let _ = conn.execute("ALTER TABLE devices ADD COLUMN access_hash TEXT", []);
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_short_id ON users(short_id);",
        )
        .map_err(map_err)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("db mutex")
    }

    /// Create a user; returns the new id + its short ID. `StoreError::Duplicate`
    /// if the email is taken. The email-uniqueness check (INSERT) is separate
    /// from short-ID assignment (UPDATE), so a short-ID collision can be retried
    /// without confusing it with a duplicate email.
    pub fn create_user(&self, email: &str, password_hash: &str) -> Result<(i64, String), StoreError> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO users (email, password_hash, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![email, password_hash, now()],
        )
        .map_err(map_err)?;
        let id = conn.last_insert_rowid();
        for _ in 0..30 {
            let sid = gen_short_id();
            match conn.execute(
                "UPDATE users SET short_id = ?1 WHERE id = ?2",
                rusqlite::params![sid, id],
            ) {
                Ok(_) => return Ok((id, sid)),
                Err(e) => match map_err(e) {
                    StoreError::Duplicate => continue, // short-ID collision, retry
                    other => return Err(other),
                },
            }
        }
        Err(StoreError::Db("could not allocate a unique short ID".into()))
    }

    fn row_to_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
        Ok(User {
            id: row.get(0)?,
            short_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            email: row.get(2)?,
            password_hash: row.get(3)?,
        })
    }

    /// Look up a user by email (case-insensitive).
    pub fn user_by_email(&self, email: &str) -> Result<Option<User>, StoreError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, short_id, email, password_hash FROM users WHERE email = ?1",
            rusqlite::params![email],
            Self::row_to_user,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(map_err(other)),
        })
    }

    /// Look up a user by their numeric id.
    pub fn user_by_id(&self, id: i64) -> Result<Option<User>, StoreError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, short_id, email, password_hash FROM users WHERE id = ?1",
            rusqlite::params![id],
            Self::row_to_user,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(map_err(other)),
        })
    }

    /// Register (or rename) a device owned by `owner`. Idempotent on id.
    pub fn upsert_device(&self, id: &str, name: &str, owner: i64) -> Result<(), StoreError> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO devices (id, name, owner, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, owner = excluded.owner",
            rusqlite::params![id, name, owner, now()],
        )
        .map_err(map_err)?;
        Ok(())
    }

    fn row_to_device(row: &rusqlite::Row<'_>) -> rusqlite::Result<Device> {
        Ok(Device {
            id: row.get(0)?,
            name: row.get(1)?,
            owner: row.get(2)?,
            has_password: row.get::<_, Option<String>>(3)?.is_some(),
        })
    }

    /// Fetch a device by id.
    pub fn device(&self, id: &str) -> Result<Option<Device>, StoreError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, name, owner, access_hash FROM devices WHERE id = ?1",
            rusqlite::params![id],
            Self::row_to_device,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(map_err(other)),
        })
    }

    /// List the devices a user owns.
    pub fn devices_of(&self, owner: i64) -> Result<Vec<Device>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, owner, access_hash FROM devices WHERE owner = ?1 ORDER BY name",
            )
            .map_err(map_err)?;
        let rows = stmt
            .query_map(rusqlite::params![owner], Self::row_to_device)
            .map_err(map_err)?;
        let mut out = Vec::new();
        for d in rows {
            out.push(d.map_err(map_err)?);
        }
        Ok(out)
    }

    /// Set (or clear, with `None`) a device's unattended-access password hash —
    /// only if `owner` owns it. Returns whether a row was updated.
    pub fn set_device_access(
        &self,
        id: &str,
        owner: i64,
        hash: Option<&str>,
    ) -> Result<bool, StoreError> {
        let conn = self.lock();
        let n = conn
            .execute(
                "UPDATE devices SET access_hash = ?1 WHERE id = ?2 AND owner = ?3",
                rusqlite::params![hash, id, owner],
            )
            .map_err(map_err)?;
        Ok(n > 0)
    }

    /// The device's access-password hash, if one is set.
    pub fn device_access_hash(&self, id: &str) -> Result<Option<String>, StoreError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT access_hash FROM devices WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get::<_, Option<String>>(0),
        )
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(map_err(other)),
        })
    }
}
