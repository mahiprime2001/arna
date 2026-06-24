//! Arna agent — screen capture + WebRTC streaming (Phase 1c).
//!
//! Captures the primary display, JPEG-encodes downscaled frames, and streams
//! them to a viewer over a WebRTC data channel.
//!
//!   # backend:  cargo run --manifest-path backend/Cargo.toml
//!   # agent:    cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
//!   # console:  cd console && npm install && npm run dev  ->  http://localhost:4310
//!
//! (Run the agent with --release for smooth capture.)

use std::io::ErrorKind::WouldBlock;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use arna_core::webrtc::data_channel::RTCDataChannel;
use arna_core::{p2p, Signaling};
use bytes::Bytes;
use scrap::{Capturer, Display};
use tokio::sync::watch;

/// Downscale the screen to this width before encoding (keeps frames small
/// enough for a data channel and cheap to encode).
const TARGET_WIDTH: u32 = 960;
const JPEG_QUALITY: u8 = 45;

/// Convert a BGRA frame (with row stride padding) into tightly-packed RGB.
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

/// Continuously capture + encode the primary display into `tx` (latest frame).
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
                    break; // no receivers left
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

    // Capture runs on its own thread (the capturer isn't Send); it publishes the
    // latest JPEG frame through a watch channel.
    let (tx, rx) = watch::channel(Vec::<u8>::new());
    thread::spawn(move || capture_loop(tx));

    let signaling = Signaling::connect(&url)
        .await
        .expect("failed to connect to signaling backend");
    signaling.register("agent", &id);
    println!("agent registered as '{id}' — waiting for a viewer...");

    // When a viewer's data channel opens, stream new frames onto it.
    let on_channel: Arc<dyn Fn(Arc<RTCDataChannel>) + Send + Sync> = Arc::new(move |dc| {
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
                if sent.is_multiple_of(30) {
                    println!("agent: streamed {sent} frames");
                }
            }
        });
    });

    if let Err(e) = p2p::answer_streaming(signaling, id.clone(), on_channel).await {
        eprintln!("agent error: {e}");
    }
}
