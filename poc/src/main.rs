//! Arna signaling proof-of-concept (Phase 1a).
//!
//! Connects to the backend, registers, and (optionally) sends a hello to another
//! peer — proving the end-to-end signaling round-trip. Run two of these against
//! a running backend:
//!
//!   # terminal 1 (backend):   cargo run --manifest-path backend/Cargo.toml
//!   # terminal 2 (listener):  cargo run -p arna-poc -- ws://127.0.0.1:8081/ws agent  store-1
//!   # terminal 3 (caller):    cargo run -p arna-poc -- ws://127.0.0.1:8081/ws console me store-1
//!
//! The listener should print a line:  RECV signal from me: {"hello": ...}

use arna_core::{ServerMsg, Signaling};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "ws://127.0.0.1:8081/ws".to_string());
    let role = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "console".to_string());
    let id = args.get(3).cloned().unwrap_or_else(|| "peer".to_string());
    let peer = args.get(4).cloned();

    let mut sig = Signaling::connect(&url)
        .await
        .expect("failed to connect to signaling backend");
    sig.register(&role, &id);
    println!("[{id}] connected and registered as {role}");

    // If a peer id was given, greet it (after a beat so it can register first).
    if let Some(peer_id) = peer.clone() {
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        sig.signal(
            &peer_id,
            serde_json::json!({ "hello": format!("hi from {id}") }),
        );
        println!("[{id}] sent hello to {peer_id}");
    }

    // Print everything the backend sends us.
    while let Some(msg) = sig.recv().await {
        match msg {
            ServerMsg::Registered { id: rid } => {
                println!("[{id}] server confirmed registration: {rid}")
            }
            ServerMsg::Signal { from, data } => println!("RECV signal from {from}: {data}"),
            ServerMsg::PeerOffline { to } => println!("[{id}] peer offline: {to}"),
            ServerMsg::Error { message } => println!("[{id}] error: {message}"),
            ServerMsg::Pong => println!("[{id}] pong"),
        }
    }
    println!("[{id}] connection closed");
}
