//! Arna WebRTC P2P proof-of-concept (Phase 1b).
//!
//! Two peers connect through the backend, negotiate a WebRTC peer connection
//! over signaling, open a data channel, and greet each other directly P2P.
//!
//!   # terminal 1 (backend):  cargo run --manifest-path backend/Cargo.toml
//!   # terminal 2 (answerer): cargo run -p arna-poc -- ws://127.0.0.1:8081/ws agent   store-1
//!   # terminal 3 (offerer):  cargo run -p arna-poc -- ws://127.0.0.1:8081/ws console me store-1
//!
//! Both should print:  [..] P2P data channel OPEN   and   P2P RECV: hello over P2P from ..

use arna_core::{p2p, Signaling};

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

    let signaling = Signaling::connect(&url)
        .await
        .expect("failed to connect to signaling backend");
    signaling.register(&role, &id, None);
    println!("[{id}] connected and registered as {role}");

    // A peer id means we initiate (offerer); otherwise we wait for an offer.
    let offerer = peer.is_some();
    if let Err(e) = p2p::run(signaling, peer, offerer, id.clone()).await {
        eprintln!("[{id}] p2p error: {e}");
    }
    println!("[{id}] session ended");
}
