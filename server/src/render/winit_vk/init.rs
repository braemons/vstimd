use ash::vk;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

use crate::render::vk::{VkContext, build_context};

/// Create a `VkContext` from a winit window using `VK_KHR_surface`.
pub fn init(window: &Window) -> VkContext {
    let entry = unsafe { ash::Entry::load().expect("failed to load libvulkan.so") };

    let display_handle = window.display_handle().unwrap().as_raw();
    let window_handle = window.window_handle().unwrap().as_raw();

    // ash-window provides the right platform surface extension(s) for this display.
    let required_exts = {
        ash_window::enumerate_required_extensions(display_handle)
            .expect("failed to enumerate required Vulkan surface extensions")
    };

    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);
    let instance_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(required_exts);
    let instance = unsafe {
        entry
            .create_instance(&instance_info, None)
            .expect("failed to create Vulkan instance")
    };

    let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
    let surface = unsafe {
        ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)
            .expect("failed to create Vulkan window surface")
    };

    let size = window.inner_size();
    let extent = vk::Extent2D {
        width: size.width.max(1),
        height: size.height.max(1),
    };

    build_context(entry, instance, surface, surface_loader, extent)
}
