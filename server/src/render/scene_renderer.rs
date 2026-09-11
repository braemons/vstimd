use std::sync::{Arc, RwLock};

use crate::render::vk::{SceneCache, VkContext, VkDotsPipeline, VkGratingPipeline, VkPipeline};
use crate::scene::SceneState;

pub struct SceneRenderer {
    pub pipeline: VkPipeline,
    pub grating_pipeline: VkGratingPipeline,
    pub dots_pipeline: VkDotsPipeline,
    pub wireframe_pipeline: VkPipeline,
    pub wireframe_grating: VkGratingPipeline,
    pub wireframe: bool,
    pub scene_cache: SceneCache,
    pub scene: Arc<RwLock<SceneState>>,
}

impl SceneRenderer {
    pub fn new(ctx: &VkContext, scene: Arc<RwLock<SceneState>>) -> Self {
        let wf_mode = if ctx.supports_wireframe {
            ash::vk::PolygonMode::LINE
        } else {
            ash::vk::PolygonMode::FILL
        };
        let pipeline = VkPipeline::new(&ctx.device, ctx.render_pass, ash::vk::PolygonMode::FILL);
        let grating_pipeline = VkGratingPipeline::new(
            &ctx.device,
            &ctx.instance,
            ctx.physical_device,
            ctx.render_pass,
            ash::vk::PolygonMode::FILL,
        );
        // No wireframe twin: a dot field in wireframe would be a few thousand
        // outlined quads, which is neither informative nor legible.
        let dots_pipeline = VkDotsPipeline::new(
            &ctx.device,
            &ctx.instance,
            ctx.physical_device,
            ctx.render_pass,
            ash::vk::PolygonMode::FILL,
        );
        let wireframe_pipeline = VkPipeline::new(&ctx.device, ctx.render_pass, wf_mode);
        let wireframe_grating = VkGratingPipeline::new(
            &ctx.device,
            &ctx.instance,
            ctx.physical_device,
            ctx.render_pass,
            wf_mode,
        );
        ctx.set_debug_name(pipeline.pipeline, "solid_pipeline");
        ctx.set_debug_name(grating_pipeline.pipeline, "grating_pipeline");
        ctx.set_debug_name(dots_pipeline.pipeline, "dots_pipeline");
        ctx.set_debug_name(wireframe_pipeline.pipeline, "solid_wireframe_pipeline");
        ctx.set_debug_name(wireframe_grating.pipeline, "grating_wireframe_pipeline");
        let scene_cache = SceneCache::new(&ctx.instance, ctx.physical_device, ctx.frames.len());
        Self {
            pipeline,
            grating_pipeline,
            dots_pipeline,
            wireframe_pipeline,
            wireframe_grating,
            wireframe: false,
            scene_cache,
            scene,
        }
    }

    pub(super) fn destroy(&mut self, device: &ash::Device) {
        self.scene_cache.destroy_all(device);
        self.wireframe_grating.destroy(device);
        self.dots_pipeline.destroy(device);
        self.wireframe_pipeline.destroy(device);
        self.grating_pipeline.destroy(device);
        self.pipeline.destroy(device);
    }
}
