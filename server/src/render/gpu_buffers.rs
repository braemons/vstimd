use std::collections::HashMap;

use wgpu::util::DeviceExt as _;

use super::Vertex;

pub struct StimulusMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

pub struct GpuBuffers {
    pub meshes: HashMap<u32, StimulusMesh>,
}

impl GpuBuffers {
    pub fn new() -> Self {
        Self { meshes: HashMap::new() }
    }

    pub fn upload(&mut self, handle: u32, device: &wgpu::Device, verts: &[Vertex], idxs: &[u32]) {
        if verts.is_empty() {
            self.meshes.remove(&handle);
            return;
        }
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(idxs),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes
            .insert(handle, StimulusMesh { vertex_buffer: vb, index_buffer: ib, index_count: idxs.len() as u32 });
    }
}
