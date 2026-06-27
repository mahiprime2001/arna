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

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};

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
}

/// HS256 claims. For a **console ticket**: `sub` = admin name, `agent` pins it to
/// one target. For an **agent token**: `sub` = the agent id, `role` = "agent".
/// `exp` is a Unix timestamp enforced by `jsonwebtoken`.
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    exp: usize,
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
    // An agent token must not double as a console connect ticket.
    if data.claims.role.as_deref() == Some("agent") {
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

    let hub = Arc::new(Hub {
        peers: DashMap::new(),
        sso_secret,
        dev_tickets,
    });

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
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

    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

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
