//! Headless Vulkan setup for the evdi backend.
//!
//! evdi's DRM node has no Vulkan device of its own (it's not a GPU, just a
//! virtual KMS device) — `VK_KHR_display` can't reach it. Instead of
//! duplicating `VkContext`/render-pass/pipeline setup, this uses
//! `VK_EXT_headless_surface` (confirmed available on this rig's V3D/Mesa
//! driver via `vulkaninfo`) to get a real `VkSwapchainKHR` that isn't tied to
//! any window system or physical display — Mesa's headless WSI backs it with
//! plain images and no presentation engine. `build_context()` (shared with
//! the DRM and Winit backends) then works completely unchanged, so
//! `render_frame()`, `SceneRenderer`, tessellation, and the egui overlay are
//! all reused as-is. The only new code is presenting the swapchain image
//! evdi's way afterward — see `evdi_render_loop.rs`.

use ash::vk;

use crate::render::vk::{VkContext, build_context, create_vk_instance};

/// Create a headless `VkContext` sized to `(width, height)`. No real display
/// is involved — this is purely an offscreen render target that happens to
/// go through the normal swapchain machinery.
pub fn init(width: u32, height: u32) -> VkContext {
    let exts = [
        ash::khr::surface::NAME.as_ptr(),
        ash::ext::headless_surface::NAME.as_ptr(),
    ];
    let (entry, instance, debug_utils_enabled) = create_vk_instance(&exts);

    let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
    let headless_loader = ash::ext::headless_surface::Instance::new(&entry, &instance);
    let surface = unsafe {
        headless_loader
            .create_headless_surface(&vk::HeadlessSurfaceCreateInfoEXT::default(), None)
            .expect("failed to create headless Vulkan surface")
    };

    let extent = vk::Extent2D { width, height };
    build_context(
        entry,
        instance,
        surface,
        surface_loader,
        extent,
        debug_utils_enabled,
        false,
    )
}
