use crate::render::vk::VkMesh;
use crate::render::vk::buffers::alloc_upload_bytes;
use std::collections::HashMap;

/// Per-stimulus `TextVertex` quad buffers, keyed by stimulus handle.
/// Rebuilt when the stimulus is dirty; reused across frames otherwise.
pub struct TextMeshCache {
    pub meshes: HashMap<u32, VkMesh>,
    mem_props: ash::vk::PhysicalDeviceMemoryProperties,
}

impl TextMeshCache {
    pub fn new(instance: &ash::Instance, physical_device: ash::vk::PhysicalDevice) -> Self {
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        Self {
            meshes: HashMap::new(),
            mem_props,
        }
    }

    /// Upload glyph quad vertices for `handle`, replacing any existing buffers.
    /// `vert_bytes` is a `bytemuck::cast_slice` of `&[TextVertex]`.
    pub fn upload(&mut self, handle: u32, device: &ash::Device, vert_bytes: &[u8], idxs: &[u32]) {
        if let Some(old) = self.meshes.remove(&handle) {
            unsafe { old.destroy(device) };
        }
        if vert_bytes.is_empty() || idxs.is_empty() {
            return;
        }
        let (vb, vm) = alloc_upload_bytes(
            &self.mem_props,
            device,
            ash::vk::BufferUsageFlags::VERTEX_BUFFER,
            vert_bytes,
        );
        let (ib, im) = alloc_upload_bytes(
            &self.mem_props,
            device,
            ash::vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(idxs),
        );
        self.meshes
            .insert(handle, VkMesh::from_raw(vb, vm, ib, im, idxs.len() as u32));
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        for mesh in self.meshes.values() {
            unsafe { mesh.destroy(device) };
        }
        self.meshes.clear();
    }
}
