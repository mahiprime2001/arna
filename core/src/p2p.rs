//! WebRTC peer-to-peer over the signaling channel.
//!
//! Two peers negotiate a real `RTCPeerConnection` — SDP offer/answer and ICE
//! candidates travel as opaque `signal` payloads through the backend — then open
//! a data channel and exchange messages directly (P2P, or via TURN later).
//!
//! - [`run`] is the simple two-peer demo (offerer/answerer + a hello).
//! - [`answer_streaming`] answers offers and hands each opened data channel to a
//!   callback — the agent uses this to stream screen frames to a viewer.

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
use webrtc::peer_connection::RTCPeerConnection;

use crate::{Error, ServerMsg, SignalSender, Signaling};

/// Build a peer connection wired for trickle ICE over signaling.
///
/// `target` is who we trickle ICE to — known up front for an offerer, or `None`
/// for an answerer (it's learned from the incoming offer and stored in the
/// returned cell before that side starts gathering candidates).
async fn build_pc(
    signaling: &Signaling,
    target: Option<String>,
    tag: &str,
) -> Result<
    (
        Arc<RTCPeerConnection>,
        SignalSender,
        Arc<Mutex<Option<String>>>,
    ),
    Error,
> {
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
    let peer_cell = Arc::new(Mutex::new(target));

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

    {
        let tag = tag.to_string();
        pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            println!("[{tag}] connection state: {state}");
            Box::pin(async {})
        }));
    }

    Ok((pc, sender, peer_cell))
}

/// Drive negotiation off the signaling channel until the socket closes.
async fn negotiation_loop(
    pc: Arc<RTCPeerConnection>,
    mut signaling: Signaling,
    sender: SignalSender,
    peer_cell: Arc<Mutex<Option<String>>>,
    tag: &str,
) -> Result<(), Error> {
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

/// Wire up logging + a greeting on a data channel (used by the [`run`] demo).
fn setup_demo_channel(dc: Arc<RTCDataChannel>, tag: String) {
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

/// Two-peer demo: `offerer = true` creates the channel + offer; otherwise waits.
pub async fn run(
    signaling: Signaling,
    target: Option<String>,
    offerer: bool,
    tag: String,
) -> Result<(), Error> {
    let (pc, sender, peer_cell) = build_pc(&signaling, target.clone(), &tag).await?;

    if offerer {
        let dc = pc.create_data_channel("data", None).await?;
        setup_demo_channel(dc, tag.clone());
        // Give the answerer a moment to register before sending the offer.
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
            setup_demo_channel(dc, tag2.clone());
            Box::pin(async {})
        }));
    }

    negotiation_loop(pc, signaling, sender, peer_cell, &tag).await
}

/// Answer incoming offers; when a data channel opens, hand it to `on_channel`.
///
/// The agent uses this: `on_channel` receives the live channel and starts
/// pushing screen frames onto it. Runs until the signaling socket closes.
pub async fn answer_streaming(
    signaling: Signaling,
    tag: String,
    on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync>,
) -> Result<(), Error> {
    let (pc, sender, peer_cell) = build_pc(&signaling, None, &tag).await?;

    let tag_dc = tag.clone();
    pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let on_channel = on_channel.clone();
        let dc_open = dc.clone();
        let tag_dc = tag_dc.clone();
        Box::pin(async move {
            dc.on_open(Box::new(move || {
                let on_channel = on_channel.clone();
                let dc_open = dc_open.clone();
                let tag_dc = tag_dc.clone();
                Box::pin(async move {
                    println!("[{tag_dc}] viewer channel open — streaming");
                    on_channel(dc_open);
                })
            }));
        })
    }));

    negotiation_loop(pc, signaling, sender, peer_cell, &tag).await
}
