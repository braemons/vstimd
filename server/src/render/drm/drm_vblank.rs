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
    crtc_pipe: u32,
}

impl DrmVblank {
    /// Iterate /dev/dri/card* and return a handle bound to the first CRTC that
    /// is actively driving a display (mode set). Returns `None` if none found.
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
                    log::info!("vstimd: DRM vblank: {path} crtc[{pipe}]");
                    return Some(Self {
                        card,
                        crtc_pipe: pipe as u32,
                    });
                }
            }
        }
        log::warn!("vstimd: no active DRM CRTC found for vblank — using GPU-completion time");
        None
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

/// Owns both vblank clock sources and the pending FIRST_PIXEL_OUT fence,
/// collapsing the three separate fields and two methods that previously lived
/// in `DrmRenderState`.
pub struct DrmVblankState {
    drm: Option<DrmVblank>,
    vk: Option<VkVblank>,
    /// Fence registered at end of previous frame; collected at start of next.
    pending_fence: Option<ash::vk::Fence>,
    /// Retained solely to destroy any orphaned fence if `vk` is disabled
    /// between a register and the following collect.
    device: ash::Device,
}

impl DrmVblankState {
    pub fn new(device: ash::Device, drm: Option<DrmVblank>, vk: Option<VkVblank>) -> Self {
        Self { device, drm, vk, pending_fence: None }
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
    /// (VK path).  Returns `None` on frame 0 or when no clock is available.
    pub fn wait(&mut self) -> Option<Instant> {
        if let Some(vblank) = self.drm.as_ref() {
            match vblank.wait() {
                Some(t) => return Some(t),
                None => {
                    log::warn!("vstimd: disabling DRM vblank clock after wait_vblank error");
                    self.drm = None;
                }
            }
        }
        // VK path: collect the fence registered at the end of the previous frame.
        // On frame 0 there is no pending fence; we return None and render without
        // a vblank timestamp (render_one_frame falls back to Instant::now()).
        if let Some(fence) = self.pending_fence.take() {
            if let Some(vblank) = self.vk.as_ref() {
                match vblank.collect(fence) {
                    Some(t) => return Some(t),
                    None => {
                        log::warn!("vstimd: disabling VK_EXT_display_control vblank after error");
                        self.vk = None;
                    }
                }
            } else {
                // vk was disabled between register and collect; destroy the orphaned fence.
                unsafe { self.device.destroy_fence(fence, None) };
            }
        }
        None
    }

    /// Register a FIRST_PIXEL_OUT fence for collection at the top of the next
    /// frame.  No-op on the DRM path (DRM uses a blocking ioctl instead) and
    /// on frame 0 (driver returns ERROR_UNKNOWN on Tegra before first present).
    pub fn register(&mut self, frame_index: u64) {
        if self.drm.is_some() {
            return;
        }
        // vkRegisterDisplayEventEXT always returns ERROR_UNKNOWN on NVIDIA Tegra
        // before the first present.  Skip frame 0 to avoid a spurious warning.
        if frame_index == 0 {
            return;
        }
        if let Some(vblank) = self.vk.as_ref() {
            self.pending_fence = vblank.register();
        }
    }
}
