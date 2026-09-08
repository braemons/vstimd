//! The renderer with no display: the ZMQ server, the VTL, and a frame clock.
//!
//! **Not a stub.** It runs the same per-frame drain as a real backend, through
//! the same [`frame_loop::advance_frame`], because it is what an end-to-end test
//! and a bench rig run — and a headless loop that quietly did something else
//! would make every test of the event stream a test of a code path that never
//! ships. It kept its own copy of the drain once, and the copy had already
//! drifted: no VTL edges published, and every command stamped frame 0.

use crate::render::backend::BackendData;
use crate::render::frame_loop;

pub struct NullBackend {
    data: BackendData,
}

impl NullBackend {
    pub fn new(data: BackendData) -> Self {
        Self { data }
    }

    pub fn run(self, on_ready: impl FnOnce()) {
        let BackendData { scene, vtl, .. } = self.data;

        log::info!("vstimd: null renderer — ZMQ server + animation loop running, no display");
        on_ready();

        let frame_period = {
            let s = scene.read().unwrap();
            std::time::Duration::from_secs_f32(1.0 / s.runtime.frame_rate_hz)
        };
        loop {
            if crate::process::shutdown::is_requested() {
                break;
            }
            let t0 = std::time::Instant::now();

            // Same order as a real backend: drain and advance first, then the
            // bookkeeping a display backend does while it tessellates. The
            // deferred flip lands here rather than before the drain for that
            // reason — `render_frame` applies it under the tessellation lock,
            // which is after this point, not before.
            frame_loop::advance_frame(vtl.as_ref(), &scene);

            let mut s = scene.write().unwrap();
            if s.runtime.pending_flip {
                s.apply_flip();
            }
            s.runtime.frame_count += 1;
            let _ = s.runtime.frame_notifier.send(s.runtime.frame_count);
            // The frame a command applied from here on will first appear on.
            // There is no swapchain and so no present id: the frame counter is
            // the only clock, and it is the one the events carry.
            s.runtime.next_render_frame = s.runtime.frame_count + 1;
            drop(s);

            if let Some(remaining) = frame_period.checked_sub(t0.elapsed()) {
                std::thread::sleep(remaining);
            }
        }
    }
}
