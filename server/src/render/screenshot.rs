//! Save the frame that is about to be presented as a PNG.
//!
//! This exists because the frames worth capturing are the ones no external
//! screenshot tool can reach. A rig runs on bare-metal DRM with no compositor,
//! no X11 and no window manager, so there is nothing on the box to ask for a
//! screenshot — and the frame a scientist wants a picture of is precisely the
//! one being driven to the panel. Reading it out of our own swapchain is the
//! only way to get it, and it works identically on every backend that renders
//! (DRM, winit, evdi), which an external tool never could.
//!
//! It captures what was *actually drawn*, overlay included, rather than a
//! re-creation of it — so a figure in the documentation is evidence about the
//! renderer instead of an illustration of it.
//!
//! ## How the frame is caught
//!
//! The copy has to be recorded into the render's own command buffer, before
//! the image is handed to the presentation engine (see [`Readback`]). So a
//! screenshot is not a thing that happens *after* a frame; it is a flag the
//! next frame is rendered *with*:
//!
//! 1. a keypress calls [`Screenshotter::request`], setting a pending flag;
//! 2. [`Screenshotter::begin`] runs just before `render_frame`, allocates a
//!    staging buffer for the current swapchain extent, and hands back the
//!    [`ReadbackTarget`] to render with;
//! 3. [`Screenshotter::finish`] runs just after, encodes, and writes the file.
//!
//! Steps 2 and 3 are no-ops on every frame where nothing was requested, which
//! is all but a handful of them, so an idle rig pays nothing for this.

use std::path::{Path, PathBuf};

use crate::render::ReadbackTarget;
use crate::render::vk::{Readback, VkContext};

/// Where screenshots go when nothing says otherwise: `$VSTIMD_SCREENSHOT_DIR`,
/// else the process's working directory.
///
/// Deliberately *not* under `--storage-dir`: that tree has exactly one child,
/// `projects/`, and putting an unrelated directory beside it would make the
/// one flag point at two unrelated things. An env var keeps the rig's data
/// layout intact while still letting a packaged unit — whose working directory
/// is not somewhere a person looks — put shots somewhere useful.
pub fn default_dir() -> PathBuf {
    std::env::var_os("VSTIMD_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Pending-screenshot state, owned by a backend alongside its `RenderState`.
#[derive(Default)]
pub struct Screenshotter {
    /// Where the next capture goes. `Some` means one is pending.
    pending: Option<PathBuf>,
    /// The staging buffer, alive only across a single `begin`/`finish` pair.
    readback: Option<Readback>,
}

impl Screenshotter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a capture for the next rendered frame.
    ///
    /// The name is a UTC timestamp, matching the scene-config archives, so
    /// shots sort chronologically and two in the same session cannot collide
    /// unless they land in the same second — in which case the second is
    /// dropped rather than silently overwriting the first.
    pub fn request(&mut self, dir: &Path) {
        if self.pending.is_some() {
            log::debug!("vstimd: screenshot already pending — ignoring");
            return;
        }
        let name = format!("vstimd-{}.png", crate::scene_config_file::archive_timestamp_name());
        self.pending = Some(dir.join(name));
    }

    /// Call immediately before `render_frame`, passing the result through as
    /// its `readback` argument. `None` on any frame with nothing pending.
    pub fn begin(&mut self, ctx: &VkContext) -> Option<&ReadbackTarget> {
        self.pending.as_ref()?;
        let extent = ctx.extent;
        // Tightly packed: unlike evdi, nothing downstream dictates a stride.
        let readback = Readback::new(ctx, extent.width as usize * 4, extent.height);
        Some(&self.readback.insert(readback).target)
    }

    /// Call immediately after `render_frame`. Encodes and writes the file,
    /// then releases the staging buffer.
    ///
    /// A failure here is logged, never fatal: a screenshot is a convenience,
    /// and an unwritable directory must not take a running experiment down.
    pub fn finish(&mut self, ctx: &VkContext) {
        let (Some(path), Some(readback)) = (self.pending.take(), self.readback.take()) else {
            return;
        };
        let extent = ctx.extent;
        match write_png(&path, readback.frame(), extent.width, extent.height) {
            Ok(()) => log::info!(
                "vstimd: screenshot saved to {} ({}x{})",
                path.display(),
                extent.width,
                extent.height
            ),
            Err(e) => log::error!("vstimd: screenshot to {} failed: {e}", path.display()),
        }
    }
}

/// Encode a BGRA frame as an opaque RGB PNG.
///
/// The swapchain is `B8G8R8A8_UNORM` (see `vk_context.rs`), so the channels are
/// reordered here. Alpha is dropped rather than carried: the swapchain's alpha
/// is not meaningful as transparency — the frame was composited against the
/// scene background already — and a PNG that claims to have an alpha channel
/// invites a viewer to composite it a second time.
fn write_png(path: &Path, bgra: &[u8], width: u32, height: u32) -> std::io::Result<()> {
    let expected = width as usize * height as usize * 4;
    if bgra.len() < expected {
        return Err(std::io::Error::other(format!(
            "readback buffer is {} bytes, expected {expected} for {width}x{height}",
            bgra.len()
        )));
    }

    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in bgra[..expected].chunks_exact(4) {
        rgb.extend_from_slice(&[px[2], px[1], px[0]]);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer.write_image_data(&rgb).map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The channel swap is the one thing here that is easy to get backwards and
    /// impossible to notice in a greyscale test pattern.
    #[test]
    fn bgra_is_written_as_rgb() {
        let dir = std::env::temp_dir().join("vstimd-screenshot-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("shot.png");

        // One pure-red pixel, in BGRA: blue=0, green=0, red=255.
        write_png(&path, &[0, 0, 255, 255], 1, 1).expect("write");

        let file = std::io::BufReader::new(std::fs::File::open(&path).expect("open"));
        let decoder = png::Decoder::new(file);
        let mut reader = decoder.read_info().expect("read info");
        let mut buf = vec![0; reader.output_buffer_size().unwrap_or(3)];
        let info = reader.next_frame(&mut buf).expect("decode");

        assert_eq!(info.color_type, png::ColorType::Rgb);
        assert_eq!(&buf[..3], &[255, 0, 0], "red must survive the BGRA→RGB swap");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn short_buffer_is_an_error_not_a_panic() {
        let path = std::env::temp_dir().join("vstimd-screenshot-short.png");
        let err = write_png(&path, &[0, 0, 0, 0], 4, 4).expect_err("must reject");
        assert!(err.to_string().contains("expected"), "{err}");
    }

    #[test]
    fn a_second_request_does_not_replace_a_pending_one() {
        let mut s = Screenshotter::new();
        s.request(Path::new("/tmp"));
        let first = s.pending.clone();
        s.request(Path::new("/somewhere/else"));
        assert_eq!(s.pending, first, "a pending capture wins over a later request");
    }
}
