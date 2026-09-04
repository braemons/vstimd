use std::sync::{Arc, Mutex};

use crate::log_buffer::LogBuffer;
use crate::render::RenderState;
use crate::render::backend::BackendData;
use crate::input::console_input::{InputState, vt_number_from_env};
use crate::render::frame_loop::{self, KeyOutcome};
use crate::system_info::{ClockSource, SystemInfo};
use crate::render::vk::VkContext;
use crate::render::RenderTarget;
use crate::render::{SceneRenderer, TextRenderer, UiRenderer};
use crate::render::render_frame;
use crate::timing::FrameTiming;
use crate::vtl_state::VtlState;
extern crate vtl;

use super::drm_display_guard::DrmDisplayGuard;
use super::drm_vblank::{DrmVblank, DrmVblankState, VkVblank};
use super::drm_virtual_terminal::DrmVtGuard;

// ── Public backend ────────────────────────────────────────────────────────────

pub struct DrmBackend {
    data: BackendData,
    log_buffer: LogBuffer,
}

impl DrmBackend {
    pub fn new(data: BackendData, log_buffer: LogBuffer) -> Self {
        Self { data, log_buffer }
    }
}

impl DrmBackend {
    pub fn run(self, on_ready: impl FnOnce()) {
        let data = DrmRenderLoopData::new(self.data, self.log_buffer);
        on_ready();
        data.run_loop();
    }
}

// ── DrmRenderLoopData ─────────────────────────────────────────────────────────

/// All data required to run one iteration of the DRM render loop.
///
/// Fields drop in declaration order: `rs` (Vulkan resources) before
/// `display_guard` (CRTC restore) before `vt_guard` (KD_TEXT restore), so
/// the VT is returned to text mode only after Vulkan has fully released the
/// display hardware.
struct DrmRenderLoopData {
    /// Pending screenshot, if any. On a bare-metal rig there is no window
    /// manager to ask for one, so this is the only way to capture the frame
    /// that is actually on the panel. Declared before `rs`: a pending capture
    /// owns a Vulkan buffer, so it must drop before `rs` tears the device down.
    shot: crate::render::Screenshotter,
    rs: RenderState,
    vtl: Option<Arc<Mutex<VtlState>>>,
    input: InputState,
    vblank: DrmVblankState,
    /// Holds the CRTC snapshot; dropped before `vt_guard` to restore the
    /// console framebuffer before KD_TEXT is re-enabled.
    #[allow(dead_code)]
    display_guard: Option<DrmDisplayGuard>,
    /// Activates the target VT and holds KD_GRAPHICS; dropped last so the
    /// terminal isn't returned to text mode until Vulkan teardown is complete.
    #[allow(dead_code)]
    vt_guard: DrmVtGuard,
    /// True while our VT is not the active one (we released the input grab).
    suspended: bool,
    /// Set in `new()` if `clock_pref` forced a clock source that turned out
    /// to be unavailable. Checked and consumed at the top of `run_loop`,
    /// which fails the same way a runtime clock loss does — this lets
    /// construction finish normally so `Self`'s Drop guards still restore
    /// the VT/CRTC, instead of `process::exit`-ing mid-construction.
    startup_clock_error: Option<String>,
}

fn check_device_permissions() {
    // Root can access all devices regardless of group membership.
    if unsafe { libc::getuid() } == 0 {
        return;
    }

    let mut missing: Vec<String> = Vec::new();

    let drm_ok = (0..8u32).any(|n| {
        let path = format!("/dev/dri/card{n}\0");
        unsafe {
            libc::access(
                path.as_ptr() as *const libc::c_char,
                libc::R_OK | libc::W_OK,
            ) == 0
        }
    });
    if !drm_ok {
        missing.push(
            "  /dev/dri/card* — add user to 'video' group:\n    sudo usermod -aG video $USER"
                .to_string(),
        );
    }

    let input_ok = unsafe {
        let grp = libc::getgrnam(c"input".as_ptr());
        if grp.is_null() {
            true
        } else {
            let input_gid = (*grp).gr_gid;
            if libc::getegid() == input_gid {
                true
            } else {
                let mut groups = vec![0u32; 64];
                let n = libc::getgroups(groups.len() as libc::c_int, groups.as_mut_ptr());
                n > 0 && groups[..n as usize].contains(&input_gid)
            }
        }
    };
    if !input_ok {
        missing.push(
            "  not in 'input' group — add user and log out/in:\n    sudo usermod -aG input $USER"
                .to_string(),
        );
    }

    if !missing.is_empty() {
        log::error!(
            "vstimd: missing device permissions — log out and back in after fixing:\n{}",
            missing.join("\n")
        );
        std::process::exit(1);
    }
}

impl DrmRenderLoopData {
    fn new(data: BackendData, log_buffer: LogBuffer) -> Self {
        let BackendData {
            scene,
            vtl,
            host_info,
            overlay_scale,
            display_pref,
            clock_pref,
            rig_config_path,
        } = data;
        check_device_permissions();

        // Snapshot display state first, while the current VT still has an
        // active CRTC mode.  On Jetson nvdisplay the VT switch deactivates
        // the CRTC (mode → None), so we must save state before switching.
        let display_guard = DrmDisplayGuard::acquire();

        // Open DRM vblank here, before the VT switch and before Vulkan init.
        // Both events clear the CRTC mode: the VT switch deactivates it on
        // nvdisplay, and VK_KHR_display acquiring DRM master also returns
        // mode → None.  wait_vblank is unprivileged so the fd stays valid
        // throughout the session.
        let drm_vblank = DrmVblank::open();

        // Now activate the target VT and set KD_GRAPHICS so the kernel stops
        // writing text over our framebuffer.
        let vt_guard = DrmVtGuard::acquire();

        // Initialise Vulkan — VK_KHR_display acquires DRM master internally.
        let (ctx, display_info, vk_display) = super::drm_init::init(display_pref);

        // Build scene + text sub-renderers first (before ctx moves).
        let storage_dir = scene.read().unwrap().runtime.storage_dir.clone();
        let scene_renderer = SceneRenderer::new(&ctx, scene);
        let text = TextRenderer::new(&ctx);

        ctx.set_debug_name(ctx.render_pass, "render_pass");
        ctx.set_debug_name(ctx.egui_render_pass, "egui_render_pass");
        for (i, frame) in ctx.frames.iter().enumerate() {
            ctx.set_debug_name(frame.command_buffer, &format!("frame[{i}]_cmd"));
        }
        for (i, img) in ctx.swapchain_images.iter().enumerate() {
            ctx.set_debug_name(*img, &format!("swapchain[{i}]"));
        }

        let ui = UiRenderer::new(&ctx, storage_dir, log_buffer, overlay_scale);

        let input = InputState::new(vt_number_from_env());

        // Resolve which clock source(s) to actually try, checking as late as
        // possible — right before the render loop starts, after all other
        // setup above — since auto-detection can't reliably predict every
        // GPU/driver combination (a clock that passes an earlier check can
        // still fail once the loop is running). `clock_pref` lets rig-config
        // or `--preferred-clock-source` pin a specific source instead of
        // probing, at the cost of a hard failure if that source isn't
        // available rather than a silent fallback.
        let make_vk_vblank = |ctx: &VkContext| {
            ctx.display_control
                .as_ref()
                .map(|loader| VkVblank::new(ctx.device.clone(), loader.clone(), vk_display))
        };
        let unavailable = |source: ClockSource| {
            format!(
                "clock source {:?} was requested but is not available on this hardware. \
                 Change `[display] clock` in {rig_config_path} (or drop --preferred-clock-source) \
                 to \"auto\" or a different source.",
                source.as_str()
            )
        };
        let (drm_vblank, vk_vblank, startup_clock_error) = match clock_pref {
            None => (
                drm_vblank.and_then(DrmVblank::validate),
                make_vk_vblank(&ctx),
                None,
            ),
            Some(ClockSource::DrmVblank) => match drm_vblank {
                Some(v) => (Some(v), None, None),
                None => (None, None, Some(unavailable(ClockSource::DrmVblank))),
            },
            Some(ClockSource::VkDisplayControl) => match make_vk_vblank(&ctx) {
                Some(v) => (None, Some(v), None),
                None => (None, None, Some(unavailable(ClockSource::VkDisplayControl))),
            },
            Some(ClockSource::PresentWait) => {
                if ctx.present_wait.is_some() {
                    (None, None, None)
                } else {
                    (None, None, Some(unavailable(ClockSource::PresentWait)))
                }
            }
            Some(ClockSource::GpuCompletion) => (None, None, None),
            Some(ClockSource::DisplayTiming) => (
                None,
                None,
                Some(format!(
                    "clock source \"display_timing\" is not implemented as a selectable clock \
                     in DRM mode. Change `[display] clock` in {rig_config_path} (or drop \
                     --preferred-clock-source) to \"auto\" or a different source."
                )),
            ),
        };
        let vblank = DrmVblankState::new(drm_vblank, vk_vblank);

        let system_info = SystemInfo {
            host: host_info,
            gpu_name: String::new(),
            backend: RenderTarget::Drm,
            supports_wireframe: ctx.supports_wireframe,
            clock_source: vblank.clock_source(ctx.present_wait.is_some()),
        };

        let rs = RenderState {
            scene_renderer,
            text,
            ui: Some(ui),
            timing: FrameTiming::new(display_info.refresh_hz),
            system_info,
            display_info,
            ctx,
        };

        Self {
            shot: crate::render::Screenshotter::new(),
            rs,
            vtl,
            input,
            vblank,
            display_guard,
            vt_guard,
            suspended: false,
            startup_clock_error,
        }
    }

    /// The session's timing/display guarantees are broken — the vblank clock
    /// died (or `clock_pref` forced a source unavailable at startup), or the
    /// CRTC's actual mode drifted away from what vstimd set (observed: an
    /// out-of-band modeset, e.g. from a DMCUB firmware fault, that Vulkan
    /// doesn't reliably surface — see `DrmDisplayGuard::check_mode`). For a
    /// stimulus-timing session, silently continuing on corrupted timing or a
    /// wrong display resolution is worse than stopping: nothing else would
    /// tell the experimenter or any data consumer. This requests a shutdown
    /// with a non-zero exit code; the caller must still `return` from
    /// `run_loop` so `self`'s Drop guards restore the VT/CRTC.
    fn fatal_display_error(reason: String) {
        log::error!(
            "vstimd: fatal display error — {reason}. Stimulus timing/output can no longer be \
             guaranteed; shutting down."
        );
        crate::process::shutdown::request_fatal(reason);
    }

    fn run_loop(mut self) {
        if let Some(reason) = self.startup_clock_error.take() {
            Self::fatal_display_error(reason);
            return;
        }

        let mut clock_logged = false;
        loop {
            if crate::process::shutdown::is_requested() {
                return;
            }

            // Periodically confirm the CRTC hasn't drifted away from the mode
            // vstimd set — cheap read-only ioctl, checked roughly once a
            // second rather than every frame to keep it off the hot path.
            // Skip frame 0: the CRTC modeset requested via
            // vkCreateDisplayPlaneSurfaceKHR doesn't necessarily commit at
            // surface-creation time — it was observed to only take effect
            // once the first frame is actually presented, so checking before
            // that happens is a false positive, not a real mismatch.
            if self.rs.timing.frame_index != 0
                && self.rs.timing.frame_index.is_multiple_of(60)
                && let Some(guard) = &self.display_guard
            {
                let expected = (self.rs.display_info.width_px, self.rs.display_info.height_px);
                if let Err(reason) = guard.check_mode(expected) {
                    Self::fatal_display_error(reason);
                    return;
                }
            }

            // Handle VT_PROCESS signals: release input grab when switching away,
            // re-acquire when switching back, so the other VT's session gets input.
            if self.vt_guard.release_requested() {
                self.input.suspend();
                self.vt_guard.allow_release();
                self.suspended = true;
                log::info!("vstimd: VT released — input suspended");
            }
            if self.vt_guard.acquire_requested() {
                self.vt_guard.confirm_acquire();
                self.input.resume();
                self.suspended = false;
                log::info!("vstimd: VT re-acquired — input resumed");
            }
            if self.suspended {
                std::thread::sleep(std::time::Duration::from_millis(16));
                continue;
            }

            // 1. Poll keyboard input (non-blocking libinput drain).
            let (app_keys, nav_events) = self.input.poll();
            for key in app_keys {
                // The VT guard holds the VT in VT_PROCESS mode, so a switch
                // has to go through it for the release handshake.
                match frame_loop::apply_app_key(key, &mut self.rs) {
                    KeyOutcome::SwitchVt(n) => self.vt_guard.switch_to(n),
                    KeyOutcome::Screenshot => self.shot.request(&crate::render::screenshot::default_dir()),
                    KeyOutcome::Handled => {}
                }
            }

            // 2. Build egui raw input (screen rect + libinput nav keys).
            let egui_raw_input = frame_loop::overlay_raw_input(&self.rs, nav_events);

            // 3. Block on vblank: DRM ioctl path blocks here directly; VK path
            //    collects the FIRST_PIXEL_OUT fence registered at end of last frame.
            //    When this returns, the previous frame is confirmed visible.
            let screen_clock = match self.vblank.wait() {
                Ok(t) => t,
                Err(reason) => {
                    Self::fatal_display_error(reason);
                    return;
                }
            };

            // The DRM clock's startup-race grace period (DRM_VBLANK_GRACE_ATTEMPTS
            // in drm_vblank.rs) can silently disable it mid-loop; resync so
            // system_info — and the one-time log line below — reflect the clock
            // actually in use rather than what was resolved at startup. This is
            // the same "don't silently degrade with no signal" concern e5dc6a5
            // fixed, scoped back down to just this one known-benign transition.
            let resolved_clock = self.vblank.clock_source(self.rs.ctx.present_wait.is_some());
            if resolved_clock != self.rs.system_info.clock_source {
                log::warn!(
                    "vstimd: vblank clock source changed: {} -> {}",
                    self.rs.system_info.clock_source.as_str(),
                    resolved_clock.as_str()
                );
                self.rs.system_info.clock_source = resolved_clock;
            }

            // Log the settled clock source once, after frame 1 (when the VK
            // fence has been collected for the first time).
            if !clock_logged && self.rs.timing.frame_index > 0 {
                clock_logged = true;
                log::info!(
                    "vstimd: vblank clock: {}",
                    self.rs.system_info.clock_source.as_str()
                );
            }

            // [A] Commit staged outputs; poll inputs; advance animations.
            frame_loop::advance_frame(self.vtl.as_ref(), &self.rs.scene_renderer.scene);

            // Register the FIRST_PIXEL_OUT fence for the frame we are about to
            // present.  The fence is collected at the top of the next iteration.
            // This two-phase_cycles register→collect pattern avoids double-blocking with
            // the FIFO vkAcquireNextImageKHR (which also syncs to the display).
            if let Err(reason) = self.vblank.register(self.rs.timing.frame_index as u64) {
                Self::fatal_display_error(reason);
                return;
            }

            // 4. Render: build overlay UI, tessellate scene, record Vulkan
            //    commands, submit to GPU, present to display.
            //    The frame prepared here will become visible at the next vblank.
            // Split the borrow so the readback target (from `shot`) and the
            // render state can be held at once.
            let Self { rs, shot, vtl, .. } = &mut self;
            let readback = shot.begin(&rs.ctx);
            render_frame(rs, screen_clock, egui_raw_input, vtl.as_deref(), readback);
            shot.finish(&rs.ctx);

        }
        // When the loop exits, `self` is consumed and fields drop in
        // declaration order: `rs` (Vulkan teardown) → `input` → `vblank`
        // → `display_guard` (CRTC restore) → `vt_guard` (KD_TEXT restore).
        // The CRTC restore therefore fires after Vulkan has released DRM master.
    }
}
