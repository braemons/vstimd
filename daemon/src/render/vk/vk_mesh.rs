use crate::render::vk::buffers::alloc_upload_bytes;

pub struct VkMesh {
    pub vertex_buffer: ash::vk::Buffer,
    pub vertex_memory: ash::vk::DeviceMemory,
    pub index_buffer: ash::vk::Buffer,
    pub index_memory: ash::vk::DeviceMemory,
    pub index_count: u32,
}

impl VkMesh {
    pub fn from_raw(
        vertex_buffer: ash::vk::Buffer,
        vertex_memory: ash::vk::DeviceMemory,
        index_buffer: ash::vk::Buffer,
        index_memory: ash::vk::DeviceMemory,
        index_count: u32,
    ) -> Self {
        Self {
            vertex_buffer,
            vertex_memory,
            index_buffer,
            index_memory,
            index_count,
        }
    }

    pub unsafe fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_buffer(self.vertex_buffer, None);
            device.free_memory(self.vertex_memory, None);
            device.destroy_buffer(self.index_buffer, None);
            device.free_memory(self.index_memory, None);
        }
    }
}

impl VkMesh {
    /// The unit quad `[-1, 1]²`, in host-visible memory.
    ///
    /// Shared by every pipeline that draws a screen-space rectangle from push
    /// constants rather than from geometry — the grating patch, and one dot of a
    /// dot field, instanced. They had a private copy each before this existed.
    pub fn unit_quad(
        device: &ash::Device,
        mem_props: &ash::vk::PhysicalDeviceMemoryProperties,
    ) -> Self {
        let n = [0.0f32, 0.0, 1.0];
        let uv = [0.0f32; 2];
        let corner = |x: f32, y: f32| crate::render::Vertex {
            position: [x, y, 0.0],
            normal: n,
            uv,
            color: crate::Color::TRANSPARENT,
        };
        let verts = [
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ];
        let idxs: [u32; 6] = [0, 1, 2, 0, 2, 3];
        let (vb, vm) = alloc_upload_bytes(
            mem_props,
            device,
            ash::vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(&verts),
        );
        let (ib, im) = alloc_upload_bytes(
            mem_props,
            device,
            ash::vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(&idxs),
        );
        Self::from_raw(vb, vm, ib, im, idxs.len() as u32)
    }
}
