use ash::vk;

use crate::render::vk::cache::{DotsInstanceCache, PhotodiodeCache, SolidMeshCache, TextMeshCache};

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
        }
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        self.solid.destroy_all(device);
        self.text.destroy_all(device);
        self.dots.destroy_all(device);
    }
}
