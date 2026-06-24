//! WebRTC peer-to-peer over the signaling channel.
//!
//! Two peers negotiate a real `RTCPeerConnection` — SDP offer/answer and ICE
//! candidates travel as opaque `signal` payloads through the backend — then open
//! a data channel and exchange messages directly (P2P, or via TURN later).
//!
//! - [`run`] is the simple two-peer demo (offerer/answerer + a hello).
//! - [`answer_streaming`] answers offers and hands each opened data channel to a
//!   callback. It builds a **fresh** peer connection per offer (keyed by the
//!   viewer id), so reconnects and multiple viewers all work.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::Mutex;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::{APIBuilder, API};
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice::mdns::MulticastDnsMode;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use crate::{Error, ServerMsg, SignalSender, Signaling};

/// Build a WebRTC API with default codecs + interceptors and mDNS enabled.
///
/// Browsers obfuscate host candidates as mDNS `.local` names; without mDNS,
/// Chrome <-> webrtc-rs ICE gets stuck at "connecting".
fn make_api() -> Result<API, Error> {
    let mut media = MediaEngine::default();
    media.register_default_codecs()?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media)?;
    let mut setting = SettingEngine::default();
    setting.set_ice_multicast_dns_mode(MulticastDnsMode::QueryAndGather);
    Ok(APIBuilder::new()
        .with_setting_engine(setting)
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build())
}

fn rtc_config() -> RTCConfiguration {
    RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Wire trickle-ICE on a connection that talks to a single, known peer.
fn wire_ice(pc: &Arc<RTCPeerConnection>, sender: SignalSender, peer: String) {
    pc.on_ice_candidate(Box::new(move |candidate| {
        let sender = sender.clone();
        let peer = peer.clone();
        Box::pin(async move {
            if let Some(c) = candidate {
                if let Ok(init) = c.to_json() {
                    sender.signal(&peer, json!({ "kind": "ice", "candidate": init }));
                }
            }
        })
    }));
}

// ---------------------------------------------------------------------------
// Streaming answerer (the agent)
// ---------------------------------------------------------------------------

/// Answer incoming offers; when a data channel opens, hand it to `on_channel`.
///
/// A **new** peer connection is created per offer (keyed by the viewer id), so
/// connect/disconnect/reconnect and multiple simultaneous viewers all work.
/// Runs until the signaling socket closes.
pub async fn answer_streaming(
    mut signaling: Signaling,
    tag: String,
    on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync>,
) -> Result<(), Error> {
    let api = make_api()?;
    let sender = signaling.sender();
    let sessions: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(HashMap::new()));

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

                let pc = Arc::new(api.new_peer_connection(rtc_config()).await?);
                wire_ice(&pc, sender.clone(), from.clone());

                // Drop the session when this connection ends.
                {
                    let sessions = sessions.clone();
                    let peer = from.clone();
                    let tag = tag.clone();
                    pc.on_peer_connection_state_change(Box::new(
                        move |s: RTCPeerConnectionState| {
                            println!("[{tag}] {peer}: {s}");
                            let sessions = sessions.clone();
                            let peer = peer.clone();
                            Box::pin(async move {
                                if matches!(
                                    s,
                                    RTCPeerConnectionState::Failed
                                        | RTCPeerConnectionState::Disconnected
                                        | RTCPeerConnectionState::Closed
                                ) {
                                    sessions.lock().await.remove(&peer);
                                }
                            })
                        },
                    ));
                }

                // Hand each opened data channel to the streamer callback.
                {
                    let on_channel = on_channel.clone();
                    let tag = tag.clone();
                    pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                        let on_channel = on_channel.clone();
                        let dc_open = dc.clone();
                        let tag = tag.clone();
                        Box::pin(async move {
                            dc.on_open(Box::new(move || {
                                let on_channel = on_channel.clone();
                                let dc_open = dc_open.clone();
                                let tag = tag.clone();
                                Box::pin(async move {
                                    println!("[{tag}] viewer channel open — streaming");
                                    on_channel(dc_open);
                                })
                            }));
                        })
                    }));
                }

                pc.set_remote_description(RTCSessionDescription::offer(sdp)?)
                    .await?;
                let answer = pc.create_answer(None).await?;
                pc.set_local_description(answer.clone()).await?;
                sender.signal(&from, json!({ "kind": "answer", "sdp": answer.sdp }));
                println!("[{tag}] sent answer to {from}");

                sessions.lock().await.insert(from.clone(), pc);
            }
            "ice" => {
                if let Some(c) = data.get("candidate") {
                    let init: RTCIceCandidateInit = serde_json::from_value(c.clone())?;
                    if let Some(pc) = sessions.lock().await.get(&from).cloned() {
                        let _ = pc.add_ice_candidate(init).await;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Two-peer demo (the poc)
// ---------------------------------------------------------------------------

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
    mut signaling: Signaling,
    target: Option<String>,
    offerer: bool,
    tag: String,
) -> Result<(), Error> {
    let api = make_api()?;
    let sender = signaling.sender();
    let pc = Arc::new(api.new_peer_connection(rtc_config()).await?);
    let peer_cell: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(target.clone()));

    // ICE candidates -> the (possibly not-yet-known) peer.
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
        let tag = tag.clone();
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            println!("[{tag}] connection state: {s}");
            Box::pin(async {})
        }));
    }

    if offerer {
        let dc = pc.create_data_channel("data", None).await?;
        setup_demo_channel(dc, tag.clone());
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
                    let _ = pc.add_ice_candidate(init).await;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
