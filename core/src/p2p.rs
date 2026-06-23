//! WebRTC peer-to-peer over the signaling channel (Phase 1b).
//!
//! Two peers negotiate a real `RTCPeerConnection` — SDP offer/answer and ICE
//! candidates travel as opaque `signal` payloads through the backend — then open
//! a data channel and exchange messages directly (P2P, or via TURN later). This
//! is the transport the screen/video, input, and file/chat channels ride on.

use std::sync::Arc;

use serde_json::json;
use tokio::sync::Mutex;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::{Error, ServerMsg, Signaling};

/// Wire up logging + a greeting on a data channel (both peers do this).
fn setup_data_channel(dc: Arc<RTCDataChannel>, tag: String) {
    let dc_open = dc.clone();
    let tag_open = tag.clone();
    dc.on_open(Box::new(move || {
        let dc_open = dc_open.clone();
        let tag_open = tag_open.clone();
        Box::pin(async move {
            println!("[{tag_open}] P2P data channel OPEN");
            let _ = dc_open
                .send_text(format!("hello over P2P from {tag_open}"))
                .await;
        })
    }));

    dc.on_message(Box::new(move |msg: DataChannelMessage| {
        let text = String::from_utf8_lossy(&msg.data).to_string();
        println!("P2P RECV: {text}");
        Box::pin(async {})
    }));
}

/// Run a P2P session over `signaling`.
///
/// - `offerer = true` creates the data channel + offer and sends it to `target`.
/// - `offerer = false` waits for an incoming offer and answers it.
///
/// Runs until the signaling socket closes.
pub async fn run(
    mut signaling: Signaling,
    target: Option<String>,
    offerer: bool,
    tag: String,
) -> Result<(), Error> {
    // Build the WebRTC API with default codecs + interceptors.
    let mut media = MediaEngine::default();
    media.register_default_codecs()?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media)?;
    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let pc = Arc::new(api.new_peer_connection(config).await?);

    let sender = signaling.sender();
    // The peer we trickle ICE to. Known up front for the offerer; learned from
    // the incoming offer for the answerer (set before its ICE gathering starts).
    let peer_cell: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(target.clone()));

    // Trickle local ICE candidates to the peer.
    {
        let sender = sender.clone();
        let peer_cell = peer_cell.clone();
        pc.on_ice_candidate(Box::new(move |candidate| {
            let sender = sender.clone();
            let peer_cell = peer_cell.clone();
            Box::pin(async move {
                if let Some(c) = candidate {
                    if let Ok(init) = c.to_json() {
                        if let Some(peer) = peer_cell.lock().await.clone() {
                            sender.signal(&peer, json!({ "kind": "ice", "candidate": init }));
                        }
                    }
                }
            })
        }));
    }

    // Log connection state transitions.
    {
        let tag = tag.clone();
        pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            println!("[{tag}] connection state: {state}");
            Box::pin(async {})
        }));
    }

    if offerer {
        let dc = pc.create_data_channel("data", None).await?;
        setup_data_channel(dc, tag.clone());

        // Give the answerer a moment to register before we send the offer.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let offer = pc.create_offer(None).await?;
        pc.set_local_description(offer.clone()).await?;
        if let Some(peer) = target.clone() {
            sender.signal(&peer, json!({ "kind": "offer", "sdp": offer.sdp }));
            println!("[{tag}] sent offer to {peer}");
        }
    } else {
        let tag2 = tag.clone();
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            setup_data_channel(dc, tag2.clone());
            Box::pin(async {})
        }));
    }

    // Drive the negotiation off the signaling channel.
    while let Some(msg) = signaling.recv().await {
        let ServerMsg::Signal { from, data } = msg else {
            continue;
        };
        match data.get("kind").and_then(|k| k.as_str()).unwrap_or("") {
            "offer" => {
                let sdp = data
                    .get("sdp")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                *peer_cell.lock().await = Some(from.clone());
                pc.set_remote_description(RTCSessionDescription::offer(sdp)?)
                    .await?;
                let answer = pc.create_answer(None).await?;
                pc.set_local_description(answer.clone()).await?;
                sender.signal(&from, json!({ "kind": "answer", "sdp": answer.sdp }));
                println!("[{tag}] sent answer to {from}");
            }
            "answer" => {
                let sdp = data
                    .get("sdp")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                pc.set_remote_description(RTCSessionDescription::answer(sdp)?)
                    .await?;
            }
            "ice" => {
                if let Some(c) = data.get("candidate") {
                    let init: RTCIceCandidateInit = serde_json::from_value(c.clone())?;
                    pc.add_ice_candidate(init).await?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
