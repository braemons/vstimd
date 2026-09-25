use std::path::PathBuf;

use crate::log_buffer::LogBuffer;
use crate::render::overlay_ui::OverlayState;
use crate::system_metrics::MetricsSampler;
use crate::render::vk::{VkContext, VkEguiRenderer};

/// egui overlay state — renderer, context, and overlay-specific data.
/// Wrapped in `Option` in `RenderState` so the overlay can be entirely absent.
pub struct UiRenderer {
    pub egui_renderer: VkEguiRenderer,
    pub egui_ctx: egui::Context,
    /// Grouped-window visibility, focus, and owned dialogs.
    pub overlay: OverlayState,
    pub metrics: MetricsSampler,
    pub log_buffer: LogBuffer,
}

impl UiRenderer {
    pub fn new(ctx: &VkContext, storage_dir: PathBuf, log_buffer: LogBuffer, overlay_scale: f32) -> Self {
        let egui_renderer = VkEguiRenderer::new(
            &ctx.device,
            &ctx.instance,
            ctx.physical_device,
            ctx.egui_render_pass,
        );
        let egui_ctx = egui::Context::default();
        // pixels_per_point = zoom_factor * native_pixels_per_point, so this
        // composes with the OS DPI scale on desktop and stands alone in DRM
        // mode (which reports no native scale factor).
        egui_ctx.set_zoom_factor(overlay_scale);
        Self {
            egui_renderer,
            egui_ctx,
            overlay: OverlayState::new(storage_dir),
            metrics: MetricsSampler::new(),
            log_buffer,
        }
    }

    pub(crate) fn destroy(&mut self, device: &ash::Device) {
        self.egui_renderer.destroy(device);
    }
}
