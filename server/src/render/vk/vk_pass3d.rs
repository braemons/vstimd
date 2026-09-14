//! The 3-D render pass and its depth buffer — created lazily, on the first frame
//! a 3-D stimulus exists (`dev/3D_ROADMAP.md` §2.2, §A.3, §A.4).
//!
//! ## Why this is a separate pass, and why the 2-D pass is left alone
//!
//! The frame is recorded as up to three passes:
//!
//! ```text
//! 3-D pass   [colour, depth]  CLEAR, CLEAR  → COLOR_ATTACHMENT_OPTIMAL   only if 3-D present
//! 2-D pass   [colour]         CLEAR or LOAD → present layout             always
//! egui pass  [colour]         LOAD          → present layout             overlay only
//! ```
//!
//! A pipeline only has to be *compatible* with the pass it runs in, and
//! compatibility depends on attachment formats and sample counts — not load ops
//! or layouts. So [`Pass3d::load_2d_pass`] is the existing 2-D pass with `CLEAR`
//! swapped for `LOAD`, and every 2-D pipeline object runs in it unchanged. No 2-D
//! pipeline, pass or framebuffer is created, modified or recreated here.
//!
//! Nothing in this file runs until a 3-D stimulus exists: a pure 2-D scene never
//! queries a depth format, never allocates a depth image and never begins an
//! extra pass, so it records the same command buffer it did before 3-D existed.
//! `render_frame` makes that choice in one `match` on whether this pass was begun.
//!
//! Each pass's `initialLayout` is the previous pass's `finalLayout`, so the
//! chain is valid whichever subset of passes runs in a frame.

use ash::vk;

use crate::render::vk::buffers::find_memory_type;

/// Depth formats in order of preference. `D32_SFLOAT` is what every 3-D target
/// (Jetson Orin, desktop NVIDIA/AMD) supports, but it is not guaranteed as an
/// optimal-tiling depth attachment, so the fallback chain stays.
const DEPTH_FORMAT_CANDIDATES: [vk::Format; 3] = [
    vk::Format::D32_SFLOAT,
    vk::Format::D32_SFLOAT_S8_UINT,
    vk::Format::D24_UNORM_S8_UINT,
];

/// The first candidate usable as an optimal-tiling depth attachment, or `None`
/// when the device has none — in which case 3-D is unavailable and the server
/// keeps serving 2-D. `optimal_features` stands in for
/// `get_physical_device_format_properties(..).optimal_tiling_features`.
pub fn select_depth_format(
    optimal_features: impl Fn(vk::Format) -> vk::FormatFeatureFlags,
) -> Option<vk::Format> {
    DEPTH_FORMAT_CANDIDATES
        .into_iter()
        .find(|&f| optimal_features(f).contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT))
}

fn has_stencil(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::D32_SFLOAT_S8_UINT | vk::Format::D24_UNORM_S8_UINT
    )
}

/// Everything the 3-D pass owns. Lives on `VkContext` as `Option<Pass3d>`,
/// `None` until the first 3-D frame.
pub struct Pass3d {
    /// `[colour, depth]`.
    pub render_pass: vk::RenderPass,
    /// The 2-D pass with `LOAD` instead of `CLEAR` — see the module doc.
    pub load_2d_pass: vk::RenderPass,
    pub depth_format: vk::Format,
    depth_image: vk::Image,
    depth_memory: vk::DeviceMemory,
    depth_view: vk::ImageView,
    /// One per swapchain image, `[swapchain view, depth view]`. The 2-D and egui
    /// passes keep using `VkContext::framebuffers`.
    pub framebuffers: Vec<vk::Framebuffer>,
}

impl Pass3d {
    /// `Err` only when the device has no usable depth format. Logs the choice.
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        color_format: vk::Format,
        present_layout: vk::ImageLayout,
        views: &[vk::ImageView],
        extent: vk::Extent2D,
    ) -> Result<Self, &'static str> {
        let depth_format = select_depth_format(|f| unsafe {
            instance
                .get_physical_device_format_properties(physical_device, f)
                .optimal_tiling_features
        })
        .ok_or("no supported depth format")?;
        log::info!("vstimd: 3-D pass enabled, depth format {depth_format:?}");

        let render_pass = create_3d_render_pass(device, color_format, depth_format);
        let load_2d_pass = create_load_2d_render_pass(device, color_format, present_layout);
        let mut pass = Self {
            render_pass,
            load_2d_pass,
            depth_format,
            depth_image: vk::Image::null(),
            depth_memory: vk::DeviceMemory::null(),
            depth_view: vk::ImageView::null(),
            framebuffers: Vec::new(),
        };
        pass.create_targets(instance, physical_device, device, views, extent);
        Ok(pass)
    }

    /// Rebuild the depth image and framebuffers for a new swapchain. The render
    /// passes depend only on formats, which a resize does not change.
    pub fn recreate_targets(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        views: &[vk::ImageView],
        extent: vk::Extent2D,
    ) {
        self.destroy_targets(device);
        self.create_targets(instance, physical_device, device, views, extent);
    }

    fn create_targets(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        views: &[vk::ImageView],
        extent: vk::Extent2D,
    ) {
        let image = unsafe {
            device
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(self.depth_format)
                        .extent(vk::Extent3D {
                            width: extent.width,
                            height: extent.height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                    None,
                )
                .expect("failed to create depth image")
        };
        let reqs = unsafe { device.get_image_memory_requirements(image) };
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let mem_type = find_memory_type(
            &mem_props,
            reqs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .expect("no DEVICE_LOCAL memory for the depth image");
        let memory = unsafe {
            device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(mem_type),
                    None,
                )
                .expect("failed to allocate depth image memory")
        };
        unsafe {
            device
                .bind_image_memory(image, memory, 0)
                .expect("failed to bind depth image memory");
        }
        let aspect = if has_stencil(self.depth_format) {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        } else {
            vk::ImageAspectFlags::DEPTH
        };
        let view = unsafe {
            device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.depth_format)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: aspect,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                    None,
                )
                .expect("failed to create depth image view")
        };
        self.framebuffers = views
            .iter()
            .map(|&color| {
                let attachments = [color, view];
                unsafe {
                    device
                        .create_framebuffer(
                            &vk::FramebufferCreateInfo::default()
                                .render_pass(self.render_pass)
                                .attachments(&attachments)
                                .width(extent.width)
                                .height(extent.height)
                                .layers(1),
                            None,
                        )
                        .expect("failed to create 3-D framebuffer")
                }
            })
            .collect();
        self.depth_image = image;
        self.depth_memory = memory;
        self.depth_view = view;
    }

    fn destroy_targets(&mut self, device: &ash::Device) {
        unsafe {
            for fb in self.framebuffers.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            device.destroy_image_view(self.depth_view, None);
            device.destroy_image(self.depth_image, None);
            device.free_memory(self.depth_memory, None);
        }
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        self.destroy_targets(device);
        unsafe {
            device.destroy_render_pass(self.render_pass, None);
            device.destroy_render_pass(self.load_2d_pass, None);
        }
    }
}

/// `[colour, depth]`, both cleared. Colour ends in `COLOR_ATTACHMENT_OPTIMAL`,
/// which is where [`create_load_2d_render_pass`] picks it up.
fn create_3d_render_pass(
    device: &ash::Device,
    color_format: vk::Format,
    depth_format: vk::Format,
) -> vk::RenderPass {
    let attachments = [
        vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
        vk::AttachmentDescription::default()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
    ];
    let color_ref = vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let depth_ref = vk::AttachmentReference::default()
        .attachment(1)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_ref))
        .depth_stencil_attachment(&depth_ref);
    let stages = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
    let dep = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(stages)
        .dst_stage_mask(stages)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );
    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dep));
    unsafe {
        device
            .create_render_pass(&info, None)
            .expect("failed to create 3-D render pass")
    }
}

/// The 2-D pass, `LOAD` flavour: same single colour attachment and format as
/// `vk_context::create_render_pass` (so render-pass compatible with every 2-D
/// pipeline), picking the image up where the 3-D pass left it and ending in the
/// same layout the `CLEAR` flavour does.
fn create_load_2d_render_pass(
    device: &ash::Device,
    color_format: vk::Format,
    present_layout: vk::ImageLayout,
) -> vk::RenderPass {
    let attachment = vk::AttachmentDescription::default()
        .format(color_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::LOAD)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .final_layout(present_layout);
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
            .expect("failed to create 2-D load render pass")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_format_prefers_d32() {
        let all = |_| vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT;
        assert_eq!(select_depth_format(all), Some(vk::Format::D32_SFLOAT));
    }

    #[test]
    fn depth_format_falls_back_in_order() {
        let only = |ok: vk::Format| {
            move |f| {
                if f == ok {
                    vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT
                } else {
                    vk::FormatFeatureFlags::SAMPLED_IMAGE
                }
            }
        };
        assert_eq!(
            select_depth_format(only(vk::Format::D32_SFLOAT_S8_UINT)),
            Some(vk::Format::D32_SFLOAT_S8_UINT)
        );
        assert_eq!(
            select_depth_format(only(vk::Format::D24_UNORM_S8_UINT)),
            Some(vk::Format::D24_UNORM_S8_UINT)
        );
    }

    #[test]
    fn no_depth_format_means_no_3d() {
        assert_eq!(
            select_depth_format(|_| vk::FormatFeatureFlags::empty()),
            None
        );
    }
}
