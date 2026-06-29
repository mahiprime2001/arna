//! Arna signaling backend (Phase 0).
//!
//! A small WebSocket hub that lets a Console and an Agent find each other and
//! exchange WebRTC handshake messages (SDP offer/answer + ICE candidates). It
//! holds NO media — peers connect directly (or via coturn) once introduced.
//!
//! Protocol (JSON, tagged by `type`):
//!   client -> server:
//!     { "type": "register", "role": "agent|console", "id": "<peer-id>", "token": "<jwt?>" }
//!     { "type": "connect_request", "to": "<agent-id>", "ticket": "<jwt?>" }
//!     { "type": "signal",   "to": "<peer-id>", "data": { ... } }
//!     { "type": "ping" }
//!   server -> client:
//!     { "type": "registered",       "id": "<peer-id>" }
//!     { "type": "incoming_request", "from": "<console-id>", "name": "<admin>" }
//!     { "type": "request_denied",   "to": "<agent-id>", "reason": "..." }
//!     { "type": "signal",           "from": "<peer-id>", "data": { ... } }
//!     { "type": "peer_offline",     "to": "<peer-id>" }
//!     { "type": "error",            "message": "..." }
//!     { "type": "pong" }
//!
//! Identity/authorization: a `connect_request` carries an optional SSO ticket
//! (HS256 JWT minted by the billing app). When `ARNA_SSO_SECRET` is set the
//! backend verifies it before forwarding `incoming_request` to the agent; when
//! unset, auth is **open** (dev mode) and the console is shown as "Console (id)".

mod store;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use store::{Store, StoreError};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Hash a password with Argon2 (slow by design — call off the async runtime).
fn hash_password(pw: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verify a password against a stored Argon2 hash.
fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .and_then(|ph| Argon2::default().verify_password(pw.as_bytes(), &ph))
        .is_ok()
}

type Tx = mpsc::UnboundedSender<Message>;

/// A connected peer: its outbound channel and whether it registered as an agent
/// (only agents are valid `connect_request` targets).
struct Peer {
    tx: Tx,
    is_agent: bool,
}

struct Hub {
    /// peer-id -> connected peer.
    peers: DashMap<String, Peer>,
    /// Shared HS256 secret for SSO tickets + agent tokens; `None` = open mode.
    sso_secret: Option<String>,
    /// Whether the `/dev/ticket` minting helper is enabled.
    dev_tickets: bool,
    /// Accounts + device registry.
    db: Store,
}

/// HS256 claims. For a **console ticket**: `sub` = admin name, `agent` pins it to
/// one target. For an **agent token**: `sub` = the agent id, `role` = "agent". For
/// a **session token** (from login): `sub` = user id, `typ` = "session".
/// `exp` is a Unix timestamp enforced by `jsonwebtoken`.
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typ: Option<String>,
    exp: usize,
}

/// Issue a session token for a logged-in user (valid 7 days).
fn issue_session(secret: &str, user_id: i64) -> Result<String, String> {
    let claims = Claims {
        sub: user_id.to_string(),
        agent: None,
        role: None,
        typ: Some("session".into()),
        exp: unix_in(7 * 24 * 3600),
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

/// Verify an agent's registration token: a valid JWT whose `sub` is the agent id
/// and `role` is "agent". In open mode (no secret) registration is unauthenticated.
fn verify_agent_token(
    secret: &Option<String>,
    token: Option<&str>,
    id: &str,
) -> Result<(), String> {
    let Some(secret) = secret else {
        return Ok(());
    };
    let token = token.ok_or("agent token required")?;
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_required_spec_claims(&["exp"]);
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map_err(|e| format!("invalid agent token: {e}"))?;
    if data.claims.sub != id {
        return Err("token is for a different agent id".into());
    }
    if data.claims.role.as_deref() != Some("agent") {
        return Err("not an agent token".into());
    }
    Ok(())
}

/// Verify a `connect_request` ticket, returning the display name to show the
/// agent. In open mode (no secret) any request is admitted as "Console (id)".
fn verify_ticket(
    secret: &Option<String>,
    ticket: Option<&str>,
    agent: &str,
    console_id: &str,
) -> Result<String, String> {
    let Some(secret) = secret else {
        return Ok(format!("Console ({console_id})"));
    };
    let ticket = ticket.ok_or("authentication required")?;
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_required_spec_claims(&["exp"]);
    let data = decode::<Claims>(ticket, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map_err(|e| format!("invalid ticket: {e}"))?;
    // An agent or session token must not double as a console connect ticket.
    if data.claims.role.as_deref() == Some("agent") || data.claims.typ.as_deref() == Some("session") {
        return Err("not a console ticket".into());
    }
    if let Some(a) = &data.claims.agent {
        if a != agent {
            return Err("ticket not valid for this agent".into());
        }
    }
    Ok(data.claims.sub)
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Incoming {
    Register {
        role: String,
        id: String,
        #[serde(default)]
        token: Option<String>,
    },
    ConnectRequest {
        to: String,
        #[serde(default)]
        ticket: Option<String>,
    },
    Signal {
        to: String,
        data: serde_json::Value,
    },
    Ping,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Outgoing {
    Registered {
        id: String,
    },
    IncomingRequest {
        from: String,
        name: String,
    },
    RequestDenied {
        to: String,
        reason: String,
    },
    Signal {
        from: String,
        data: serde_json::Value,
    },
    PeerOffline {
        to: String,
    },
    Error {
        message: String,
    },
    Pong,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let sso_secret = std::env::var("ARNA_SSO_SECRET").ok().filter(|s| !s.is_empty());
    let dev_tickets = std::env::var("ARNA_DEV_TICKETS").as_deref() == Ok("1");
    if sso_secret.is_none() {
        warn!("ARNA_SSO_SECRET not set — running in OPEN mode (no SSO verification)");
    } else {
        info!("SSO ticket verification enabled");
    }

    let db_path = std::env::var("ARNA_DB").unwrap_or_else(|_| "arna.db".to_string());
    let db = Store::open(&db_path).expect("failed to open database");
    info!("accounts database at {db_path}");

    let hub = Arc::new(Hub {
        peers: DashMap::new(),
        sso_secret,
        dev_tickets,
        db,
    });

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/dev/ticket", get(dev_ticket))
        .route("/ws", get(ws_handler))
        .with_state(hub);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("0.0.0.0:{port}");
    info!("Arna signaling backend listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

async fn ws_handler(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, hub))
}

async fn handle_socket(socket: WebSocket, hub: Arc<Hub>) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Forward this peer's outbound queue to its socket.
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut my_id: Option<String> = None;

    // Per-connection rate limits: cap overall message rate (anti-flood, closes the
    // socket) and connect_requests (anti popup-spam, just refuses the extra ones).
    const MSG_PER_SEC: u32 = 60;
    const CREQ_PER_WINDOW: u32 = 10;
    let creq_window = Duration::from_secs(5);
    let (mut msg_win, mut msg_n) = (Instant::now(), 0u32);
    let (mut creq_win, mut creq_n) = (Instant::now(), 0u32);

    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        // Anti-flood: too many messages per second -> drop this abusive socket.
        let now = Instant::now();
        if now.duration_since(msg_win) >= Duration::from_secs(1) {
            msg_win = now;
            msg_n = 0;
        }
        msg_n += 1;
        if msg_n > MSG_PER_SEC {
            warn!(id = ?my_id, "rate limit: message flood — closing connection");
            break;
        }

        let incoming: Incoming = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.send(encode(&Outgoing::Error {
                    message: format!("bad message: {e}"),
                }));
                continue;
            }
        };

        match incoming {
            Incoming::Register { role, id, token } => {
                let is_agent = role == "agent";
                // Agents must prove they own their id (when auth is enabled), so
                // nobody can impersonate a device. Consoles use throwaway ids that
                // are never connection targets, so they don't need a token here.
                if is_agent {
                    if let Err(reason) = verify_agent_token(&hub.sso_secret, token.as_deref(), &id) {
                        warn!(id = %id, %reason, "agent registration denied");
                        let _ = tx.send(encode(&Outgoing::Error {
                            message: format!("registration denied: {reason}"),
                        }));
                        continue;
                    }
                } else if hub.peers.get(&id).is_some_and(|p| p.is_agent) {
                    // A console can't take over an id a real agent already holds.
                    let _ = tx.send(encode(&Outgoing::Error {
                        message: "id in use by a device".into(),
                    }));
                    continue;
                }
                hub.peers.insert(
                    id.clone(),
                    Peer {
                        tx: tx.clone(),
                        is_agent,
                    },
                );
                my_id = Some(id.clone());
                info!(role = %role, id = %id, "peer registered");
                let _ = tx.send(encode(&Outgoing::Registered { id }));
            }
            Incoming::ConnectRequest { to, ticket } => {
                let from = match &my_id {
                    Some(id) => id.clone(),
                    None => {
                        let _ = tx.send(encode(&Outgoing::Error {
                            message: "register before connecting".into(),
                        }));
                        continue;
                    }
                };
                // Throttle connect_requests so one console can't spam an agent's
                // consent popup (or brute-force agent ids).
                let now = Instant::now();
                if now.duration_since(creq_win) >= creq_window {
                    creq_win = now;
                    creq_n = 0;
                }
                creq_n += 1;
                if creq_n > CREQ_PER_WINDOW {
                    let _ = tx.send(encode(&Outgoing::RequestDenied {
                        to,
                        reason: "too many requests — slow down".into(),
                    }));
                    continue;
                }
                match verify_ticket(&hub.sso_secret, ticket.as_deref(), &to, &from) {
                    Ok(name) => match hub.peers.get(&to) {
                        // Only route to a peer that actually registered as an agent.
                        Some(agent) if agent.is_agent => {
                            info!(console = %from, agent = %to, %name, "connect_request -> incoming_request");
                            let _ = agent.tx.send(encode(&Outgoing::IncomingRequest { from, name }));
                        }
                        Some(_) => {
                            let _ = tx.send(encode(&Outgoing::RequestDenied {
                                to,
                                reason: "not an agent".into(),
                            }));
                        }
                        None => {
                            let _ = tx.send(encode(&Outgoing::RequestDenied {
                                to,
                                reason: "agent offline".into(),
                            }));
                        }
                    },
                    Err(reason) => {
                        warn!(console = %from, agent = %to, %reason, "connect_request denied");
                        let _ = tx.send(encode(&Outgoing::RequestDenied { to, reason }));
                    }
                }
            }
            Incoming::Signal { to, data } => {
                let from = match &my_id {
                    Some(id) => id.clone(),
                    None => {
                        let _ = tx.send(encode(&Outgoing::Error {
                            message: "register before signaling".into(),
                        }));
                        continue;
                    }
                };
                match hub.peers.get(&to) {
                    Some(peer) => {
                        let _ = peer.tx.send(encode(&Outgoing::Signal { from, data }));
                    }
                    None => {
                        let _ = tx.send(encode(&Outgoing::PeerOffline { to }));
                    }
                }
            }
            Incoming::Ping => {
                let _ = tx.send(encode(&Outgoing::Pong));
            }
        }
    }

    // Disconnect cleanup.
    if let Some(id) = my_id {
        // Only remove if this socket still owns the slot (avoid evicting a
        // reconnect that re-registered the same id).
        hub.peers
            .remove_if(&id, |_, existing| existing.tx.same_channel(&tx));
        warn!(id = %id, "peer disconnected");
    }
    send_task.abort();
}

fn encode(out: &Outgoing) -> Message {
    Message::Text(serde_json::to_string(out).unwrap_or_else(|_| "{}".into()))
}

#[derive(Deserialize)]
struct AuthReq {
    email: String,
    password: String,
}

/// Create an account; returns a 7-day session token.
async fn signup(State(hub): State<Arc<Hub>>, Json(req): Json<AuthReq>) -> impl IntoResponse {
    let Some(secret) = hub.sso_secret.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "accounts disabled (set ARNA_SSO_SECRET)").into_response();
    };
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return (StatusCode::BAD_REQUEST, "a valid email is required").into_response();
    }
    if req.password.len() < 8 {
        return (StatusCode::BAD_REQUEST, "password must be at least 8 characters").into_response();
    }
    let pw = req.password.clone();
    let hash = match tokio::task::spawn_blocking(move || hash_password(&pw)).await {
        Ok(Ok(h)) => h,
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, "could not hash password").into_response(),
    };
    match hub.db.create_user(&email, &hash) {
        Ok(uid) => match issue_session(&secret, uid) {
            Ok(token) => Json(serde_json::json!({ "token": token, "user_id": uid })).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "token error").into_response(),
        },
        Err(StoreError::Duplicate) => (StatusCode::CONFLICT, "email already registered").into_response(),
        Err(e) => {
            warn!(%e, "signup failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "signup failed").into_response()
        }
    }
}

/// Log in; returns a 7-day session token. Same vague error for unknown email and
/// wrong password (don't reveal which emails exist).
async fn login(State(hub): State<Arc<Hub>>, Json(req): Json<AuthReq>) -> impl IntoResponse {
    let Some(secret) = hub.sso_secret.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "accounts disabled").into_response();
    };
    let email = req.email.trim().to_lowercase();
    let user = match hub.db.user_by_email(&email) {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "invalid email or password").into_response(),
        Err(e) => {
            warn!(%e, "login lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "login failed").into_response();
        }
    };
    let pw = req.password.clone();
    let hash = user.password_hash.clone();
    let ok = tokio::task::spawn_blocking(move || verify_password(&pw, &hash))
        .await
        .unwrap_or(false);
    if !ok {
        return (StatusCode::UNAUTHORIZED, "invalid email or password").into_response();
    }
    match issue_session(&secret, user.id) {
        Ok(token) => Json(serde_json::json!({ "token": token, "user_id": user.id })).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "token error").into_response(),
    }
}

#[derive(Deserialize)]
struct TicketQuery {
    /// "agent" to mint an agent registration token; otherwise a console ticket.
    #[serde(default)]
    role: Option<String>,
    /// Agent id (for an agent token).
    #[serde(default)]
    id: Option<String>,
    /// Target agent (for a console ticket).
    #[serde(default)]
    agent: Option<String>,
    /// Console display name (for a console ticket).
    #[serde(default)]
    name: Option<String>,
}

fn unix_in(secs: usize) -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as usize + secs)
        .unwrap_or(0)
}

/// Dev helper: mint tokens so apps can be tested with auth on, without a real
/// identity service. Enabled only when `ARNA_SSO_SECRET` is set **and**
/// `ARNA_DEV_TICKETS=1`; never enable in production.
///
/// - `?role=agent&id=<agent-id>` → a long-lived **agent registration token**.
/// - `?agent=<agent-id>&name=<n>` → a short-lived **console connect ticket**.
async fn dev_ticket(
    State(hub): State<Arc<Hub>>,
    Query(q): Query<TicketQuery>,
) -> impl IntoResponse {
    let Some(secret) = hub.sso_secret.clone() else {
        return (StatusCode::NOT_FOUND, "SSO disabled").into_response();
    };
    if !hub.dev_tickets {
        return (StatusCode::FORBIDDEN, "dev tickets disabled").into_response();
    }

    let claims = if q.role.as_deref() == Some("agent") {
        let Some(id) = q.id.clone() else {
            return (StatusCode::BAD_REQUEST, "agent token needs id").into_response();
        };
        Claims {
            sub: id,
            agent: None,
            role: Some("agent".into()),
            typ: None,
            exp: unix_in(365 * 24 * 3600), // long-lived provisioning token
        }
    } else {
        let Some(agent) = q.agent.clone() else {
            return (StatusCode::BAD_REQUEST, "console ticket needs agent").into_response();
        };
        Claims {
            sub: q.name.unwrap_or_else(|| "Dev Admin".into()),
            agent: Some(agent),
            role: None,
            typ: None,
            exp: unix_in(300),
        }
    };

    match jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ) {
        Ok(token) => Json(serde_json::json!({ "ticket": token, "token": token })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
    }
}
