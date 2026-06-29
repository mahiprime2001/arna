//! Arna agent core — screen capture + WebRTC video + input injection.
//!
//! The reusable agent loop lives here so both the headless binary
//! (`agent/src/main.rs`) and the unified Arna desktop app (`console/src-tauri`)
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
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::sync::{watch, Mutex as AsyncMutex};

mod winmon;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_screens() {
        winmon::make_dpi_aware();
        let screens = enumerate_screens();
        println!("found {} screen(s):", screens.len());
        for (i, s) in screens.iter().enumerate() {
            println!(
                "  [{i}] {}x{} at ({},{}) primary={}",
                s.width, s.height, s.ox, s.oy, s.primary
            );
        }
        println!("announce: {}", monitors_announce());
        assert!(!screens.is_empty());
    }
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

/// A capturable screen: scrap's `all()` order defines the index, enriched with
/// each monitor's desktop origin + primary flag (from Win32, matched by
/// resolution since scrap doesn't expose monitor positions).
#[derive(Clone, Copy)]
struct ScreenInfo {
    ox: i32,
    oy: i32,
    width: i32,
    height: i32,
    primary: bool,
}

/// Enumerate capturable screens. scrap defines order/index (so a chosen index
/// maps to the right `Capturer`); Win32 supplies the origin + primary flag,
/// joined by matching resolution (greedy; two identical monitors may swap, which
/// only matters for input on the rarer of the two).
fn enumerate_screens() -> Vec<ScreenInfo> {
    let wins = winmon::monitors();
    let mut used = vec![false; wins.len()];
    let displays = Display::all().unwrap_or_default();
    let mut out = Vec::new();
    for (i, d) in displays.iter().enumerate() {
        let (w, h) = (d.width() as i32, d.height() as i32);
        let matched = wins
            .iter()
            .enumerate()
            .find(|(j, m)| !used[*j] && m.width == w && m.height == h);
        let (ox, oy, primary) = match matched {
            Some((j, m)) => {
                used[j] = true;
                (m.x, m.y, m.primary)
            }
            None => (0, 0, i == 0),
        };
        out.push(ScreenInfo {
            ox,
            oy,
            width: w,
            height: h,
            primary,
        });
    }
    if out.is_empty() {
        let (w, h) = primary_size();
        out.push(ScreenInfo {
            ox: 0,
            oy: 0,
            width: w,
            height: h,
            primary: true,
        });
    }
    out
}

/// JSON sent to the console on the input channel listing the screens it can pick
/// from: `{ "t":"monitors", "list":[{ index, label, width, height, primary }] }`.
fn monitors_announce() -> String {
    let list: Vec<serde_json::Value> = enumerate_screens()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            serde_json::json!({
                "index": i,
                "label": format!("Screen {}", i + 1),
                "width": s.width,
                "height": s.height,
                "primary": s.primary,
            })
        })
        .collect();
    serde_json::json!({ "t": "monitors", "list": list }).to_string()
}

/// Capture the *selected* screen, downscale, and publish frames. Switching the
/// selection (via `sel`) rebuilds the capturer on the new display; the active
/// screen's origin + full size is published on `active` so input maps correctly.
fn capture_loop(
    tx: watch::Sender<Frame>,
    mut sel: watch::Receiver<usize>,
    active: watch::Sender<(i32, i32, i32, i32)>,
) {
    loop {
        let screens = enumerate_screens();
        let idx = (*sel.borrow_and_update()).min(screens.len().saturating_sub(1));
        let info = screens[idx];

        // Re-fetch owned displays to build the capturer for the chosen index.
        let displays = match Display::all() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("agent: cannot list displays: {e}");
                return;
            }
        };
        let display = match displays.into_iter().nth(idx).or_else(|| Display::primary().ok()) {
            Some(d) => d,
            None => {
                eprintln!("agent: no display to capture");
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
        let _ = active.send((info.ox, info.oy, w as i32, h as i32));
        println!("agent: capturing screen {idx} {w}x{h} -> encoding {tw}x{th}");

        loop {
            // Selection changed → drop this capturer and rebuild on the new screen.
            if sel.has_changed().unwrap_or(false) {
                break;
            }
            match capturer.frame() {
                Ok(frame) => {
                    let rgb_full = bgra_to_rgb(&frame, w, h);
                    let img = match image::RgbImage::from_raw(w as u32, h as u32, rgb_full) {
                        Some(i) => i,
                        None => continue,
                    };
                    let scaled = image::imageops::resize(
                        &img,
                        tw,
                        th,
                        image::imageops::FilterType::Triangle,
                    );
                    let f = Frame {
                        rgb: Arc::new(scaled.into_raw()),
                        w: tw as usize,
                        h: th as usize,
                    };
                    if tx.send(f).is_err() {
                        return;
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
}

// ---------------------------------------------------------------------------
// H.264 video track (one encoder per viewer)
// ---------------------------------------------------------------------------

/// Build a fresh H.264 encoder. Called again whenever the frame resolution
/// changes (e.g. the operator switches to a differently-sized monitor) — OpenH264
/// fixes its dimensions from the first frame, so a size change needs a new one.
fn new_encoder() -> Option<Encoder> {
    let cfg = EncoderConfig::new()
        .usage_type(UsageType::ScreenContentRealTime)
        .rate_control_mode(RateControlMode::Bitrate)
        .set_bitrate_bps(BITRATE_BPS)
        .max_frame_rate(30.0)
        .enable_skip_frame(true);
    match Encoder::with_api_config(OpenH264API::from_source(), cfg) {
        Ok(e) => Some(e),
        Err(e) => {
            eprintln!("agent: H.264 encoder init failed: {e}");
            None
        }
    }
}

/// Encode the shared capture stream to H.264 for one viewer and push samples to
/// its WebRTC track until the connection ends.
fn spawn_video_encoder(track: Arc<TrackLocalStaticSample>, mut rx: watch::Receiver<Frame>) {
    tokio::spawn(async move {
        let mut encoder = match new_encoder() {
            Some(e) => e,
            None => return,
        };
        // Dimensions the current encoder was built for; a change rebuilds it.
        let mut dims = (0usize, 0usize);

        let mut n: u64 = 0;
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let frame = rx.borrow_and_update().clone();
            if frame.rgb.is_empty() {
                continue;
            }

            // Resolution changed (monitor switch) → rebuild; first frame is an IDR.
            if (frame.w, frame.h) != dims {
                match new_encoder() {
                    Some(e) => encoder = e,
                    None => continue,
                }
                dims = (frame.w, frame.h);
                n = 0;
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

/// Apply one input event (JSON) to the local machine. `active` carries the
/// streamed screen's `(origin_x, origin_y, width, height)` so normalized mouse
/// coords land on the right monitor; `select` lets the console switch monitors.
fn handle_input(
    json: &str,
    enigo: &Mutex<Enigo>,
    active: &watch::Receiver<(i32, i32, i32, i32)>,
    select: &watch::Sender<usize>,
) {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("t").and_then(|t| t.as_str()).unwrap_or("");

    // Monitor switch: no enigo lock needed.
    if t == "display" {
        if let Some(i) = v.get("i").and_then(|n| n.as_u64()) {
            let _ = select.send(i as usize);
        }
        return;
    }

    let Ok(mut e) = enigo.lock() else { return };

    match t {
        "m" => {
            let (ox, oy, w, h) = *active.borrow();
            let nx = v.get("x").and_then(|n| n.as_f64()).unwrap_or(0.0);
            let ny = v.get("y").and_then(|n| n.as_f64()).unwrap_or(0.0);
            // Map the normalized point onto the active monitor in global
            // virtual-desktop pixels.
            let gx = ox + (nx * w as f64) as i32;
            let gy = oy + (ny * h as f64) as i32;
            if winmon::CAN_TARGET_ANY_MONITOR {
                winmon::move_to_global(gx, gy);
            } else {
                // Fallback (non-Windows): enigo's absolute path; correct on the
                // primary monitor (origin 0,0).
                let _ = e.move_mouse(gx, gy, Coordinate::Abs);
            }
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
// File transfer (console -> agent): files land in ~/ArnaRemote/Incoming
// ---------------------------------------------------------------------------

/// Where received files are saved on the store PC.
fn incoming_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join("ArnaRemote").join("Incoming")
}

/// Keep only the file name (drop any path the sender included) so a transfer
/// can't write outside the incoming folder.
fn safe_file_name(name: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("file")
        .to_string()
}

/// Pick a non-clobbering path: `name`, then `name (1)`, `name (2)`, …
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|s| s.to_str());
    for i in 1..10_000 {
        let candidate = match ext {
            Some(e) => dir.join(format!("{stem} ({i}).{e}")),
            None => dir.join(format!("{stem} ({i})")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    first
}

/// An in-progress incoming file.
struct IncomingFile {
    name: String,
    file: tokio::fs::File,
    received: u64,
}

/// A file the operator chose to send back to the console (download).
pub struct DownloadFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Asked when the console requests a download: returns the file the operator
/// picks, or `None` if they cancel. (A native dialog in the desktop app; a fixed
/// file for the headless agent.)
pub type DownloadProvider =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<DownloadFile>> + Send>> + Send + Sync>;

/// Stream a chosen file back to the console over the `files` channel:
/// `dl_start` → binary chunks (throttled by `buffered_amount`) → `dl_end`.
async fn stream_download(dc: Arc<RTCDataChannel>, download: DownloadProvider) {
    let Some(file) = download().await else {
        let _ = dc
            .send_text(serde_json::json!({ "t": "dl_cancel", "reason": "cancelled" }).to_string())
            .await;
        return;
    };
    let size = file.bytes.len();
    if dc
        .send_text(serde_json::json!({ "t": "dl_start", "name": file.name, "size": size }).to_string())
        .await
        .is_err()
    {
        return;
    }
    for chunk in file.bytes.chunks(16 * 1024) {
        while dc.buffered_amount().await > 1024 * 1024 {
            tokio::time::sleep(StdDuration::from_millis(8)).await;
        }
        if dc.send(&Bytes::from(chunk.to_vec())).await.is_err() {
            return;
        }
    }
    let _ = dc.send_text(serde_json::json!({ "t": "dl_end" }).to_string()).await;
    println!("agent: sent '{}' ({size} bytes) to console", file.name);
}

/// Wire up a `files` data channel: text frames are control messages
/// (`file_start`/`file_end` for uploads, `dl_request` for downloads), binary
/// frames are upload chunks. One transfer at a time per channel.
fn handle_files_channel(dc: Arc<RTCDataChannel>, download: DownloadProvider) {
    println!("agent: file channel open");
    let state: Arc<AsyncMutex<Option<IncomingFile>>> = Arc::new(AsyncMutex::new(None));
    let ack = dc.clone();
    dc.on_message(Box::new(move |msg| {
        let state = state.clone();
        let ack = ack.clone();
        let download = download.clone();
        Box::pin(async move {
            if msg.is_string {
                let text = String::from_utf8_lossy(&msg.data);
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    return;
                };
                match v.get("t").and_then(|t| t.as_str()).unwrap_or("") {
                    "dl_request" => {
                        let dc = ack.clone();
                        tokio::spawn(async move { stream_download(dc, download).await });
                    }
                    "file_start" => {
                        let name =
                            safe_file_name(v.get("name").and_then(|n| n.as_str()).unwrap_or("file"));
                        let size = v.get("size").and_then(|n| n.as_u64()).unwrap_or(0);
                        let dir = incoming_dir();
                        let _ = tokio::fs::create_dir_all(&dir).await;
                        let path = unique_path(&dir, &name);
                        match tokio::fs::File::create(&path).await {
                            Ok(file) => {
                                println!(
                                    "agent: receiving '{name}' ({size} bytes) -> {}",
                                    path.display()
                                );
                                *state.lock().await = Some(IncomingFile {
                                    name,
                                    file,
                                    received: 0,
                                });
                            }
                            Err(e) => eprintln!("agent: cannot create '{}': {e}", path.display()),
                        }
                    }
                    "file_end" => {
                        if let Some(mut inc) = state.lock().await.take() {
                            let _ = inc.file.flush().await;
                            let _ = inc.file.sync_all().await;
                            println!("agent: saved '{}' ({} bytes)", inc.name, inc.received);
                            let _ = ack
                                .send_text(
                                    serde_json::json!({
                                        "t": "file_done",
                                        "name": inc.name,
                                        "bytes": inc.received,
                                    })
                                    .to_string(),
                                )
                                .await;
                        }
                    }
                    _ => {}
                }
            } else {
                // Binary chunk: append to the active transfer.
                let mut guard = state.lock().await;
                if let Some(inc) = guard.as_mut() {
                    if inc.file.write_all(&msg.data).await.is_ok() {
                        inc.received += msg.data.len() as u64;
                    }
                }
            }
        })
    }));
}

// ---------------------------------------------------------------------------
// Chat (live text, both ways) — bridges the `chat` channel to the host app
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Bridges in-session chat between the WebRTC `chat` data channels and the host
/// app (terminal for the headless agent, a window for the desktop app). Incoming
/// messages are handed to `on_incoming`; [`ChatBridge::send`] delivers a reply to
/// every connected viewer.
#[derive(Clone)]
pub struct ChatBridge {
    channels: Arc<Mutex<Vec<Arc<RTCDataChannel>>>>,
    on_incoming: Arc<dyn Fn(String) + Send + Sync>,
}

impl ChatBridge {
    /// `on_incoming(text)` is called for each message a viewer sends.
    pub fn new(on_incoming: impl Fn(String) + Send + Sync + 'static) -> Self {
        Self {
            channels: Arc::new(Mutex::new(Vec::new())),
            on_incoming: Arc::new(on_incoming),
        }
    }

    /// Send a chat message to all connected viewers.
    pub async fn send(&self, text: &str) {
        let payload = serde_json::json!({ "t": "msg", "text": text, "ts": now_ms() }).to_string();
        // Snapshot the channels so we don't hold the lock across awaits.
        let chans: Vec<Arc<RTCDataChannel>> = match self.channels.lock() {
            Ok(c) => c.clone(),
            Err(_) => return,
        };
        for ch in chans {
            let _ = ch.send_text(payload.clone()).await;
        }
    }

    /// Attach a freshly-opened `chat` channel: forward its messages to
    /// `on_incoming` and keep it for outgoing replies. Called synchronously when
    /// the channel opens so no early message is missed.
    fn attach(&self, dc: Arc<RTCDataChannel>) {
        let on_incoming = self.on_incoming.clone();
        dc.on_message(Box::new(move |msg| {
            let on_incoming = on_incoming.clone();
            Box::pin(async move {
                if !msg.is_string {
                    return;
                }
                let text = String::from_utf8_lossy(&msg.data);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("t").and_then(|t| t.as_str()) == Some("msg") {
                        if let Some(body) = v.get("text").and_then(|t| t.as_str()) {
                            on_incoming(body.to_string());
                        }
                    }
                }
            })
        }));
        if let Ok(mut chans) = self.channels.lock() {
            chans.push(dc);
        }
    }
}

// ---------------------------------------------------------------------------
// Clipboard sync (both ways) — bridges the OS clipboard to the `clip` channel
// ---------------------------------------------------------------------------

/// Keeps the remote PC's clipboard in sync with the console over a `clip` data
/// channel. A background thread watches the OS clipboard and pushes changes to
/// viewers; inbound `{t:"clip",text}` messages set the OS clipboard. `last`
/// (the most recently synced text) suppresses echo loops in both directions.
#[derive(Clone)]
struct ClipSync {
    channels: Arc<Mutex<Vec<Arc<RTCDataChannel>>>>,
    last: Arc<Mutex<String>>,
}

impl ClipSync {
    /// Create the bridge and start watching the OS clipboard.
    fn new() -> Self {
        let me = Self {
            channels: Arc::new(Mutex::new(Vec::new())),
            last: Arc::new(Mutex::new(String::new())),
        };
        me.start_watch();
        me
    }

    /// Write text to the OS clipboard, recording it as `last` first so the watch
    /// thread doesn't bounce it straight back to the console.
    fn set_os_clipboard(&self, text: &str) {
        if let Ok(mut g) = self.last.lock() {
            *g = text.to_string();
        }
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(text.to_string());
            println!("agent: clipboard updated from console ({} chars)", text.len());
        }
    }

    /// Attach a freshly-opened `clip` channel: apply inbound clipboard text and
    /// keep the channel for outbound updates.
    fn attach(&self, dc: Arc<RTCDataChannel>) {
        let me = self.clone();
        dc.on_message(Box::new(move |msg| {
            let me = me.clone();
            Box::pin(async move {
                if !msg.is_string {
                    return;
                }
                let text = String::from_utf8_lossy(&msg.data);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("t").and_then(|t| t.as_str()) == Some("clip") {
                        if let Some(body) = v.get("text").and_then(|t| t.as_str()) {
                            me.set_os_clipboard(body);
                        }
                    }
                }
            })
        }));
        if let Ok(mut chans) = self.channels.lock() {
            chans.push(dc);
        }
    }

    /// Background watcher: poll the OS clipboard (`arboard` isn't `Send` across
    /// awaits, so it lives on its own thread) and forward changes to a tokio task
    /// that broadcasts them to all connected viewers.
    fn start_watch(&self) {
        let channels = self.channels.clone();
        let last = self.last.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        std::thread::spawn(move || {
            let mut cb = match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("agent: clipboard unavailable: {e}");
                    return;
                }
            };
            loop {
                if let Ok(text) = cb.get_text() {
                    let changed = match last.lock() {
                        Ok(mut g) if *g != text => {
                            *g = text.clone();
                            true
                        }
                        _ => false,
                    };
                    if changed && !text.is_empty() {
                        let _ = tx.send(text);
                    }
                }
                std::thread::sleep(StdDuration::from_millis(700));
            }
        });

        tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                let payload = serde_json::json!({ "t": "clip", "text": text }).to_string();
                let chans: Vec<Arc<RTCDataChannel>> = match channels.lock() {
                    Ok(c) => c.clone(),
                    Err(_) => continue,
                };
                for ch in chans {
                    let _ = ch.send_text(payload.clone()).await;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// The agent loop
// ---------------------------------------------------------------------------

/// Run the agent: capture the primary screen, register on the signaling
/// backend, and serve admitted viewers (H.264 screen video + inject their
/// input + files + chat). `consent` decides whether each requesting console is
/// admitted; `chat` bridges live text to the host app; `download` supplies a file
/// when the console requests one. Returns when the signaling socket closes.
pub async fn run(
    url: String,
    id: String,
    token: Option<String>,
    consent: ConsentFn,
    chat: ChatBridge,
    download: DownloadProvider,
) {
    // Per-monitor DPI aware so scrap capture sizes and Win32 monitor rects agree.
    winmon::make_dpi_aware();

    // Default to the primary screen; the console can switch via the input channel.
    let screens = enumerate_screens();
    let default_idx = screens.iter().position(|s| s.primary).unwrap_or(0);
    let init = screens.get(default_idx).copied().unwrap_or(ScreenInfo {
        ox: 0,
        oy: 0,
        width: 1920,
        height: 1080,
        primary: true,
    });

    // Capture thread publishes the latest downscaled RGB frame for the selected
    // screen; `sel` switches screens and `active` reports the live origin+size.
    let (tx, rx) = watch::channel(Frame::default());
    let (sel_tx, sel_rx) = watch::channel(default_idx);
    let (active_tx, active_rx) = watch::channel((init.ox, init.oy, init.width, init.height));
    thread::spawn(move || capture_loop(tx, sel_rx, active_tx));

    // One shared input injector.
    let enigo = Arc::new(Mutex::new(
        Enigo::new(&Settings::default()).expect("failed to init input injector"),
    ));

    // Two-way clipboard sync (watches the OS clipboard from here on).
    let clip = ClipSync::new();

    let signaling = match Signaling::connect(&url).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("agent: failed to connect to signaling backend: {e}");
            return;
        }
    };
    signaling.register("agent", &id, token.as_deref());
    println!("agent registered as '{id}' — waiting for a viewer...");

    // Per admitted connection: attach a screen video track + start encoding.
    let on_peer = make_on_peer(rx);

    // Per opened data channel: "input" -> inject events; "files" -> receive
    // files into ~/ArnaRemote/Incoming; "chat" -> bridge live text.
    let on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync> =
        Arc::new(move |dc| match dc.label() {
            "input" => {
                println!("agent: control channel open");
                let enigo = enigo.clone();
                let active = active_rx.clone();
                let select = sel_tx.clone();
                // Announce the available monitors so the console can offer a picker.
                let dc_open = dc.clone();
                dc.on_open(Box::new(move || {
                    let dc = dc_open.clone();
                    let announce = monitors_announce();
                    Box::pin(async move {
                        let _ = dc.send(&Bytes::from(announce)).await;
                    })
                }));
                dc.on_message(Box::new(move |msg| {
                    let enigo = enigo.clone();
                    let active = active.clone();
                    let select = select.clone();
                    let text = String::from_utf8_lossy(&msg.data).to_string();
                    Box::pin(async move {
                        handle_input(&text, &enigo, &active, &select);
                    })
                }));
            }
            "files" => handle_files_channel(dc, download.clone()),
            "chat" => {
                println!("agent: chat channel open");
                chat.attach(dc);
            }
            "clip" => {
                println!("agent: clipboard channel open");
                clip.attach(dc);
            }
            other => println!("agent: ignoring unknown channel '{other}'"),
        });

    if let Err(e) = p2p::answer_streaming(signaling, id, consent, on_peer, on_channel).await {
        eprintln!("agent error: {e}");
    }
}
