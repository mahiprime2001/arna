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

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::Mutex;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264};
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
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::rtp_transceiver::RTCPFeedback;

use crate::{Error, ServerMsg, SignalSender, Signaling};

/// The single H.264 video codec the agent streams: constrained-baseline,
/// packetization-mode 1. The agent's video track **must** be created with this
/// exact capability so webrtc-rs binds the track to the negotiated codec (an
/// empty/mismatched fmtp silently produces no RTP).
pub fn h264_capability() -> RTCRtpCodecCapability {
    RTCRtpCodecCapability {
        mime_type: MIME_TYPE_H264.to_owned(),
        clock_rate: 90_000,
        channels: 0,
        sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
            .to_owned(),
        rtcp_feedback: vec![
            RTCPFeedback {
                typ: "nack".to_owned(),
                parameter: String::new(),
            },
            RTCPFeedback {
                typ: "nack".to_owned(),
                parameter: "pli".to_owned(),
            },
            RTCPFeedback {
                typ: "goog-remb".to_owned(),
                parameter: String::new(),
            },
        ],
    }
}

/// Build a WebRTC API with mDNS enabled and a **single** H.264 video codec.
///
/// - mDNS: browsers obfuscate host candidates as `.local` names; without it,
///   Chrome <-> webrtc-rs ICE gets stuck at "connecting".
/// - One codec: registering only H.264 (matching [`h264_capability`]) keeps the
///   answer unambiguous so the agent's screen track binds and actually sends.
fn make_api() -> Result<API, Error> {
    let mut media = MediaEngine::default();
    media.register_codec(
        RTCRtpCodecParameters {
            capability: h264_capability(),
            payload_type: 102,
            ..Default::default()
        },
        RTPCodecType::Video,
    )?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media)?;
    let mut setting = SettingEngine::default();
    // QueryOnly: resolve the browser's `.local` (mDNS) candidates, but advertise
    // our own host candidates as real IPs. (QueryAndGather puts a second mDNS
    // responder on the box, which makes same-machine ICE flaky.)
    setting.set_ice_multicast_dns_mode(MulticastDnsMode::QueryOnly);
    Ok(APIBuilder::new()
        .with_setting_engine(setting)
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build())
}

/// Fallback STUN when the backend configures no ICE servers of its own.
fn default_ice() -> Vec<RTCIceServer> {
    vec![RTCIceServer {
        urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        ..Default::default()
    }]
}

/// Translate the backend's ICE config into webrtc-rs servers (dropping entries
/// with no urls).
fn to_rtc_ice(servers: &[crate::IceServer]) -> Vec<RTCIceServer> {
    servers
        .iter()
        .filter(|s| !s.urls.is_empty())
        .map(|s| RTCIceServer {
            urls: s.urls.clone(),
            username: s.username.clone().unwrap_or_default(),
            credential: s.credential.clone().unwrap_or_default(),
            ..Default::default()
        })
        .collect()
}

fn rtc_config(ice: &[RTCIceServer]) -> RTCConfiguration {
    RTCConfiguration {
        ice_servers: if ice.is_empty() {
            default_ice()
        } else {
            ice.to_vec()
        },
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
// Consent gate (the agent decides whether to admit a console)
// ---------------------------------------------------------------------------

/// A console asking to start a session (already SSO-verified by the backend).
pub struct ConnectRequest {
    /// Signaling id of the requesting console.
    pub from: String,
    /// Verified admin identity to show in the popup.
    pub name: String,
}

/// The agent's answer to a [`ConnectRequest`].
pub enum Consent {
    /// Admit the console. `code` (if any) is shown to both sides as a bonus.
    Accept { code: Option<String> },
    /// Admit only after the console echoes this exact code (the operator reads it
    /// out, the caller types it in). Guards against blind-accept social engineering.
    AskCode { code: String },
    /// Refuse, with a short human-readable reason.
    Decline { reason: String },
}

/// Async callback the agent supplies to decide consent (terminal prompt, fixed
/// policy, or — once wrapped in Tauri — an always-on-top window).
pub type ConsentFn =
    Arc<dyn Fn(ConnectRequest) -> Pin<Box<dyn Future<Output = Consent> + Send>> + Send + Sync>;

/// Called for each admitted peer connection (after the remote offer is set, before
/// the answer is created) so the agent can attach media tracks — e.g. the screen
/// video track — to it. Keeps codec/capture concerns out of `core`.
pub type OnPeer = Arc<
    dyn Fn(Arc<RTCPeerConnection>, String) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

// ---------------------------------------------------------------------------
// Streaming answerer (the agent)
// ---------------------------------------------------------------------------

/// Answer incoming offers; when a data channel opens, hand it to `on_channel`.
///
/// Consent is enforced: the backend delivers `incoming_request` first, we ask
/// `consent`, reply over signaling, and only **answer offers from peers we have
/// admitted**. A **new** peer connection is created per offer (keyed by the
/// viewer id), so connect/disconnect/reconnect and multiple viewers all work.
/// Runs until the signaling socket closes.
pub async fn answer_streaming(
    mut signaling: Signaling,
    tag: String,
    consent: ConsentFn,
    on_peer: OnPeer,
    on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync>,
) -> Result<(), Error> {
    let api = make_api()?;
    let sender = signaling.sender();
    let sessions: Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    // Consoles the operator has admitted; an offer from anyone else is ignored.
    let approved: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    // Consoles awaiting code entry (require-code mode): peer -> (expected, attempts).
    let pending_codes: Arc<Mutex<HashMap<String, (String, u32)>>> =
        Arc::new(Mutex::new(HashMap::new()));
    // STUN/TURN servers, learned from the backend's `registered` reply (sent
    // before any offer arrives). Empty until then → default STUN.
    let mut ice: Vec<RTCIceServer> = Vec::new();

    while let Some(msg) = signaling.recv().await {
        let (from, data) = match msg {
            ServerMsg::Registered { ice_servers, .. } => {
                let conv = to_rtc_ice(&ice_servers);
                if !conv.is_empty() {
                    println!("[{tag}] using {} ICE server(s) from backend", conv.len());
                    ice = conv;
                }
                continue;
            }
            ServerMsg::IncomingRequest { from, name } => {
                println!("[{tag}] connection request from {name} ({from})");
                let consent = consent.clone();
                let sender = sender.clone();
                let approved = approved.clone();
                let pending_codes = pending_codes.clone();
                let tag = tag.clone();
                tokio::spawn(async move {
                    match consent(ConnectRequest {
                        from: from.clone(),
                        name,
                    })
                    .await
                    {
                        Consent::Accept { code } => {
                            approved.lock().await.insert(from.clone());
                            println!("[{tag}] admitted {from}");
                            sender.signal(
                                &from,
                                json!({ "kind": "consent", "accepted": true, "code": code }),
                            );
                        }
                        Consent::AskCode { code } => {
                            pending_codes.lock().await.insert(from.clone(), (code, 0));
                            println!("[{tag}] {from}: waiting for the caller to enter the code");
                            sender.signal(
                                &from,
                                json!({ "kind": "consent", "accepted": true, "require_code": true }),
                            );
                        }
                        Consent::Decline { reason } => {
                            println!("[{tag}] declined {from}: {reason}");
                            sender.signal(
                                &from,
                                json!({ "kind": "consent", "accepted": false, "reason": reason }),
                            );
                        }
                    }
                });
                continue;
            }
            ServerMsg::Signal { from, data } => (from, data),
            _ => continue,
        };
        match data.get("kind").and_then(|k| k.as_str()).unwrap_or("") {
            "code" => {
                let got = data.get("code").and_then(|c| c.as_str()).unwrap_or("");
                let mut codes = pending_codes.lock().await;
                // Some(true) = correct, Some(false) = wrong, None = nothing pending.
                let matched = match codes.get_mut(&from) {
                    Some((expected, attempts)) => {
                        if got == expected {
                            Some(true)
                        } else {
                            *attempts += 1;
                            Some(false)
                        }
                    }
                    None => None,
                };
                match matched {
                    Some(true) => {
                        codes.remove(&from);
                        drop(codes);
                        approved.lock().await.insert(from.clone());
                        println!("[{tag}] {from}: correct code — admitted");
                        sender.signal(&from, json!({ "kind": "code_ok" }));
                    }
                    Some(false) => {
                        let exhausted = codes.get(&from).map(|(_, a)| *a >= 3).unwrap_or(true);
                        if exhausted {
                            codes.remove(&from);
                        }
                        println!("[{tag}] {from}: wrong code (final={exhausted})");
                        sender.signal(&from, json!({ "kind": "code_bad", "final": exhausted }));
                    }
                    None => {}
                }
            }
            "offer" => {
                if !approved.lock().await.contains(&from) {
                    println!("[{tag}] ignoring offer from unapproved peer {from}");
                    continue;
                }
                let sdp = data
                    .get("sdp")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();

                let pc = Arc::new(api.new_peer_connection(rtc_config(&ice)).await?);
                wire_ice(&pc, sender.clone(), from.clone());

                // Drop the session — and revoke approval — when it ends, so a
                // reconnect has to ask for consent again.
                {
                    let sessions = sessions.clone();
                    let approved = approved.clone();
                    let pending_codes = pending_codes.clone();
                    let peer = from.clone();
                    let tag = tag.clone();
                    pc.on_peer_connection_state_change(Box::new(
                        move |s: RTCPeerConnectionState| {
                            println!("[{tag}] {peer}: {s}");
                            let sessions = sessions.clone();
                            let approved = approved.clone();
                            let pending_codes = pending_codes.clone();
                            let peer = peer.clone();
                            Box::pin(async move {
                                if matches!(
                                    s,
                                    RTCPeerConnectionState::Failed
                                        | RTCPeerConnectionState::Disconnected
                                        | RTCPeerConnectionState::Closed
                                ) {
                                    sessions.lock().await.remove(&peer);
                                    approved.lock().await.remove(&peer);
                                    pending_codes.lock().await.remove(&peer);
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

                // Attach the agent's screen video track *before* applying the
                // remote offer. webrtc-rs only marks a sender "negotiated" (and
                // later binds it) when generating the answer for a transceiver it
                // can match by mid; adding the track first makes that matching
                // reliable, otherwise the sender never binds and emits no RTP.
                on_peer(pc.clone(), from.clone()).await;
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
    let pc = Arc::new(api.new_peer_connection(rtc_config(&[])).await?);
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
