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
    pub password_hash: String,
}

/// A registered device (an agent), owned by a user.
pub struct Device {
    pub id: String,
    pub name: String,
    pub owner: i64,
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
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("db mutex")
    }

    /// Create a user; returns the new id. `StoreError::Duplicate` if the email
    /// is taken.
    pub fn create_user(&self, email: &str, password_hash: &str) -> Result<i64, StoreError> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO users (email, password_hash, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![email, password_hash, now()],
        )
        .map_err(map_err)?;
        Ok(conn.last_insert_rowid())
    }

    /// Look up a user by email (case-insensitive).
    pub fn user_by_email(&self, email: &str) -> Result<Option<User>, StoreError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, password_hash FROM users WHERE email = ?1",
            rusqlite::params![email],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    password_hash: row.get(1)?,
                })
            },
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

    /// Fetch a device by id.
    pub fn device(&self, id: &str) -> Result<Option<Device>, StoreError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, name, owner FROM devices WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(Device {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    owner: row.get(2)?,
                })
            },
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
            .prepare("SELECT id, name, owner FROM devices WHERE owner = ?1 ORDER BY name")
            .map_err(map_err)?;
        let rows = stmt
            .query_map(rusqlite::params![owner], |row| {
                Ok(Device {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    owner: row.get(2)?,
                })
            })
            .map_err(map_err)?;
        let mut out = Vec::new();
        for d in rows {
            out.push(d.map_err(map_err)?);
        }
        Ok(out)
    }
}
