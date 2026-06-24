//! Arna agent — screen capture + WebRTC streaming + input injection (Phase 2).
//!
//! Streams the primary display to a viewer over a "screen" data channel, and
//! injects the mouse/keyboard events the viewer sends back over an "input"
//! channel (so you can actually drive the machine).
//!
//!   # backend:  cargo run --manifest-path backend/Cargo.toml
//!   # agent:    cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
//!   # console:  cd console && npm install && npm run dev  ->  http://localhost:4310
//!
//! (Run the agent with --release for smooth capture.)

use std::io::ErrorKind::WouldBlock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use arna_core::webrtc::data_channel::RTCDataChannel;
use arna_core::{p2p, Signaling};
use bytes::Bytes;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use scrap::{Capturer, Display};
use tokio::sync::watch;

const TARGET_WIDTH: u32 = 960;
const JPEG_QUALITY: u8 = 45;

// ---------------------------------------------------------------------------
// Screen capture
// ---------------------------------------------------------------------------

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

fn encode_frame(frame: &[u8], w: usize, h: usize) -> Vec<u8> {
    let rgb = bgra_to_rgb(frame, w, h);
    let img = image::RgbImage::from_raw(w as u32, h as u32, rgb).expect("rgb buffer");
    let scaled = if w as u32 > TARGET_WIDTH {
        let target_h = (h as u32 * TARGET_WIDTH) / w as u32;
        image::imageops::resize(
            &img,
            TARGET_WIDTH,
            target_h,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };
    let (sw, sh) = scaled.dimensions();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode(scaled.as_raw(), sw, sh, image::ExtendedColorType::Rgb8)
        .expect("jpeg encode");
    jpeg
}

fn capture_loop(tx: watch::Sender<Vec<u8>>) {
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
    println!("agent: capturing primary display {w}x{h}");

    loop {
        match capturer.frame() {
            Ok(frame) => {
                let jpeg = encode_frame(&frame, w, h);
                if tx.send(jpeg).is_err() {
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

// ---------------------------------------------------------------------------

fn primary_size() -> (i32, i32) {
    match Display::primary() {
        Ok(d) => (d.width() as i32, d.height() as i32),
        Err(_) => (1920, 1080),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "ws://127.0.0.1:8081/ws".to_string());
    let id = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "agent-1".to_string());

    let (screen_w, screen_h) = primary_size();

    // Capture thread publishes the latest JPEG frame.
    let (tx, rx) = watch::channel(Vec::<u8>::new());
    thread::spawn(move || capture_loop(tx));

    // One shared input injector.
    let enigo = Arc::new(Mutex::new(
        Enigo::new(&Settings::default()).expect("failed to init input injector"),
    ));

    let signaling = Signaling::connect(&url)
        .await
        .expect("failed to connect to signaling backend");
    signaling.register("agent", &id);
    println!("agent registered as '{id}' — waiting for a viewer...");

    // Each opened data channel: "screen" -> we stream frames; "input" -> we
    // inject the events the viewer sends.
    let on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync> =
        Arc::new(move |dc| match dc.label() {
            "screen" => {
                let mut rx = rx.clone();
                tokio::spawn(async move {
                    let mut sent: u64 = 0;
                    loop {
                        if rx.changed().await.is_err() {
                            break;
                        }
                        let frame = rx.borrow().clone();
                        if frame.is_empty() {
                            continue;
                        }
                        if dc.send(&Bytes::from(frame)).await.is_err() {
                            break;
                        }
                        sent += 1;
                        if sent.is_multiple_of(60) {
                            println!("agent: streamed {sent} frames");
                        }
                    }
                });
            }
            "input" => {
                println!("agent: control channel open");
                let enigo = enigo.clone();
                dc.on_message(Box::new(move |msg| {
                    let enigo = enigo.clone();
                    let text = String::from_utf8_lossy(&msg.data).to_string();
                    Box::pin(async move {
                        handle_input(&text, &enigo, screen_w, screen_h);
                    })
                }));
            }
            other => println!("agent: ignoring unknown channel '{other}'"),
        });

    if let Err(e) = p2p::answer_streaming(signaling, id.clone(), on_channel).await {
        eprintln!("agent error: {e}");
    }
}
