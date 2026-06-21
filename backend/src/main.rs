//! Arna signaling backend (Phase 0).
//!
//! A small WebSocket hub that lets a Console and an Agent find each other and
//! exchange WebRTC handshake messages (SDP offer/answer + ICE candidates). It
//! holds NO media — peers connect directly (or via coturn) once introduced.
//!
//! Protocol (JSON, tagged by `type`):
//!   client -> server:
//!     { "type": "register", "role": "agent|console", "id": "<peer-id>" }
//!     { "type": "signal",   "to": "<peer-id>", "data": { ... } }
//!     { "type": "ping" }
//!   server -> client:
//!     { "type": "registered",   "id": "<peer-id>" }
//!     { "type": "signal",       "from": "<peer-id>", "data": { ... } }
//!     { "type": "peer_offline", "to": "<peer-id>" }
//!     { "type": "error",        "message": "..." }
//!     { "type": "pong" }
//!
//! Identity/authorization (SSO ticket verification) is added in Phase 3; for now
//! registration is open so we can prove the transport end-to-end.

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};

type Tx = mpsc::UnboundedSender<Message>;

#[derive(Default)]
struct Hub {
    /// peer-id -> outbound channel for that peer's socket.
    peers: DashMap<String, Tx>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Incoming {
    Register { role: String, id: String },
    Signal { to: String, data: serde_json::Value },
    Ping,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Outgoing {
    Registered {
        id: String,
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

    let hub = Arc::new(Hub::default());

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
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
            Incoming::Register { role, id } => {
                hub.peers.insert(id.clone(), tx.clone());
                my_id = Some(id.clone());
                info!(role = %role, id = %id, "peer registered");
                let _ = tx.send(encode(&Outgoing::Registered { id }));
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
                        let _ = peer.send(encode(&Outgoing::Signal { from, data }));
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
            .remove_if(&id, |_, existing| existing.same_channel(&tx));
        warn!(id = %id, "peer disconnected");
    }
    send_task.abort();
}

fn encode(out: &Outgoing) -> Message {
    Message::Text(serde_json::to_string(out).unwrap_or_else(|_| "{}".into()))
}
