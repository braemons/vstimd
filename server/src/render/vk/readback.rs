//! A host-visible staging buffer that `render_frame` copies the rendered
//! swapchain image into, before the image is presented.
//!
//! Two callers want the finished frame on the CPU, for unrelated reasons:
//!
//! * the **evdi** backend, every frame — the pixels *are* the output, handed
//!   straight to `EvdiOutput::present`;
//! * **screenshots**, on a keypress — see [`crate::render::screenshot`].
//!
//! Both need the same thing, and both need it in the same narrow window.
//! Reading `ctx.swapchain_images` after present is undefined behaviour (the
//! presentation engine owns the image until the next `acquire_next_image`),
//! so the copy has to be recorded into the render's own command buffer.
//! `render_frame` does that when handed a [`ReadbackTarget`], and waits for it
//! to land before returning — which is what makes [`Readback::frame`] current
//! rather than racy.

use ash::vk;

use crate::render::ReadbackTarget;
use crate::render::vk::VkContext;

/// A persistent host-visible staging buffer sized to one frame.
///
/// `pitch` is the row stride in **bytes**, which is not always a tightly
/// packed `width * 4`: evdi dictates its own stride, and a readback handed to
/// it must match. A screenshot has no such constraint and passes `width * 4`.
pub struct Readback {
    device: ash::Device,
    memory: vk::DeviceMemory,
    mapped: *mut u8,
    size: usize,
    pub target: ReadbackTarget,
}

impl Readback {
    pub fn new(ctx: &VkContext, pitch: usize, height: u32) -> Self {
        let device = ctx.device.clone();
        let size = pitch * height as usize;

        let buffer = unsafe {
            device
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size as vk::DeviceSize)
                        .usage(vk::BufferUsageFlags::TRANSFER_DST)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .expect("failed to create readback buffer")
        };
        let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let mem_props =
            unsafe { ctx.instance.get_physical_device_memory_properties(ctx.physical_device) };
        let mem_type = (0..mem_props.memory_type_count)
            .find(|&i| {
                (reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize].property_flags.contains(
                        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    )
            })
            .expect("no HOST_VISIBLE|HOST_COHERENT memory for readback buffer");
        let memory = unsafe {
            device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(mem_type),
                    None,
                )
                .expect("failed to allocate readback memory")
        };
        unsafe {
            device.bind_buffer_memory(buffer, memory, 0).expect("bind readback memory");
        }
        let mapped = unsafe {
            device
                .map_memory(memory, 0, size as vk::DeviceSize, vk::MemoryMapFlags::empty())
                .expect("failed to map readback memory") as *mut u8
        };

        Self {
            device,
            memory,
            mapped,
            size,
            target: ReadbackTarget {
                buffer,
                row_length_texels: (pitch / 4) as u32,
            },
        }
    }

    /// A view of the staging buffer as of the last `render_frame` call that
    /// was given `Some(&self.target)`. `render_frame` waits for its copy to
    /// land on the GPU before returning, so this is always current.
    pub fn frame(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.mapped, self.size) }
    }
}

impl Drop for Readback {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.target.buffer, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
