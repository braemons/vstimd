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
