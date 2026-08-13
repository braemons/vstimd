use std::ffi::OsStr;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Instant;

use drm::Device as DrmDevice;
use drm::control::Device as ControlDevice;

use crate::render::system_info::ClockSource;

struct Card(std::fs::File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl ControlDevice for Card {}

pub struct DrmVblank {
    card: Card,
    path: String,
    crtc_pipe: u32,
}

impl DrmVblank {
    /// Iterate /dev/dri/card* and return a handle bound to the first CRTC that
    /// is actively driving a display (mode set). Returns `None` if none found.
    ///
    /// Skips `evdi`-driven nodes (DisplayLink docks): an evdi node enumerates
    /// before the real display controller (`vc4-drm` is `card5` on a Pi 5,
    /// evdi nodes are lower-numbered) and can still show an active CRTC/mode
    /// left over from a prior `--evdi` run, which would otherwise get bound
    /// here instead of the real display's CRTC — see `DrmDisplayGuard::acquire`,
    /// which had (and fixes) the identical bug.
    ///
    /// This only checks that a CRTC has a mode set — it does *not* confirm
    /// `DRM_IOCTL_WAIT_VBLANK` actually works. Call `validate()` as late as
    /// possible — after the rest of `DrmRenderLoopData::new()`'s setup has
    /// run, right before the render loop starts — to confirm that (see
    /// `validate` for why the check can't happen here).
    pub fn open() -> Option<Self> {
        for n in 0..8u8 {
            let path = format!("/dev/dri/card{n}");
            let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
            else {
                continue;
            };
            let card = Card(file);

            let Ok(driver) = card.get_driver() else {
                continue;
            };
            if driver.name() == OsStr::new("evdi") {
                continue;
            }

            // Release master immediately. Opening with O_RDWR automatically
            // grants DRM master when no other fd holds it (which is the case
            // here: DrmDisplayGuard already released master). If we keep master,
            // VK_KHR_display cannot acquire it during swapchain creation.
            // wait_vblank is an unprivileged ioctl — no master required.
            if let Err(err) = DrmDevice::release_master_lock(&card) {
                log::warn!("vstimd: failed to release DRM master for {path}: {err}");
            }

            let Ok(res) = card.resource_handles() else {
                continue;
            };
            for (pipe, &crtc_handle) in res.crtcs().iter().enumerate() {
                let Ok(crtc) = card.get_crtc(crtc_handle) else {
                    continue;
                };
                if crtc.mode().is_some() {
                    return Some(Self {
                        card,
                        path,
                        crtc_pipe: pipe as u32,
                    });
                }
            }
        }
        log::warn!(
            "vstimd: no active DRM CRTC found for vblank — using VK_EXT_display_control or GPU-completion time"
        );
        None
    }

    /// Confirm the legacy vblank ioctl works right now, on this CRTC. Call
    /// once, as late as possible — after the rest of
    /// `DrmRenderLoopData::new()`'s setup has run, immediately before the
    /// render loop starts.
    ///
    /// This is a best-effort check, not a guarantee. On AMD RENOIR (amdgpu
    /// DC), `DRM_IOCTL_WAIT_VBLANK` has been observed to succeed when probed
    /// and then fail with EINVAL on the very next call once the render loop
    /// starts — the exact trigger in vstimd's own setup hasn't been pinned
    /// down (an earlier theory blaming libinput's seat acquisition doesn't
    /// hold up: the same code path runs fine on other machines), and it
    /// correlates with a DMCUB (AMD Display Core firmware) fault confirmed
    /// to independently and intermittently hit this same hardware outside of
    /// vstimd too (it has also crashed GDM's own modeset) — i.e. this is
    /// most likely external to vstimd's code. Validating as late as possible
    /// only narrows the window between "checked" and "used"; it can't rule
    /// out a fault landing in that remaining gap. A clock that still fails
    /// mid-session despite passing this check is caught — and treated as
    /// fatal — by `DrmVblankState::wait`.
    ///
    /// Consumes `self`; returns `None` (dropping the DRM fd) if the ioctl
    /// doesn't work right now, so the caller falls through to
    /// `VK_EXT_display_control` instead of committing to a clock that's
    /// already broken.
    pub fn validate(self) -> Option<Self> {
        if self.wait().is_some() {
            log::info!("vstimd: DRM vblank: {} crtc[{}]", self.path, self.crtc_pipe);
            Some(self)
        } else {
            log::warn!(
                "vstimd: DRM_IOCTL_WAIT_VBLANK on {} crtc[{}] failed right after setup — \
                 falling back to VK_EXT_display_control",
                self.path,
                self.crtc_pipe
            );
            None
        }
    }

    /// Block until the next vblank on the selected CRTC.
    /// Returns an `Instant` captured immediately after the kernel unblocks.
    pub fn wait(&self) -> Option<Instant> {
        match DrmDevice::wait_vblank(
            &self.card,
            drm::VblankWaitTarget::Relative(1),
            drm::VblankWaitFlags::empty(),
            self.crtc_pipe,
            0,
        ) {
            Ok(_) => Some(Instant::now()),
            Err(err) => {
                log::warn!(
                    "vstimd: DRM wait_vblank failed on CRTC {}: {err}",
                    self.crtc_pipe
                );
                None
            }
        }
    }
}

// ── VkVblank ─────────────────────────────────────────────────────────────────

/// Vblank clock using `VK_EXT_display_control`.
///
/// `vkRegisterDisplayEventEXT` creates a one-shot fence that fires on the
/// display's first-pixel-out event (≈ vblank).  This is the fallback when
/// the legacy `DRM_IOCTL_WAIT_VBLANK` ioctl is not supported by the driver
/// (e.g. NVIDIA Tegra nvdisplay).
///
/// # Two-phase usage (avoids double-blocking with FIFO acquire)
///
/// With `VK_PRESENT_MODE_FIFO_KHR`, `vkAcquireNextImageKHR` already blocks at
/// the display vblank boundary.  If we also block on `FIRST_PIXEL_OUT` *before*
/// the acquire the loop runs at half the refresh rate.
///
/// The fix: **register** the fence just before render/present; **collect** it at
/// the very top of the *next* iteration before acquire.  The collect blocks for
/// the remaining ≈7 ms until FIRST_PIXEL_OUT fires, then acquire sees a free
/// image and returns immediately.
pub struct VkVblank {
    device: ash::Device,
    loader: ash::ext::display_control::Device,
    display: ash::vk::DisplayKHR,
}

impl VkVblank {
    pub fn new(
        device: ash::Device,
        loader: ash::ext::display_control::Device,
        display: ash::vk::DisplayKHR,
    ) -> Self {
        Self { device, loader, display }
    }

    /// Register a FIRST_PIXEL_OUT event and return the one-shot fence.
    /// Returns `None` on error.
    pub fn register(&self) -> Option<ash::vk::Fence> {
        let event_info = ash::vk::DisplayEventInfoEXT::default()
            .display_event(ash::vk::DisplayEventTypeEXT::FIRST_PIXEL_OUT);
        let mut fence = ash::vk::Fence::null();
        let result = unsafe {
            (self.loader.fp().register_display_event_ext)(
                self.loader.device(),
                self.display,
                &event_info as *const _,
                std::ptr::null(),
                &mut fence,
            )
        };
        if result != ash::vk::Result::SUCCESS {
            log::warn!("vstimd: vkRegisterDisplayEventEXT failed: {result:?}");
            return None;
        }
        Some(fence)
    }

    /// Wait for a previously registered fence and return the timestamp.
    /// Destroys the fence regardless of outcome.
    /// Returns `None` on error (caller should disable and fall back).
    pub fn collect(&self, fence: ash::vk::Fence) -> Option<Instant> {
        let wait_result = unsafe { self.device.wait_for_fences(&[fence], true, u64::MAX) };
        let t = Instant::now();
        unsafe { self.device.destroy_fence(fence, None) };
        wait_result.ok()?;
        Some(t)
    }
}

// ── DrmVblankState ────────────────────────────────────────────────────────────

/// Number of legacy `DRM_IOCTL_WAIT_VBLANK` attempts (on `DrmVblank`'s own
/// fd — separate from the fd Vulkan/`VK_KHR_display` uses for atomic
/// commits) within which a single `EINVAL` is tolerated as a startup
/// transient instead of fatal.
///
/// Root-caused via `strace` on a Raspberry Pi 5 (vc4/v3d): the first two
/// `wait_vblank` calls succeed against whatever was on screen before vstimd
/// started (vblank IRQs fire regardless of content); the instant Vulkan's
/// first real `DRM_IOCTL_MODE_ATOMIC` page-flip lands on its own fd — ~130µs
/// later — the very next `wait_vblank` call on this fd fails `EINVAL` once,
/// then the fallback clock works normally for the rest of the session. This
/// is a legacy-vblank-vs-atomic-commit race across file descriptors in the
/// vc4 kernel driver, reproduced 3/3 times, always within the first few
/// calls — not the AMD RENOIR DMCUB scenario `validate()`/fail-fast (see its
/// doc comment) was written for, where a *later*, less predictable failure
/// is a real sign timing can no longer be trusted. 5 gives headroom over the
/// observed 3rd-attempt failure without extending grace deep enough into a
/// session to blur that distinction.
const DRM_VBLANK_GRACE_ATTEMPTS: u32 = 5;

/// Owns both vblank clock sources and the pending FIRST_PIXEL_OUT fence,
/// collapsing the three separate fields and two methods that previously lived
/// in `DrmRenderState`.
pub struct DrmVblankState {
    drm: Option<DrmVblank>,
    vk: Option<VkVblank>,
    /// Fence registered at end of previous frame; collected at start of next.
    /// Only ever `Some` while `vk` is also `Some` (set in `register`).
    pending_fence: Option<ash::vk::Fence>,
    /// Count of `wait()` calls that reached the DRM branch, success or not.
    /// Compared against `DRM_VBLANK_GRACE_ATTEMPTS` to tell the startup race
    /// apart from a genuine later clock loss.
    drm_wait_attempts: u32,
}

impl DrmVblankState {
    pub fn new(drm: Option<DrmVblank>, vk: Option<VkVblank>) -> Self {
        Self { drm, vk, pending_fence: None, drm_wait_attempts: 0 }
    }

    pub fn clock_source(&self, has_present_wait: bool) -> ClockSource {
        if self.drm.is_some() {
            ClockSource::DrmVblank
        } else if self.vk.is_some() {
            ClockSource::VkDisplayControl
        } else if has_present_wait {
            ClockSource::PresentWait
        } else {
            ClockSource::GpuCompletion
        }
    }

    /// Block until the next vblank (DRM path) or collect the pending
    /// FIRST_PIXEL_OUT fence registered at the end of the previous frame
    /// (VK path). Returns `Ok(None)` on frame 0, before a fence exists yet.
    ///
    /// The clock source is fixed once at startup — `DrmVblank::open()` only
    /// selects a CRTC after confirming the ioctl actually works, so `self.drm`
    /// being `Some` here means it was already verified. A failure within
    /// `DRM_VBLANK_GRACE_ATTEMPTS` is the known vc4 startup race (see its doc
    /// comment) and falls through to whatever fallback clock is available,
    /// same as before `e5dc6a5` — but only once, and only this early. Any
    /// later failure means the previously-working clock just died at
    /// runtime: for a stimulus-timing session that's not something to
    /// silently paper over with a worse fallback, so the caller treats it as
    /// fatal.
    pub fn wait(&mut self) -> Result<Option<Instant>, String> {
        if let Some(vblank) = self.drm.as_ref() {
            self.drm_wait_attempts += 1;
            match vblank.wait() {
                Some(t) => return Ok(Some(t)),
                None if self.drm_wait_attempts <= DRM_VBLANK_GRACE_ATTEMPTS => {
                    log::warn!(
                        "vstimd: DRM_IOCTL_WAIT_VBLANK failed on CRTC {} on attempt {} — within \
                         the startup grace window, treating as the known vc4 atomic-commit race \
                         rather than a real clock failure; disabling DRM vblank clock for the \
                         rest of this session",
                        vblank.crtc_pipe,
                        self.drm_wait_attempts
                    );
                    self.drm = None;
                    // Fall through to the VK path below for this frame. No fence is
                    // pending yet (register() no-ops while self.drm was Some), so
                    // this frame returns Ok(None) — same as frame 0/1 — and the VK
                    // path (if available) takes over starting next frame.
                }
                None => {
                    return Err(format!(
                        "DRM_IOCTL_WAIT_VBLANK failed on CRTC {} after previously succeeding",
                        vblank.crtc_pipe
                    ));
                }
            }
        }
        // VK path: collect the fence registered at the end of the previous frame.
        // On frame 0 (and frame 1, since register() skips frame 0) there is no
        // pending fence yet; that's expected, not a failure.
        if let Some(fence) = self.pending_fence.take() {
            let vblank = self
                .vk
                .as_ref()
                .expect("pending_fence is only set while vk is Some");
            return match vblank.collect(fence) {
                Some(t) => Ok(Some(t)),
                None => Err(
                    "VK_EXT_display_control fence wait failed after previously succeeding"
                        .to_string(),
                ),
            };
        }
        Ok(None)
    }

    /// Register a FIRST_PIXEL_OUT fence for collection at the top of the next
    /// frame.  No-op on the DRM path (DRM uses a blocking ioctl instead) and
    /// on frame 0 (driver returns ERROR_UNKNOWN on Tegra before first present).
    pub fn register(&mut self, frame_index: u64) -> Result<(), String> {
        if self.drm.is_some() {
            return Ok(());
        }
        // vkRegisterDisplayEventEXT always returns ERROR_UNKNOWN on NVIDIA Tegra
        // before the first present.  Skip frame 0 to avoid a spurious warning.
        if frame_index == 0 {
            return Ok(());
        }
        if let Some(vblank) = self.vk.as_ref() {
            match vblank.register() {
                Some(fence) => self.pending_fence = Some(fence),
                None => {
                    return Err(
                        "vkRegisterDisplayEventEXT failed — VK_EXT_display_control vblank \
                         clock is unavailable"
                            .to_string(),
                    );
                }
            }
        }
        Ok(())
    }
}
