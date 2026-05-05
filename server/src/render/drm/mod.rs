mod init;
mod input;

use std::sync::{Arc, RwLock};

use crate::render::vk::{GpuBuffers, VkPipeline, render_frame};
use crate::scene::SceneState;
use crate::timing::FrameStats;

use self::input::{AppKey, InputState};

/// Bare-metal Linux render state — drives the display directly via
/// `VK_KHR_display` without a compositor.
pub struct DrmRenderState {
    ctx: crate::render::vk::VkContext,
    pipeline: VkPipeline,
    gpu_buffers: GpuBuffers,
    input: InputState,
    scene: Arc<RwLock<SceneState>>,
    frame_stats: FrameStats,
}

impl DrmRenderState {
    pub fn new(scene: Arc<RwLock<SceneState>>) -> Self {
        let (ctx, _display_info) = init::init();
        let pipeline = VkPipeline::new(&ctx.device, ctx.render_pass);
        let gpu_buffers = GpuBuffers::new(&ctx.instance, ctx.physical_device);
        let input = InputState::new();
        Self { ctx, pipeline, gpu_buffers, input, scene, frame_stats: FrameStats::new(60.0) }
    }

    pub fn run_loop(mut self) {
        let mut frame_index: usize = 0;
        loop {
            for key in self.input.poll() {
                match key {
                    AppKey::Escape => {
                        self.cleanup();
                        return;
                    }
                    AppKey::D => crate::render::spawn_demo_stimuli(&self.scene),
                    AppKey::F1 => {} // overlay not implemented for DRM yet
                }
            }
            render_frame(
                &self.ctx,
                &self.pipeline,
                &mut self.gpu_buffers,
                &self.scene,
                &mut frame_index,
                &mut self.frame_stats,
            );
        }
    }

    fn cleanup(&mut self) {
        unsafe { self.ctx.device.device_wait_idle().ok() };
        self.gpu_buffers.destroy_all(&self.ctx.device);
        self.pipeline.destroy(&self.ctx.device);
    }
}
