//! Vulkan setup for the evdi backend.
//!
//! Deliberately does *not* go through `VK_EXT_headless_surface` +
//! `vkCreateSwapchainKHR` — what `build_context()` (and thus the DRM and
//! Winit backends) use. Mesa's v3dv WSI headless-surface implementation has
//! a real bug on current (26.2.0) `mesa-vulkan-drivers`: `vkCreateSwapchainKHR`
//! internally calls `DRM_IOCTL_MODE_CREATE_DUMB` (a KMS-only ioctl, only
//! valid on a primary/`cardN` node, never a render node) with an fd that was
//! never opened, so the kernel returns `EBADF`, which v3dv reports up as
//! `VK_ERROR_OUT_OF_DEVICE_MEMORY` — confirmed via `strace`, 100% reproducible,
//! independent of every swapchain parameter (extent, format, image count,
//! usage flags, present mode, API version). Plain `vkCreateImage` +
//! `vkAllocateMemory` on the same driver, same device, same usage flags,
//! works fine — the bug is isolated to the WSI layer, not V3D itself.
//!
//! evdi never actually needed a real swapchain: it reads the rendered image
//! back to the CPU and pushes it to evdi's own virtual KMS device
//! (`evdi_kms.rs`) directly via `page_flip`, entirely outside Vulkan — there
//! is no presentation engine on the other end to synchronize with. So this
//! allocates its own `vk::Image`s directly and round-robins between them.
//! `build_context()`'s render-pass/pipeline/framebuffer setup, and
//! `render_frame()` itself, are otherwise reused unchanged — see
//! `VkContext::self_presented`, which tells `render_frame()` to skip
//! `acquire_next_image`/`queue_present` for a context built this way.

use ash::vk;

use crate::render::vk::VkContext;
use crate::render::vk::create_vk_instance;
use crate::render::vk::vk_context::{FRAMES_IN_FLIGHT, FrameSync, create_framebuffers};

/// Number of self-owned images to round-robin between. Matches the DRM/Winit
/// backends' minimum (2) — evdi has no presentation engine to give headroom
/// against, and FRAMES_IN_FLIGHT == 1 already fully serializes each frame, so
/// more would only cost VRAM without buying anything.
const IMAGE_COUNT: usize = 2;

/// Create a `VkContext` sized to `(width, height)`. No real display or
/// swapchain is involved — this allocates its own offscreen render targets.
pub fn init(width: u32, height: u32) -> VkContext {
    let instance_exts = [ash::khr::surface::NAME.as_ptr()];
    let (entry, instance, debug_utils_enabled) = create_vk_instance(&instance_exts);

    // No real surface is ever created — VK_KHR_surface is enabled purely so
    // this loader is valid for Drop's unconditional `destroy_surface(VK_NULL_HANDLE,
    // ...)` call, which the spec defines as a no-op.
    let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

    let physical_devices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("no Vulkan physical devices")
    };
    let (physical_device, graphics_queue_family) = physical_devices
        .iter()
        .find_map(|&pd| find_graphics_queue(&instance, pd).map(|qf| (pd, qf)))
        .expect("no Vulkan device with a graphics queue");

    {
        let props = unsafe { instance.get_physical_device_properties(physical_device) };
        let name = unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) };
        log::info!(
            "vstimd: Vulkan physical device: {:?}  type={:?}  driver=0x{:x}  vendor=0x{:x}",
            name, props.device_type, props.driver_version, props.vendor_id
        );
    }

    let phys_features = unsafe { instance.get_physical_device_features(physical_device) };
    let supports_wireframe = phys_features.fill_mode_non_solid == vk::TRUE;
    if supports_wireframe {
        log::info!("vstimd: fillModeNonSolid supported — wireframe toggle available");
    }

    let queue_priorities = [1.0_f32];
    let queue_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(graphics_queue_family)
        .queue_priorities(&queue_priorities);
    // VK_KHR_swapchain is enabled purely so Drop's swapchain_loader is valid
    // to call destroy_swapchain(VK_NULL_HANDLE, ...) on — also a defined
    // no-op. Nothing here creates a real swapchain.
    let device_exts = [ash::khr::swapchain::NAME.as_ptr()];
    let enabled_features =
        vk::PhysicalDeviceFeatures::default().fill_mode_non_solid(supports_wireframe);
    let device_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_info))
        .enabled_extension_names(&device_exts)
        .enabled_features(&enabled_features);
    let device = unsafe {
        instance
            .create_device(physical_device, &device_info, None)
            .expect("failed to create Vulkan logical device")
    };
    let graphics_queue = unsafe { device.get_device_queue(graphics_queue_family, 0) };
    let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);

    let format = vk::Format::B8G8R8A8_UNORM;
    let extent = vk::Extent2D { width, height };

    let (swapchain_images, swapchain_image_views, owned_image_memory) =
        create_owned_images(&instance, &device, physical_device, format, extent);

    let pool_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(graphics_queue_family);
    let command_pool = unsafe {
        device
            .create_command_pool(&pool_info, None)
            .expect("failed to create command pool")
    };
    let cmd_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(FRAMES_IN_FLIGHT as u32);
    let cbs = unsafe {
        device
            .allocate_command_buffers(&cmd_info)
            .expect("failed to allocate command buffers")
    };
    let frames: Vec<FrameSync> = (0..FRAMES_IN_FLIGHT)
        .map(|i| {
            let sem = vk::SemaphoreCreateInfo::default();
            let fence = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            FrameSync {
                image_available: unsafe { device.create_semaphore(&sem, None).unwrap() },
                render_done: unsafe { device.create_semaphore(&sem, None).unwrap() },
                in_flight: unsafe { device.create_fence(&fence, None).unwrap() },
                command_buffer: cbs[i],
            }
        })
        .collect();

    let debug_utils =
        debug_utils_enabled.then(|| ash::ext::debug_utils::Device::new(&instance, &device));
    if debug_utils.is_some() {
        log::info!("vstimd: VK_EXT_debug_utils enabled — RenderDoc labels/names active");
    }

    let render_pass = create_render_pass_no_wsi(&device, format);
    let egui_render_pass = create_egui_render_pass_no_wsi(&device, format);
    let framebuffers = create_framebuffers(&device, render_pass, &swapchain_image_views, extent);

    VkContext {
        frames,
        framebuffers,
        render_pass,
        egui_render_pass,
        swapchain_image_views,
        swapchain_images,
        command_pool,
        swapchain: vk::SwapchainKHR::null(),
        swapchain_loader,
        graphics_queue,
        graphics_queue_family,
        device,
        surface_loader,
        surface: vk::SurfaceKHR::null(),
        physical_device,
        format,
        extent,
        present_mode: vk::PresentModeKHR::FIFO,
        present_wait: None,
        next_present_id: std::cell::Cell::new(1),
        display_timing: None,
        instance,
        entry,
        supports_wireframe,
        debug_utils,
        display_control: None,
        surface_counter_enabled: false,
        self_presented: true,
        owned_image_memory,
    }
}

/// Allocate `IMAGE_COUNT` device-local images (+ views) for evdi to render
/// into and read back from directly, sized and formatted to match what
/// `create_swapchain()` would otherwise produce.
fn create_owned_images(
    instance: &ash::Instance,
    device: &ash::Device,
    physical_device: vk::PhysicalDevice,
    format: vk::Format,
    extent: vk::Extent2D,
) -> (Vec<vk::Image>, Vec<vk::ImageView>, Vec<vk::DeviceMemory>) {
    let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC;

    let mut images = Vec::with_capacity(IMAGE_COUNT);
    let mut views = Vec::with_capacity(IMAGE_COUNT);
    let mut memory = Vec::with_capacity(IMAGE_COUNT);

    for _ in 0..IMAGE_COUNT {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = unsafe {
            device
                .create_image(&image_info, None)
                .expect("failed to create evdi render target image")
        };

        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let type_index = (0..mem_props.memory_type_count)
            .find(|&i| {
                (reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize]
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .expect("no DEVICE_LOCAL memory type for evdi render target image");
        let mem = unsafe {
            device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(type_index),
                    None,
                )
                .expect("failed to allocate evdi render target memory")
        };
        unsafe {
            device
                .bind_image_memory(image, mem, 0)
                .expect("failed to bind evdi render target memory")
        };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe {
            device
                .create_image_view(&view_info, None)
                .expect("failed to create evdi render target image view")
        };

        images.push(image);
        views.push(view);
        memory.push(mem);
    }

    (images, views, memory)
}

/// Same attachment/subpass shape as `vk_context::create_render_pass`, except
/// the color attachment's layouts stay `GENERAL` instead of `PRESENT_SRC_KHR`.
///
/// `PRESENT_SRC_KHR` is spec-defined for presentable (swapchain) images only;
/// on this driver, transitioning a *self-owned* image (see `init()`'s doc
/// comment) to/from it makes v3dv spin forever retrying a DRM syncobj ioctl
/// that keeps returning EINVAL, instead of erroring out — confirmed via
/// strace (`DRM_IOCTL_SYNCOBJ_WAIT`/an EINVAL-looping ioctl pair, 100% CPU,
/// unkillable even by SIGTERM). `GENERAL` carries no WSI-specific meaning, so
/// it avoids whatever internal bookkeeping v3dv attaches to that transition
/// for images it didn't hand out itself. render_frame.rs's readback barrier
/// mirrors this via `ctx.self_presented`.
fn create_render_pass_no_wsi(device: &ash::Device, format: vk::Format) -> vk::RenderPass {
    let attachment = vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::GENERAL);
    let color_ref =
        vk::AttachmentReference::default().layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_ref));
    let dep = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
    let info = vk::RenderPassCreateInfo::default()
        .attachments(std::slice::from_ref(&attachment))
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dep));
    unsafe {
        device
            .create_render_pass(&info, None)
            .expect("failed to create render pass")
    }
}

/// `GENERAL`-layout counterpart of `vk_context::create_egui_render_pass` —
/// see `create_render_pass_no_wsi`'s doc comment.
fn create_egui_render_pass_no_wsi(device: &ash::Device, format: vk::Format) -> vk::RenderPass {
    let attachment = vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::LOAD)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::GENERAL)
        .final_layout(vk::ImageLayout::GENERAL);
    let color_ref =
        vk::AttachmentReference::default().layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_ref));
    let dep = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
    let info = vk::RenderPassCreateInfo::default()
        .attachments(std::slice::from_ref(&attachment))
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dep));
    unsafe {
        device
            .create_render_pass(&info, None)
            .expect("failed to create egui render pass")
    }
}

fn find_graphics_queue(instance: &ash::Instance, pd: vk::PhysicalDevice) -> Option<u32> {
    let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
    families.iter().enumerate().find_map(|(i, props)| {
        props
            .queue_flags
            .contains(vk::QueueFlags::GRAPHICS)
            .then_some(i as u32)
    })
}
