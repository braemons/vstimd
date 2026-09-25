use ash::vk;

use crate::render::vk::cache::{
    DotsInstanceCache, Mesh3dCache, PhotodiodeCache, SolidMeshCache, SplatCache, TextMeshCache,
};

/// Unified GPU-side cache for all stimulus types.
///
/// One `SceneCache` lives in `RenderState` and is passed as a single `&mut`
/// argument to `render_frame`.  Each field is the GPU buffer store for one
/// stimulus category; new categories (3-D meshes, video frames, …) add a
/// field here.
pub struct SceneCache {
    pub solid: SolidMeshCache,
    pub text: TextMeshCache,
    /// Dot fields hold no mesh — only a per-frame-slot instance buffer.
    pub dots: DotsInstanceCache,
    pub photodiode: PhotodiodeCache,
    /// Shared unit meshes for 3-D stimuli, keyed by geometry. Empty, and never
    /// touched, in a pure 2-D scene.
    pub mesh3d: Mesh3dCache,
    /// Gaussian splat clouds, keyed by stimulus handle. Empty in a scene
    /// without splats.
    pub splats: SplatCache,
    /// Reused by [`SplatCache::sync`] so following the scene does not allocate.
    pub splat_handles: Vec<u32>,
}

impl SceneCache {
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        frames_in_flight: usize,
    ) -> Self {
        Self {
            solid: SolidMeshCache::new(instance, physical_device),
            text: TextMeshCache::new(instance, physical_device),
            dots: DotsInstanceCache::new(instance, physical_device, frames_in_flight),
            photodiode: PhotodiodeCache::default(),
            mesh3d: Mesh3dCache::new(instance, physical_device),
            splats: SplatCache::new(instance, physical_device, frames_in_flight),
            splat_handles: Vec::new(),
        }
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        self.solid.destroy_all(device);
        self.text.destroy_all(device);
        self.dots.destroy_all(device);
        self.mesh3d.destroy_all(device);
        self.splats.destroy_all(device);
    }
}
