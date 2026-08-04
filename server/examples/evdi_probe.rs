//! Hardware smoke test: find a connected evdi node and present a few
//! seconds of an animated pattern through `render::evdi::EvdiOutput`
//! (double-buffered dumb buffers, `set_crtc` per frame) — no Vulkan, no
//! compositor. Confirms the KMS presentation path before wiring in the
//! real Vulkan render loop.
//!
//! `cargo run --release --example evdi_probe -- [seconds-to-hold] [fps]`

use vstimd::render::evdi::{EvdiOutput, find_connected_evdi};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut args = std::env::args().skip(1);
    let hold_secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let fps: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);

    let node = match find_connected_evdi() {
        Some(n) => n,
        None => {
            eprintln!("no connected evdi node found");
            std::process::exit(1);
        }
    };
    println!("found connected evdi node: {} connector={:?}", node.path, node.connector);

    let mut out = EvdiOutput::new(node).expect("EvdiOutput::new");
    println!(
        "output ready: {}x{} pitch={}",
        out.width,
        out.height,
        out.pitch()
    );

    let pitch = out.pitch();
    let (width, height) = (out.width as usize, out.height as usize);
    let mut frame = vec![0u8; pitch * height];

    let frame_count = hold_secs * fps;
    let frame_dur = std::time::Duration::from_millis(1000 / fps.max(1));
    let start = std::time::Instant::now();

    for i in 0..frame_count {
        // Scrolling vertical color bars, ~64px wide, offset by frame index
        // — moving content makes it obvious each `present()` actually
        // reached the screen rather than the first frame just sticking.
        let shift = (i as usize * 8) % width.max(1);
        for y in 0..height {
            for x in 0..width {
                let band = ((x + shift) / 64) % 3;
                let (b, g, r) = match band {
                    0 => (255u8, 0u8, 0u8),
                    1 => (0, 255, 0),
                    _ => (0, 0, 255),
                };
                let o = y * pitch + x * 4;
                frame[o] = b;
                frame[o + 1] = g;
                frame[o + 2] = r;
                frame[o + 3] = 0;
            }
        }
        // Collect the previous flip, then queue this one — same pipelined
        // order the real backend uses.
        out.wait_flip().expect("wait_flip");
        out.submit(&frame).expect("submit");

        let target = frame_dur * (i as u32 + 1);
        let elapsed = start.elapsed();
        if target > elapsed {
            std::thread::sleep(target - elapsed);
        }
    }

    let elapsed = start.elapsed();
    println!(
        "presented {frame_count} frames in {:.2}s ({:.1} fps achieved)",
        elapsed.as_secs_f64(),
        frame_count as f64 / elapsed.as_secs_f64()
    );
}
