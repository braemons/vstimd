use crate::geom::Vertex;
use crate::render::vk::VkMesh;
use crate::render::vk::buffers::alloc_upload_bytes;
use ash::vk;
use std::collections::HashMap;

/// GPU mesh cache for the solid triangle-list pipeline (shape stimuli and the
/// photodiode indicator).  Gratings use their own shared quad.
pub struct SolidMeshCache {
    pub fill_meshes: HashMap<u32, VkMesh>,
    pub stroke_meshes: HashMap<u32, VkMesh>,
    mem_props: vk::PhysicalDeviceMemoryProperties,
}

impl SolidMeshCache {
    pub fn new(instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> Self {
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        Self {
            fill_meshes: HashMap::new(),
            stroke_meshes: HashMap::new(),
            mem_props,
        }
    }

    /// Upload fill and stroke geometry for a handle, replacing any existing
    /// buffers.  Passing empty vertex slices for either skips that upload and
    /// removes the old mesh.
    pub fn upload(
        &mut self,
        handle: u32,
        device: &ash::Device,
        fill: (&[Vertex], &[u32]),
        stroke: (&[Vertex], &[u32]),
    ) {
        Self::upload_mesh(
            &mut self.fill_meshes,
            &self.mem_props,
            handle,
            device,
            fill.0,
            fill.1,
        );
        Self::upload_mesh(
            &mut self.stroke_meshes,
            &self.mem_props,
            handle,
            device,
            stroke.0,
            stroke.1,
        );
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        for mesh in self.fill_meshes.values() {
            unsafe { mesh.destroy(device) };
        }
        self.fill_meshes.clear();
        for mesh in self.stroke_meshes.values() {
            unsafe { mesh.destroy(device) };
        }
        self.stroke_meshes.clear();
    }

    /// Overwrite vertex data in the fill buffer for `handle` without
    /// reallocation.  The new slice must be the same byte size as the original.
    /// Used for the photodiode's colour-only updates.
    pub fn overwrite_fill_vertices(&self, handle: u32, device: &ash::Device, verts: &[Vertex]) {
        let Some(mesh) = self.fill_meshes.get(&handle) else {
            return;
        };
        let data: &[u8] = bytemuck::cast_slice(verts);
        unsafe {
            let ptr = device
                .map_memory(
                    mesh.vertex_memory,
                    0,
                    data.len() as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )
                .expect("map") as *mut u8;
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            device.unmap_memory(mesh.vertex_memory);
        }
    }

    fn upload_mesh(
        map: &mut HashMap<u32, VkMesh>,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
        handle: u32,
        device: &ash::Device,
        verts: &[Vertex],
        idxs: &[u32],
    ) {
        if let Some(old) = map.remove(&handle) {
            unsafe { old.destroy(device) };
        }
        if verts.is_empty() || idxs.is_empty() {
            return;
        }
        let (vb, vm) = alloc_upload_bytes(
            mem_props,
            device,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(verts),
        );
        let (ib, im) = alloc_upload_bytes(
            mem_props,
            device,
            vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(idxs),
        );
        map.insert(handle, VkMesh::from_raw(vb, vm, ib, im, idxs.len() as u32));
    }
}
