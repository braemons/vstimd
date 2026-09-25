//! Mirroring the finished frame, for a rig whose optics already mirror it.
//!
//! A back-projection screen is viewed from the side opposite the projector, so
//! everything on it reads backwards. The fix belongs at the very end of the
//! frame rather than in each stimulus's geometry, for two reasons:
//!
//! * a screen mirrors *the whole image* — 3-D, 2-D, text, splats and the
//!   overlay alike — and a per-shader mirror would have to be repeated in all
//!   eight shaders and re-repeated in the ninth;
//! * 2-D stimuli and text are tessellated straight into NDC on the CPU, so
//!   there is no single transform to negate even if one wanted to.
//!
//! So: when a mirror is configured, every pass renders into an offscreen colour
//! image instead of the swapchain image, and one `vkCmdBlitImage` copies it
//! across with the source region reversed on whichever axes are mirrored.
//! Blitting a region with `x0 > x1` is how Vulkan spells a mirror, and with
//! identical formats and extents `FILTER_NEAREST` moves each pixel exactly
//! once — no resampling and nothing lost.
//!
//! **`ScreenMirror::None` allocates nothing and records nothing.** A rig that
//! does not need this runs the frame path it ran before the setting existed,
//! which is what keeps the cost of the feature zero for everybody else.
//!
//! The mirror is deliberately *after* rasterisation, so it costs nothing in
//! winding or culling: a mirror applied to the projection matrix would reverse
//! every triangle's winding and need `front_face` flipped to match.

use ash::vk;

use crate::render::vk::buffers::find_memory_type;
use crate::system_info::ScreenMirror;

/// One offscreen colour image per swapchain image, rendered into and then
/// mirrored across.
///
/// There is one per swapchain image rather than a single shared one because
/// frames overlap: frame N's blit may still be reading while frame N+1 begins
/// recording, and the per-image indexing already in `render_frame` keeps them
/// apart without another fence.
pub struct MirrorTarget {
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    memory: Vec<vk::DeviceMemory>,
}

impl MirrorTarget {
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        format: vk::Format,
        extent: vk::Extent2D,
        count: usize,
    ) -> Self {
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let mut images = Vec::with_capacity(count);
        let mut views = Vec::with_capacity(count);
        let mut memory = Vec::with_capacity(count);

        for _ in 0..count {
            let image = unsafe {
                device
                    .create_image(
                        &vk::ImageCreateInfo::default()
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
                            // COLOR_ATTACHMENT because every pass draws into
                            // it; TRANSFER_SRC because the blit reads it.
                            .usage(
                                vk::ImageUsageFlags::COLOR_ATTACHMENT
                                    | vk::ImageUsageFlags::TRANSFER_SRC,
                            )
                            .sharing_mode(vk::SharingMode::EXCLUSIVE)
                            .initial_layout(vk::ImageLayout::UNDEFINED),
                        None,
                    )
                    .expect("failed to create mirror image")
            };
            let reqs = unsafe { device.get_image_memory_requirements(image) };
            let mem_type = find_memory_type(
                &mem_props,
                reqs.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("no DEVICE_LOCAL memory for the mirror image");
            let mem = unsafe {
                device
                    .allocate_memory(
                        &vk::MemoryAllocateInfo::default()
                            .allocation_size(reqs.size)
                            .memory_type_index(mem_type),
                        None,
                    )
                    .expect("failed to allocate mirror image memory")
            };
            unsafe {
                device
                    .bind_image_memory(image, mem, 0)
                    .expect("failed to bind mirror image memory");
            }
            let view = unsafe {
                device
                    .create_image_view(
                        &vk::ImageViewCreateInfo::default()
                            .image(image)
                            .view_type(vk::ImageViewType::TYPE_2D)
                            .format(format)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            }),
                        None,
                    )
                    .expect("failed to create mirror image view")
            };
            images.push(image);
            views.push(view);
            memory.push(mem);
        }

        Self { images, views, memory }
    }

    /// Records the mirrored copy of `index`'s offscreen image into
    /// `swapchain_image`, leaving the swapchain image in `present_layout`.
    ///
    /// The offscreen image arrives in `TRANSFER_SRC_OPTIMAL` already — that is
    /// the final layout every pass was built with when mirroring is on — so
    /// only the destination needs a barrier in.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn cmd_blit(
        &self,
        device: &ash::Device,
        cb: vk::CommandBuffer,
        index: usize,
        swapchain_image: vk::Image,
        extent: vk::Extent2D,
        mirror: ScreenMirror,
        present_layout: vk::ImageLayout,
    ) {
        let range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };

        // UNDEFINED as the old layout: the blit overwrites every pixel, so the
        // previous contents are not worth preserving and discarding them lets
        // the driver skip a decompress.
        let to_dst = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(swapchain_image)
            .subresource_range(range);

        unsafe {
            device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&to_dst),
            );
        }

        // The mirror, spelled as a reversed source region: for a flipped axis
        // the offsets run high→low, so the blit reads it backwards.
        let (x0, x1) = if mirror.flips_x() {
            (extent.width as i32, 0)
        } else {
            (0, extent.width as i32)
        };
        let (y0, y1) = if mirror.flips_y() {
            (extent.height as i32, 0)
        } else {
            (0, extent.height as i32)
        };
        let layers = vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        };
        let region = vk::ImageBlit::default()
            .src_subresource(layers)
            .src_offsets([
                vk::Offset3D { x: x0, y: y0, z: 0 },
                vk::Offset3D { x: x1, y: y1, z: 1 },
            ])
            .dst_subresource(layers)
            .dst_offsets([
                vk::Offset3D { x: 0, y: 0, z: 0 },
                vk::Offset3D {
                    x: extent.width as i32,
                    y: extent.height as i32,
                    z: 1,
                },
            ]);

        unsafe {
            device.cmd_blit_image(
                cb,
                self.images[index],
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                swapchain_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&region),
                // Same format, same extent, whole pixels: NEAREST moves each
                // one exactly once. LINEAR here would soften every edge for
                // nothing.
                vk::Filter::NEAREST,
            );
        }

        let to_present = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::MEMORY_READ)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(present_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(swapchain_image)
            .subresource_range(range);

        unsafe {
            device.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&to_present),
            );
        }
    }

    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            for &view in &self.views {
                device.destroy_image_view(view, None);
            }
            for &image in &self.images {
                device.destroy_image(image, None);
            }
            for &mem in &self.memory {
                device.free_memory(mem, None);
            }
        }
        self.views.clear();
        self.images.clear();
        self.memory.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The blit's source offsets are the whole mirror, so they are worth
    /// pinning: a reversed axis runs high→low, an unmirrored one low→high.
    fn src_offsets(mirror: ScreenMirror, w: i32, h: i32) -> ((i32, i32), (i32, i32)) {
        let x = if mirror.flips_x() { (w, 0) } else { (0, w) };
        let y = if mirror.flips_y() { (h, 0) } else { (0, h) };
        (x, y)
    }

    #[test]
    fn horizontal_reverses_x_and_leaves_y_alone() {
        assert_eq!(src_offsets(ScreenMirror::Horizontal, 1920, 1080), ((1920, 0), (0, 1080)));
    }

    #[test]
    fn vertical_reverses_y_and_leaves_x_alone() {
        assert_eq!(src_offsets(ScreenMirror::Vertical, 1920, 1080), ((0, 1920), (1080, 0)));
    }

    #[test]
    fn both_reverses_each_axis() {
        assert_eq!(src_offsets(ScreenMirror::Both, 1920, 1080), ((1920, 0), (1080, 0)));
    }

    #[test]
    fn none_is_the_identity_region() {
        assert_eq!(src_offsets(ScreenMirror::None, 1920, 1080), ((0, 1920), (0, 1080)));
    }

    /// `None` must stay a true no-op — it is what decides whether any of this
    /// is allocated or recorded at all.
    #[test]
    fn only_none_is_identity() {
        assert!(ScreenMirror::None.is_identity());
        for m in [ScreenMirror::Horizontal, ScreenMirror::Vertical, ScreenMirror::Both] {
            assert!(!m.is_identity(), "{m:?} claimed to be the identity");
        }
    }

    #[test]
    fn backprojection_is_a_spelling_of_horizontal() {
        assert_eq!(ScreenMirror::parse_pref("backprojection"), Ok(ScreenMirror::Horizontal));
        assert_eq!(ScreenMirror::parse_pref("back-projection"), Ok(ScreenMirror::Horizontal));
        assert_eq!(ScreenMirror::parse_pref("BackProjection"), Ok(ScreenMirror::Horizontal));
        assert_eq!(ScreenMirror::parse_pref("horizontal"), Ok(ScreenMirror::Horizontal));
        assert_eq!(ScreenMirror::parse_pref("none"), Ok(ScreenMirror::None));
        assert!(ScreenMirror::parse_pref("sideways").is_err());
    }
}
