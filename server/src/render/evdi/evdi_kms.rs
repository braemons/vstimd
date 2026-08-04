//! evdi KMS output: mode/CRTC selection, double-buffered dumb buffers, and
//! frame presentation. The selection logic here is exactly what
//! `examples/evdi_probe.rs` proved works against real hardware (a Pi 5 with
//! a DisplayLink dongle on `/dev/dri/card2`) before being folded in.
//!
//! Presentation uses `set_crtc` every frame rather than `page_flip`.
//! `page_flip` needs vblank-event bookkeeping to avoid `EBUSY` on a still-
//! pending flip; `set_crtc` is synchronous and already proven. Since this
//! backend has no timing goals — DisplayLink relays over USB with no GPU
//! vsync regardless — the heavier per-frame modeset costs nothing that
//! matters here.

use drm::buffer::{Buffer, DrmFourcc};
use drm::control::Device as CtrlDevice;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{Mode, connector, crtc, framebuffer};

use super::evdi_detect::{Card, EvdiNode};

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
    /// Index into `buffers` of the frame currently on screen. `present`
    /// writes into the other one and flips to it.
    front: usize,
}

impl EvdiOutput {
    /// Picks the connector's first reported mode, finds a usable CRTC (the
    /// current encoder's if the connector already has one active,
    /// otherwise the first encoder whose `possible_crtcs` is non-empty),
    /// and allocates two XRGB8888 dumb buffers sized to that mode.
    pub fn new(node: EvdiNode) -> std::io::Result<Self> {
        let card = node.card;

        let conn = card.get_connector(node.connector, false)?;
        let mode = *conn.modes().first().ok_or_else(|| {
            std::io::Error::other("evdi connector reports no modes")
        })?;
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

        Ok(Self {
            card,
            connector: node.connector,
            crtc,
            mode,
            width,
            height,
            buffers,
            front: 0,
        })
    }

    /// Row pitch of the framebuffers, in bytes. Callers writing into
    /// `present`'s `src` argument should honor this if it differs from
    /// `width * 4` (the dumb-buffer allocator is free to pad rows).
    pub fn pitch(&self) -> usize {
        self.buffers[0].pitch
    }

    /// Copies `src` (XRGB8888, `pitch()` bytes per row, `height` rows) into
    /// the back buffer and presents it.
    pub fn present(&mut self, src: &[u8]) -> std::io::Result<()> {
        let back = 1 - self.front;
        {
            let mut map = self.card.map_dumb_buffer(&mut self.buffers[back].dumb)?;
            let n = src.len().min(map.len());
            map[..n].copy_from_slice(&src[..n]);
        }

        let fb = self.buffers[back].fb;
        self.card
            .set_crtc(self.crtc, Some(fb), (0, 0), &[self.connector], Some(self.mode))?;

        self.front = back;
        Ok(())
    }
}

impl Drop for EvdiOutput {
    fn drop(&mut self) {
        for buf in &self.buffers {
            let _ = self.card.destroy_framebuffer(buf.fb);
        }
    }
}
