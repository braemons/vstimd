//! Does evdi expose a real DRM vblank clock? A Wayland/X11 compositor
//! driving a DisplayLink output presumably syncs to *something* — this
//! checks whether `DRM_IOCTL_WAIT_VBLANK` (or page-flip completion events)
//! actually work against evdi's CRTC, the same ioctl vstimd's real DRM
//! backend uses (see `render/drm/drm_vblank.rs`).

use drm::Device as DrmDevice;
use drm::buffer::DrmFourcc;
use drm::control::Device as CtrlDevice;
use drm::control::{Event, PageFlipTarget};

use vstimd::render::evdi::find_connected_evdi;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let node = find_connected_evdi().expect("no connected evdi node");
    println!("using {}", node.path);
    let card = node.card;

    let conn = card.get_connector(node.connector, false).unwrap();
    let mode = *conn.modes().first().expect("no modes");
    let (width_px, height_px) = mode.size();
    println!("mode {:?} {}x{} vrefresh={}", mode.name(), width_px, height_px, mode.vrefresh());

    let res = card.resource_handles().unwrap();
    let crtc_pipe_and_handle = conn
        .current_encoder()
        .and_then(|enc_h| card.get_encoder(enc_h).ok())
        .and_then(|enc| enc.crtc())
        .and_then(|crtc_h| res.crtcs().iter().position(|&c| c == crtc_h).map(|p| (p, crtc_h)))
        .or_else(|| {
            conn.encoders().iter().find_map(|&enc_h| {
                let enc = card.get_encoder(enc_h).ok()?;
                let crtc_h = res.filter_crtcs(enc.possible_crtcs()).into_iter().next()?;
                let pipe = res.crtcs().iter().position(|&c| c == crtc_h)?;
                Some((pipe, crtc_h))
            })
        })
        .expect("no usable CRTC");
    let (crtc_pipe, crtc_h) = crtc_pipe_and_handle;
    println!("CRTC {crtc_h:?} pipe={crtc_pipe}");

    // Establish the mode first — wait_vblank / page_flip need an active CRTC.
    let mut dumb = card
        .create_dumb_buffer((width_px as u32, height_px as u32), DrmFourcc::Xrgb8888, 32)
        .unwrap();
    {
        let mut map = card.map_dumb_buffer(&mut dumb).unwrap();
        map.fill(0);
    }
    let fb1 = card.add_framebuffer(&dumb, 24, 32).unwrap();
    card.set_crtc(crtc_h, Some(fb1), (0, 0), &[node.connector], Some(mode))
        .expect("initial set_crtc");
    println!("initial set_crtc ok, mode established");

    // 1) Try the blocking legacy vblank ioctl, several times, timing each call.
    println!("\n--- DRM_IOCTL_WAIT_VBLANK ---");
    for i in 0..5 {
        let start = std::time::Instant::now();
        let result = DrmDevice::wait_vblank(
            &card,
            drm::VblankWaitTarget::Relative(1),
            drm::VblankWaitFlags::empty(),
            crtc_pipe as u32,
            0,
        );
        let elapsed = start.elapsed();
        match result {
            Ok(reply) => println!("  [{i}] ok, took {elapsed:?}, reply={reply:?}"),
            Err(e) => {
                println!("  [{i}] FAILED: {e} (took {elapsed:?})");
                break;
            }
        }
    }

    // 2) Try page_flip with DRM_MODE_PAGE_FLIP_EVENT and see whether a
    // completion event actually arrives on the fd.
    println!("\n--- page_flip + event ---");
    let mut dumb2 = card
        .create_dumb_buffer((width_px as u32, height_px as u32), DrmFourcc::Xrgb8888, 32)
        .unwrap();
    {
        let mut map = card.map_dumb_buffer(&mut dumb2).unwrap();
        map.fill(0xFF);
    }
    let fb2 = card.add_framebuffer(&dumb2, 24, 32).unwrap();

    for i in 0..5 {
        let target_fb = if i % 2 == 0 { fb2 } else { fb1 };
        let start = std::time::Instant::now();
        let flip_result = card.page_flip(
            crtc_h,
            target_fb,
            drm::control::PageFlipFlags::EVENT,
            None::<PageFlipTarget>,
        );
        if let Err(e) = flip_result {
            println!("  [{i}] page_flip call FAILED: {e}");
            break;
        }
        match card.receive_events() {
            Ok(events) => {
                let mut got = false;
                for ev in events {
                    match ev {
                        Event::PageFlip(pf) => {
                            println!(
                                "  [{i}] PageFlip event: frame={} duration={:?} (call->event {:?})",
                                pf.frame, pf.duration, start.elapsed()
                            );
                            got = true;
                        }
                        Event::Vblank(v) => println!("  [{i}] Vblank event: frame={}", v.frame),
                        Event::Unknown(_) => println!("  [{i}] unknown event"),
                    }
                }
                if !got {
                    println!("  [{i}] receive_events returned but no PageFlip event in it");
                }
            }
            Err(e) => println!("  [{i}] receive_events FAILED: {e}"),
        }
    }

    let _ = card.destroy_framebuffer(fb1);
    let _ = card.destroy_framebuffer(fb2);
}
