//! Arna agent — screen capture proof (Phase 1c, step 1).
//!
//! Grabs a few frames from the primary display, converts BGRA -> RGB, and
//! JPEG-encodes each one — proving the capture + encode pipeline before we wire
//! it onto the WebRTC data channel. Run it directly:  `cargo run -p arna-agent`.

use std::io::ErrorKind::WouldBlock;
use std::thread;
use std::time::{Duration, Instant};

use scrap::{Capturer, Display};

fn encode_jpeg(rgb: &[u8], w: u32, h: u32, quality: u8) -> Vec<u8> {
    let mut jpeg = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality);
    encoder
        .encode(rgb, w, h, image::ExtendedColorType::Rgb8)
        .expect("jpeg encode");
    jpeg
}

/// Convert a BGRA frame (with row stride padding) into tightly-packed RGB.
fn bgra_to_rgb(frame: &[u8], w: usize, h: usize) -> Vec<u8> {
    let stride = frame.len() / h;
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        let row = y * stride;
        for x in 0..w {
            let i = row + x * 4;
            rgb.push(frame[i + 2]); // R
            rgb.push(frame[i + 1]); // G
            rgb.push(frame[i]); // B
        }
    }
    rgb
}

fn main() {
    let display = Display::primary().expect("no primary display found");
    let mut capturer = Capturer::new(display).expect("failed to start screen capturer");
    let (w, h) = (capturer.width(), capturer.height());
    println!("agent: capturing primary display {w}x{h}");

    let mut captured = 0;
    let target = 5;
    while captured < target {
        match capturer.frame() {
            Ok(frame) => {
                let t = Instant::now();
                let rgb = bgra_to_rgb(&frame, w, h);
                let jpeg = encode_jpeg(&rgb, w as u32, h as u32, 60);
                captured += 1;
                println!(
                    "frame {captured}/{target}: {w}x{h} -> {} KB jpeg ({} ms)",
                    jpeg.len() / 1024,
                    t.elapsed().as_millis()
                );
            }
            Err(ref e) if e.kind() == WouldBlock => {
                // The frame isn't ready yet — wait a moment and retry.
                thread::sleep(Duration::from_millis(30));
            }
            Err(e) => panic!("capture error: {e}"),
        }
    }
    println!("capture + encode pipeline OK");
}
