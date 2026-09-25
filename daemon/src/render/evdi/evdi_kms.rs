//! evdi KMS output: mode/CRTC selection, double-buffered dumb buffers, and
//! frame presentation. The mode/CRTC selection here is exactly what
//! `examples/evdi_probe.rs` proved works against real hardware (a Pi 5 with
//! a DisplayLink dongle on `/dev/dri/card2`) before being folded in.
//!
//! evdi has no periodic vblank timer (`evdi_enable_vblank()` in the kernel
//! driver is a stub returning `1`; confirmed on hardware too —
//! `DRM_IOCTL_WAIT_VBLANK` against evdi returns in under 2µs every time with
//! a frozen frame counter). What it does have: every atomic commit marks the
//! frame dirty, and if that commit was a `page_flip` submitted with
//! `DRM_MODE_PAGE_FLIP_EVENT`, the kernel driver
//! (`evdi_painter_set_vblank`/`evdi_painter_send_update_ready_if_needed` in
//! `evdi_painter.c`) only completes the flip event once DisplayLinkManager
//! has actually drained the *previous* frame's dirty rects through its
//! `GRABPIX` ioctl — otherwise completion is deferred until it does.
//! Confirmed on hardware: flip-to-event latency was irregular (1.5–19ms),
//! not periodic — real flow control paced to actual consumption, not a
//! synthetic clock. `present()` therefore uses `page_flip` + blocks on the
//! completion event, giving proper backpressure instead of the unthrottled
//! write race a plain `set_crtc`-every-frame loop has (which is what an
//! earlier version of this module did, and which visibly teared).

use drm::buffer::{Buffer, DrmFourcc};
use drm::control::Device as CtrlDevice;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    Event, Mode, ModeTypeFlags, PageFlipFlags, PageFlipTarget, connector, crtc, framebuffer,
};

use super::evdi_detect::{Card, EvdiNode};

/// Picks the connector's native mode: the one its EDID marks
/// `DRM_MODE_TYPE_PREFERRED`. Confirmed on hardware (ASUS MB168B) that evdi
/// does report this correctly. **Not** simply `modes().first()`: the list's
/// order/cache is not reliable — an early or stale probe (e.g. before the
/// EDID is fully read) can leave `modes()` returning fewer entries in a
/// different order, silently picking a lower resolution (1024×768 observed
/// in the field on a monitor whose native mode is 1366×768). Falls back to
/// the largest-area mode if — some EDIDs are incomplete — none is marked
/// preferred.
fn pick_native_mode(modes: &[Mode]) -> Option<Mode> {
    modes
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| {
            modes.iter().max_by_key(|m| {
                let (w, h) = m.size();
                w as u64 * h as u64
            })
        })
        .copied()
}

struct OutputBuffer {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
    pitch: usize,
}

/// A `evdi` KMS output with a mode picked, a CRTC selected, and two
/// double-buffered XRGB8888 framebuffers allocated and ready to present.
pub struct EvdiOutput {
    card: Card,
    connector: connector::Handle,
    crtc: crtc::Handle,
    pub mode: Mode,
    pub width: u32,
    pub height: u32,
    buffers: [OutputBuffer; 2],
    /// Index into `buffers` of the frame currently on screen. `submit`
    /// writes into the other one and flips to it.
    front: usize,
    /// True between a `submit` and the `wait_flip` that collects its event.
    flip_pending: bool,
}

impl EvdiOutput {
    /// Picks the connector's native (EDID-preferred) mode, finds a usable
    /// CRTC (the current encoder's if the connector already has one active,
    /// otherwise the first encoder whose `possible_crtcs` is non-empty),
    /// allocates two XRGB8888 dumb buffers sized to that mode, and sets the
    /// initial mode via `set_crtc` (`page_flip` needs an already-active
    /// CRTC to flip from).
    pub fn new(node: EvdiNode) -> std::io::Result<Self> {
        let card = node.card;

        // force=true: a fresh reprobe, not whatever the DRM subsystem last
        // cached for this connector (which — see pick_native_mode — is
        // exactly the situation that produced a wrong/stale mode list in
        // the field).
        let conn = card.get_connector(node.connector, true)?;
        let mode = pick_native_mode(conn.modes())
            .ok_or_else(|| std::io::Error::other("evdi connector reports no modes"))?;
        let (width, height) = mode.size();
        let (width, height) = (width as u32, height as u32);

        let res = card.resource_handles()?;
        let crtc = conn
            .current_encoder()
            .and_then(|enc_h| card.get_encoder(enc_h).ok())
            .and_then(|enc| enc.crtc())
            .or_else(|| {
                conn.encoders().iter().find_map(|&enc_h| {
                    let enc = card.get_encoder(enc_h).ok()?;
                    res.filter_crtcs(enc.possible_crtcs()).into_iter().next()
                })
            })
            .ok_or_else(|| std::io::Error::other("no usable CRTC for evdi connector"))?;

        let make_buffer = || -> std::io::Result<OutputBuffer> {
            let dumb = card.create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)?;
            let pitch = dumb.pitch() as usize;
            let fb = card.add_framebuffer(&dumb, 24, 32)?;
            Ok(OutputBuffer { dumb, fb, pitch })
        };
        let buffers = [make_buffer()?, make_buffer()?];

        log::info!(
            "vstimd: evdi output {}×{} on {:?}, CRTC {:?}",
            width,
            height,
            node.connector,
            crtc
        );

        let out = Self {
            card,
            connector: node.connector,
            crtc,
            mode,
            width,
            height,
            buffers,
            front: 0,
            flip_pending: false,
        };

        out.card.set_crtc(
            out.crtc,
            Some(out.buffers[0].fb),
            (0, 0),
            &[out.connector],
            Some(out.mode),
        )?;

        Ok(out)
    }

    /// Row pitch of the framebuffers, in bytes. Callers writing into
    /// `present`'s `src` argument should honor this if it differs from
    /// `width * 4` (the dumb-buffer allocator is free to pad rows).
    pub fn pitch(&self) -> usize {
        self.buffers[0].pitch
    }

    /// Copies `src` (XRGB8888, `pitch()` bytes per row, `height` rows) into
    /// the back buffer and flips to it, **without** waiting for the flip to
    /// complete. Call [`wait_flip`](Self::wait_flip) before the next
    /// `submit` — the back buffer is only free to overwrite once the
    /// outstanding flip has landed.
    ///
    /// Split from the wait so the caller can render and read back the *next*
    /// frame while DisplayLink is still draining this one. The flip event is
    /// paced by actual consumption (see module docs), so it is the longest
    /// single stall in the loop; overlapping it with GPU work is most of the
    /// difference between a serial and a pipelined presenter.
    pub fn submit(&mut self, src: &[u8]) -> std::io::Result<()> {
        let back = 1 - self.front;
        {
            let mut map = self.card.map_dumb_buffer(&mut self.buffers[back].dumb)?;
            if src.len() != map.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "evdi frame buffer size mismatch: got {} bytes, dumb buffer is {} bytes",
                        src.len(),
                        map.len()
                    ),
                ));
            }
            map.copy_from_slice(src);
        }

        let fb = self.buffers[back].fb;
        self.card
            .page_flip(self.crtc, fb, PageFlipFlags::EVENT, None::<PageFlipTarget>)?;
        self.front = back;
        self.flip_pending = true;

        Ok(())
    }

    /// Blocks until evdi confirms the outstanding flip landed (see module
    /// docs — this is real backpressure, not a fixed-rate wait). No-op if no
    /// flip is outstanding.
    pub fn wait_flip(&mut self) -> std::io::Result<()> {
        if !self.flip_pending {
            return Ok(());
        }
        loop {
            for ev in self.card.receive_events()? {
                if let Event::PageFlip(_) = ev {
                    self.flip_pending = false;
                    return Ok(());
                }
            }
        }
    }
}

impl Drop for EvdiOutput {
    fn drop(&mut self) {
        for buf in &self.buffers {
            let _ = self.card.destroy_framebuffer(buf.fb);
        }
    }
}
