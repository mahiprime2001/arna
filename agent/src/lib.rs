//! Arna agent core — screen capture + WebRTC video + input injection.
//!
//! The reusable agent loop lives here so both the headless binary
//! (`agent/src/main.rs`) and the Tauri desktop app (`agent-desktop/src-tauri`)
//! drive the same logic; they differ only in how they answer **consent**
//! (terminal/policy vs. a popup window). See [`run`].
//!
//! The screen is sent as a real **H.264 video track** (encoded with OpenH264):
//! the capture thread publishes downscaled RGB frames, and each admitted viewer
//! gets its own encoder feeding a WebRTC track. Control flows back over an
//! `input` data channel.

use std::io::ErrorKind::WouldBlock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use arna_core::p2p;
use arna_core::p2p::OnPeer;
use arna_core::webrtc::data_channel::RTCDataChannel;
use arna_core::webrtc::media::Sample;
use arna_core::webrtc::peer_connection::RTCPeerConnection;
use arna_core::webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use arna_core::webrtc::track::track_local::TrackLocal;
use arna_core::Signaling;
use bytes::Bytes;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use openh264::encoder::{Encoder, EncoderConfig, RateControlMode, UsageType};
use openh264::formats::{RgbSliceU8, YUVBuffer};
use openh264::OpenH264API;
use scrap::{Capturer, Display};
use tokio::sync::watch;

// Re-export the consent types so the apps can build a `ConsentFn` without a
// direct dependency on `arna-core`.
pub use arna_core::p2p::{Consent, ConnectRequest, ConsentFn};

/// Cap the encoded width; taller/wider screens are scaled down to keep software
/// H.264 encoding real-time. Height follows to preserve aspect ratio.
const TARGET_WIDTH: u32 = 1280;
/// Target bitrate for the H.264 stream (bits/sec).
const BITRATE_BPS: u32 = 4_000_000;
/// Force a keyframe every N frames so loss/late tune-in recovers (~4s @ 30fps).
const KEYFRAME_INTERVAL: u64 = 120;

/// A captured screen frame, downscaled to even dimensions and packed tight RGB.
/// `rgb` is `Arc`-shared so each viewer's encoder clones cheaply.
#[derive(Clone, Default)]
struct Frame {
    rgb: Arc<Vec<u8>>,
    w: usize,
    h: usize,
}

/// A one-time 6-digit session code, shown to both sides (a confirmation aid; it
/// does not gate the connection in the default Accept-only mode).
pub fn session_code() -> String {
    use rand::Rng;
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
}

fn even(n: u32) -> u32 {
    n & !1
}

// ---------------------------------------------------------------------------
// Screen capture
// ---------------------------------------------------------------------------

/// Unpack scrap's (possibly stride-padded) BGRA frame into tight RGB.
fn bgra_to_rgb(frame: &[u8], w: usize, h: usize) -> Vec<u8> {
    let stride = frame.len() / h;
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        let row = y * stride;
        for x in 0..w {
            let i = row + x * 4;
            rgb.push(frame[i + 2]);
            rgb.push(frame[i + 1]);
            rgb.push(frame[i]);
        }
    }
    rgb
}

fn capture_loop(tx: watch::Sender<Frame>) {
    let display = match Display::primary() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("agent: no primary display: {e}");
            return;
        }
    };
    let mut capturer = match Capturer::new(display) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("agent: failed to start capturer: {e}");
            return;
        }
    };
    let (w, h) = (capturer.width(), capturer.height());
    let tw = even((w.min(TARGET_WIDTH as usize)) as u32);
    let th = even((h as u32 * tw) / w as u32);
    println!("agent: capturing {w}x{h} -> encoding {tw}x{th}");

    loop {
        match capturer.frame() {
            Ok(frame) => {
                let rgb_full = bgra_to_rgb(&frame, w, h);
                let img = match image::RgbImage::from_raw(w as u32, h as u32, rgb_full) {
                    Some(i) => i,
                    None => continue,
                };
                let scaled =
                    image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle);
                let f = Frame {
                    rgb: Arc::new(scaled.into_raw()),
                    w: tw as usize,
                    h: th as usize,
                };
                if tx.send(f).is_err() {
                    break;
                }
            }
            Err(ref e) if e.kind() == WouldBlock => thread::sleep(Duration::from_millis(30)),
            Err(e) => {
                eprintln!("agent: capture error: {e}");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// H.264 video track (one encoder per viewer)
// ---------------------------------------------------------------------------

/// Encode the shared capture stream to H.264 for one viewer and push samples to
/// its WebRTC track until the connection ends.
fn spawn_video_encoder(track: Arc<TrackLocalStaticSample>, mut rx: watch::Receiver<Frame>) {
    tokio::spawn(async move {
        let cfg = EncoderConfig::new()
            .usage_type(UsageType::ScreenContentRealTime)
            .rate_control_mode(RateControlMode::Bitrate)
            .set_bitrate_bps(BITRATE_BPS)
            .max_frame_rate(30.0)
            .enable_skip_frame(true);
        let mut encoder = match Encoder::with_api_config(OpenH264API::from_source(), cfg) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("agent: H.264 encoder init failed: {e}");
                return;
            }
        };

        let mut n: u64 = 0;
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let frame = rx.borrow_and_update().clone();
            if frame.rgb.is_empty() {
                continue;
            }

            if n > 0 && n % KEYFRAME_INTERVAL == 0 {
                encoder.force_intra_frame();
            }
            let src = RgbSliceU8::new(&frame.rgb, (frame.w, frame.h));
            let yuv = YUVBuffer::from_rgb_source(src);
            let encoded = encoder.encode(&yuv).map(|b| b.to_vec());
            n += 1;

            match encoded {
                Ok(data) if !data.is_empty() => {
                    let sample = Sample {
                        data: Bytes::from(data),
                        duration: Duration::from_millis(33),
                        ..Default::default()
                    };
                    if let Err(e) = track.write_sample(&sample).await {
                        eprintln!("agent: write_sample failed: {e}");
                        break;
                    }
                    if n == 1 {
                        println!("agent: streaming H.264 video");
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("agent: H.264 encode error: {e}"),
            }
        }
    });
}

/// Build the per-connection callback that attaches a screen video track to each
/// admitted viewer and starts encoding for it.
fn make_on_peer(rx: watch::Receiver<Frame>) -> OnPeer {
    Arc::new(move |pc: Arc<RTCPeerConnection>, peer: String| {
        let rx = rx.clone();
        Box::pin(async move {
            let track = Arc::new(TrackLocalStaticSample::new(
                // Must match the codec registered in core's media engine, or
                // webrtc-rs won't bind the track and no RTP is sent.
                p2p::h264_capability(),
                "video".to_owned(),
                format!("arna-screen-{peer}"),
            ));

            match pc
                .add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>)
                .await
            {
                Ok(rtp_sender) => {
                    // Drain RTCP (receiver reports etc.) so the sender keeps flowing.
                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 1500];
                        while rtp_sender.read(&mut buf).await.is_ok() {}
                    });
                    spawn_video_encoder(track, rx);
                }
                Err(e) => eprintln!("agent: add_track failed for {peer}: {e}"),
            }
        })
    })
}

// ---------------------------------------------------------------------------
// Input injection
// ---------------------------------------------------------------------------

fn mouse_button(b: Option<u64>) -> Button {
    match b {
        Some(2) => Button::Right,
        Some(1) => Button::Middle,
        _ => Button::Left,
    }
}

/// Map a browser `KeyboardEvent.key` to an enigo key.
fn map_key(k: &str) -> Option<Key> {
    Some(match k {
        "Enter" => Key::Return,
        "Backspace" => Key::Backspace,
        "Tab" => Key::Tab,
        "Escape" | "Esc" => Key::Escape,
        "Shift" => Key::Shift,
        "Control" => Key::Control,
        "Alt" => Key::Alt,
        "Meta" => Key::Meta,
        " " | "Spacebar" | "Space" => Key::Space,
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,
        "Delete" | "Del" => Key::Delete,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "CapsLock" => Key::CapsLock,
        _ => {
            let mut chars = k.chars();
            let c = chars.next()?;
            if chars.next().is_none() {
                Key::Unicode(c)
            } else {
                return None; // named key we don't handle
            }
        }
    })
}

/// Apply one input event (JSON) to the local machine.
fn handle_input(json: &str, enigo: &Mutex<Enigo>, screen_w: i32, screen_h: i32) {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("t").and_then(|t| t.as_str()).unwrap_or("");
    let Ok(mut e) = enigo.lock() else { return };

    match t {
        "m" => {
            let x = (v.get("x").and_then(|n| n.as_f64()).unwrap_or(0.0) * screen_w as f64) as i32;
            let y = (v.get("y").and_then(|n| n.as_f64()).unwrap_or(0.0) * screen_h as f64) as i32;
            let _ = e.move_mouse(x, y, Coordinate::Abs);
        }
        "d" => {
            let _ = e.button(
                mouse_button(v.get("b").and_then(|b| b.as_u64())),
                Direction::Press,
            );
        }
        "u" => {
            let _ = e.button(
                mouse_button(v.get("b").and_then(|b| b.as_u64())),
                Direction::Release,
            );
        }
        "w" => {
            let dy = v.get("dy").and_then(|n| n.as_f64()).unwrap_or(0.0);
            if dy != 0.0 {
                let _ = e.scroll(if dy > 0.0 { 1 } else { -1 }, Axis::Vertical);
            }
        }
        "kd" => {
            if let Some(key) = v.get("k").and_then(|k| k.as_str()).and_then(map_key) {
                // Printable chars are typed on key-down (Click); modifiers and
                // named keys are held (Press) until key-up.
                let dir = if matches!(key, Key::Unicode(_)) {
                    Direction::Click
                } else {
                    Direction::Press
                };
                let _ = e.key(key, dir);
            }
        }
        "ku" => {
            if let Some(key) = v.get("k").and_then(|k| k.as_str()).and_then(map_key) {
                if !matches!(key, Key::Unicode(_)) {
                    let _ = e.key(key, Direction::Release);
                }
            }
        }
        _ => {}
    }
}

fn primary_size() -> (i32, i32) {
    match Display::primary() {
        Ok(d) => (d.width() as i32, d.height() as i32),
        Err(_) => (1920, 1080),
    }
}

// ---------------------------------------------------------------------------
// The agent loop
// ---------------------------------------------------------------------------

/// Run the agent: capture the primary screen, register on the signaling
/// backend, and serve admitted viewers (H.264 screen video + inject their
/// input). `consent` decides whether each requesting console is admitted.
/// Returns when the signaling socket closes.
pub async fn run(url: String, id: String, consent: ConsentFn) {
    let (screen_w, screen_h) = primary_size();

    // Capture thread publishes the latest downscaled RGB frame.
    let (tx, rx) = watch::channel(Frame::default());
    thread::spawn(move || capture_loop(tx));

    // One shared input injector.
    let enigo = Arc::new(Mutex::new(
        Enigo::new(&Settings::default()).expect("failed to init input injector"),
    ));

    let signaling = match Signaling::connect(&url).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("agent: failed to connect to signaling backend: {e}");
            return;
        }
    };
    signaling.register("agent", &id);
    println!("agent registered as '{id}' — waiting for a viewer...");

    // Per admitted connection: attach a screen video track + start encoding.
    let on_peer = make_on_peer(rx);

    // Each opened data channel: "input" -> inject the events the viewer sends.
    let on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync> = Arc::new(move |dc| {
        if dc.label() == "input" {
            println!("agent: control channel open");
            let enigo = enigo.clone();
            dc.on_message(Box::new(move |msg| {
                let enigo = enigo.clone();
                let text = String::from_utf8_lossy(&msg.data).to_string();
                Box::pin(async move {
                    handle_input(&text, &enigo, screen_w, screen_h);
                })
            }));
        } else {
            println!("agent: ignoring unknown channel '{}'", dc.label());
        }
    });

    if let Err(e) = p2p::answer_streaming(signaling, id, consent, on_peer, on_channel).await {
        eprintln!("agent error: {e}");
    }
}
