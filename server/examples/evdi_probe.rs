//! Hardware smoke test: find a connected evdi node, push one solid-color
//! frame to it via raw KMS (dumb buffer + `set_crtc`), no compositor and no
//! Vulkan. Confirms the KMS plumbing works before building the real
//! Vulkan-backed render loop on top of it.
//!
//! `cargo run --release --example evdi_probe -- [seconds-to-hold]`

use drm::buffer::{Buffer, DrmFourcc};
use drm::control::Device as CtrlDevice;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let hold_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let node = match vstimd::render::evdi::find_connected_evdi() {
        Some(n) => n,
        None => {
            eprintln!("no connected evdi node found");
            std::process::exit(1);
        }
    };
    println!("found connected evdi node: {} connector={:?}", node.path, node.connector);

    let card = &node.card;

    let conn = card
        .get_connector(node.connector, false)
        .expect("get_connector");
    let mode = *conn.modes().first().expect("connector reports no modes");
    let (width, height) = mode.size();
    println!(
        "using mode {:?} {}x{} @ {} (encoders: {})",
        mode.name(),
        width,
        height,
        mode.vrefresh(),
        conn.encoders().len()
    );

    let res = card.resource_handles().expect("resource_handles");

    // Prefer the connector's already-active encoder/CRTC if it has one;
    // otherwise pick the first encoder and the first CRTC it allows.
    let crtc_handle = conn
        .current_encoder()
        .and_then(|enc_h| card.get_encoder(enc_h).ok())
        .and_then(|enc| enc.crtc())
        .or_else(|| {
            conn.encoders().iter().find_map(|&enc_h| {
                let enc = card.get_encoder(enc_h).ok()?;
                res.filter_crtcs(enc.possible_crtcs()).into_iter().next()
            })
        })
        .expect("no usable CRTC found for this connector");
    println!("using CRTC {crtc_handle:?}");

    let mut dumb = card
        .create_dumb_buffer((width as u32, height as u32), DrmFourcc::Xrgb8888, 32)
        .expect("create_dumb_buffer");

    {
        let pitch = dumb.pitch() as usize;
        let mut map = card.map_dumb_buffer(&mut dumb).expect("map_dumb_buffer");
        // XRGB8888 little-endian in memory: B, G, R, X. Checkerboard of
        // magenta/black in 64px squares so it's obvious on the physical
        // screen and not mistaken for a stale/garbage framebuffer.
        for y in 0..height as usize {
            for x in 0..width as usize {
                let i = (y * pitch) + x * 4;
                let on = ((x / 64) + (y / 64)) % 2 == 0;
                let (b, g, r) = if on { (255u8, 0u8, 255u8) } else { (0, 0, 0) };
                map[i] = b;
                map[i + 1] = g;
                map[i + 2] = r;
                map[i + 3] = 0;
            }
        }
    }

    let fb = card.add_framebuffer(&dumb, 24, 32).expect("add_framebuffer");

    card.set_crtc(crtc_handle, Some(fb), (0, 0), &[node.connector], Some(mode))
        .expect("set_crtc");

    println!("set_crtc ok — holding for {hold_secs}s, check the physical screen");
    std::thread::sleep(std::time::Duration::from_secs(hold_secs));

    let _ = card.destroy_framebuffer(fb);
}
