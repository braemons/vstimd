use std::sync::{Arc, RwLock};

use crate::render::vk::{
    Mesh3dRenderer, Pass3d, SceneCache, SplatRenderer, VkContext, VkDotsPipeline, VkGratingPipeline, VkPipeline,
};
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
    /// Pipelines and the scene uniform for the 3-D pass. `None` until the first
    /// 3-D frame, like `VkContext::pass_3d` — see [`Self::ensure_3d`].
    pub mesh3d: Option<Mesh3dRenderer>,
    /// The splat and veil pipelines, created with `mesh3d`.
    pub splat: Option<SplatRenderer>,
    /// Set once 3-D setup has failed (no depth format), so it is not retried —
    /// and logged — every frame. 2-D keeps rendering.
    pub mesh3d_unavailable: bool,
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
            mesh3d: None,
            splat: None,
            mesh3d_unavailable: false,
        }
    }

    /// Create the 3-D pass, depth buffer and pipelines if they do not exist yet.
    /// Returns whether 3-D can be drawn this frame.
    ///
    /// Runs on the render thread, so the first 3-D frame pays for a pipeline
    /// build and a depth allocation and may drop. #72 moves this onto the ZMQ
    /// thread's create command, which already allocates.
    pub fn ensure_3d(&mut self, ctx: &mut VkContext) -> bool {
        if self.mesh3d.is_some() {
            return true;
        }
        if self.mesh3d_unavailable {
            return false;
        }
        let pass = match Pass3d::new(
            &ctx.instance,
            ctx.physical_device,
            &ctx.device,
            ctx.format,
            ctx.present_layout,
            &ctx.swapchain_image_views,
            ctx.extent,
        ) {
            Ok(pass) => pass,
            Err(e) => {
                log::error!("vstimd: 3-D unavailable ({e}); continuing with 2-D only");
                self.mesh3d_unavailable = true;
                if let Ok(mut scene) = self.scene.write() {
                    scene.runtime.render_3d_unavailable = true;
                }
                return false;
            }
        };
        let mesh3d = Mesh3dRenderer::new(
            &ctx.instance,
            ctx.physical_device,
            &ctx.device,
            pass.render_pass,
            ctx.frames.len(),
            ctx.supports_wireframe,
        );
        ctx.set_debug_name(pass.render_pass, "render_pass_3d");
        ctx.set_debug_name(pass.load_2d_pass, "render_pass_2d_load");
        ctx.set_debug_name(mesh3d.pipeline.pipeline, "mesh3d_pipeline");
        ctx.set_debug_name(mesh3d.wireframe_pipeline.pipeline, "mesh3d_wireframe_pipeline");
        let splat = SplatRenderer::new(&ctx.device, pass.render_pass);
        ctx.set_debug_name(splat.pipeline, "splat_pipeline");
        ctx.set_debug_name(splat.veil_pipeline, "veil_pipeline");
        self.splat = Some(splat);
        ctx.pass_3d = Some(pass);
        self.mesh3d = Some(mesh3d);
        true
    }

    pub(super) fn destroy(&mut self, device: &ash::Device) {
        if let Some(mesh3d) = &self.mesh3d {
            mesh3d.destroy(device);
        }
        if let Some(splat) = &self.splat {
            splat.destroy(device);
        }
        self.scene_cache.destroy_all(device);
        self.wireframe_grating.destroy(device);
        self.dots_pipeline.destroy(device);
        self.wireframe_pipeline.destroy(device);
        self.grating_pipeline.destroy(device);
        self.pipeline.destroy(device);
    }
}
