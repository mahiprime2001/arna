//! Arna shared core — signaling client + WebRTC P2P.
//!
//! - [`Signaling`] is the WebSocket client that connects peers through the
//!   backend and relays opaque payloads.
//! - [`p2p`] uses that signaling channel to negotiate a real WebRTC peer
//!   connection (SDP offer/answer + ICE) and open a data channel — the
//!   foundation the screen/video and file/chat channels are built on next.

// `webrtc::Error` is large, which makes our `Error` enum large; boxing it
// everywhere would hurt `?` ergonomics for little gain here.
#![allow(clippy::result_large_err)]

pub mod p2p;

// Re-export so downstream crates (agent/console) can name WebRTC types without a
// direct dependency, and stay on the exact version core uses.
pub use webrtc;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Messages this client sends to the signaling backend.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Register {
        role: String,
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// Ask to start a session with `to`. The backend verifies `ticket` (when SSO
    /// is enabled) and forwards an `incoming_request` to the agent.
    ConnectRequest {
        to: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ticket: Option<String>,
    },
    Signal {
        to: String,
        data: serde_json::Value,
    },
    Ping,
}

/// Messages the backend sends back.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Registered {
        id: String,
    },
    /// A console wants to connect (delivered to the agent after the backend has
    /// verified the SSO ticket). `name` is the verified admin identity to show.
    IncomingRequest {
        from: String,
        name: String,
    },
    /// The backend refused a `connect_request` (bad/expired ticket, agent
    /// offline). Delivered to the console that asked.
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("websocket error: {0}")]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("webrtc error: {0}")]
    Webrtc(#[from] webrtc::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A cheap, cloneable handle for sending signaling messages — usable from
/// WebRTC callbacks (e.g. to trickle ICE candidates back to the peer).
#[derive(Clone)]
pub struct SignalSender {
    out: mpsc::UnboundedSender<Message>,
}

impl SignalSender {
    fn send(&self, msg: &ClientMsg) {
        if let Ok(text) = serde_json::to_string(msg) {
            let _ = self.out.send(Message::Text(text));
        }
    }

    /// Send an opaque payload to another peer by id.
    pub fn signal(&self, to: &str, data: serde_json::Value) {
        self.send(&ClientMsg::Signal {
            to: to.to_string(),
            data,
        });
    }
}

/// A live signaling connection: send [`ClientMsg`]s, receive [`ServerMsg`]s.
pub struct Signaling {
    out: mpsc::UnboundedSender<Message>,
    inbox: mpsc::UnboundedReceiver<ServerMsg>,
}

impl Signaling {
    /// Connect to the signaling backend, e.g. `ws://127.0.0.1:8081/ws`.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let (ws, _resp) = tokio_tungstenite::connect_async(url).await?;
        let (mut sink, mut stream) = ws.split();

        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
        let (in_tx, in_rx) = mpsc::unbounded_channel::<ServerMsg>();

        // Writer: drain our outbound queue to the socket.
        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader: parse inbound text frames into ServerMsg (unknown frames ignored).
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(parsed) = serde_json::from_str::<ServerMsg>(&text) {
                        if in_tx.send(parsed).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            out: out_tx,
            inbox: in_rx,
        })
    }

    fn send(&self, msg: &ClientMsg) {
        if let Ok(text) = serde_json::to_string(msg) {
            let _ = self.out.send(Message::Text(text));
        }
    }

    /// Register this peer with the backend so others can reach it by `id`. Agents
    /// pass a `token` proving they own the id (required when the backend has auth
    /// enabled); consoles pass `None`.
    pub fn register(&self, role: &str, id: &str, token: Option<&str>) {
        self.send(&ClientMsg::Register {
            role: role.to_string(),
            id: id.to_string(),
            token: token.map(|t| t.to_string()),
        });
    }

    /// Send an opaque signaling payload to another peer by id.
    pub fn signal(&self, to: &str, data: serde_json::Value) {
        self.send(&ClientMsg::Signal {
            to: to.to_string(),
            data,
        });
    }

    /// Ask the backend to broker a session with agent `to`, presenting an
    /// optional SSO `ticket` for verification.
    pub fn connect_request(&self, to: &str, ticket: Option<String>) {
        self.send(&ClientMsg::ConnectRequest {
            to: to.to_string(),
            ticket,
        });
    }

    /// Liveness ping (backend replies with `Pong`).
    pub fn ping(&self) {
        self.send(&ClientMsg::Ping);
    }

    /// A cloneable sender handle for use inside callbacks.
    pub fn sender(&self) -> SignalSender {
        SignalSender {
            out: self.out.clone(),
        }
    }

    /// Await the next message from the backend (`None` once the socket closes).
    pub async fn recv(&mut self) -> Option<ServerMsg> {
        self.inbox.recv().await
    }
}
