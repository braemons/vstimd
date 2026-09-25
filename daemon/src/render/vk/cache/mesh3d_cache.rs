//! Shared unit meshes for 3-D stimuli, keyed by geometry — not by stimulus
//! handle (`dev/3D_ROADMAP.md` §1.6).
//!
//! Every other mesh cache here keys by handle, because 2-D vertices are baked
//! per stimulus. 3-D vertices are unit primitives and size, placement and colour
//! are per-draw constants, so fifty spheres of fifty sizes share one mesh. A
//! handle-keyed cache would make a corridor's VRAM and upload cost grow with its
//! tile count.
//!
//! Meshes live in device-local memory: they are uploaded once per [`MeshKey`]
//! and never rewritten. A screen resize does not touch them.

use std::collections::HashMap;

use ash::vk;

use crate::render::vk::VkMesh;
use crate::render::vk::buffers::upload_device_local;
use crate::scene::stimulus::MeshKey;

pub struct Mesh3dCache {
    mem_props: vk::PhysicalDeviceMemoryProperties,
    meshes: HashMap<MeshKey, VkMesh>,
    /// Reused across [`sync`](Self::sync) calls so the per-frame key set does not
    /// allocate once it has grown to the scene's size.
    referenced: Vec<MeshKey>,
}

impl Mesh3dCache {
    pub fn new(instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> Self {
        Self {
            mem_props: unsafe { instance.get_physical_device_memory_properties(physical_device) },
            meshes: HashMap::new(),
            referenced: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.meshes.len()
    }

    /// Make the cache hold exactly the meshes `keys` references: upload each
    /// missing one, free each no longer referenced.
    ///
    /// An upload blocks until the GPU has the mesh, so the frame a new geometry
    /// first appears on pays for it. It happens once per distinct geometry.
    pub fn sync(
        &mut self,
        device: &ash::Device,
        pool: vk::CommandPool,
        queue: vk::Queue,
        keys: impl Iterator<Item = MeshKey>,
    ) {
        self.referenced.clear();
        self.referenced.extend(keys);
        for &key in &self.referenced {
            if self.meshes.contains_key(&key) {
                continue;
            }
            let (verts, idxs) = crate::render::tess3d::unit_mesh(key);
            let upload = |usage, bytes: &[u8]| {
                upload_device_local(&self.mem_props, device, pool, queue, usage, bytes)
            };
            let (vb, vm) = upload(
                vk::BufferUsageFlags::VERTEX_BUFFER,
                bytemuck::cast_slice(&verts),
            );
            let (ib, im) = upload(
                vk::BufferUsageFlags::INDEX_BUFFER,
                bytemuck::cast_slice(&idxs),
            );
            log::debug!(
                "vstimd: uploaded 3-D mesh {key:?} ({} vertices)",
                verts.len()
            );
            self.meshes
                .insert(key, VkMesh::from_raw(vb, vm, ib, im, idxs.len() as u32));
        }
        let referenced = &self.referenced;
        self.meshes.retain(|key, mesh| {
            let keep = referenced.contains(key);
            if !keep {
                unsafe { mesh.destroy(device) };
            }
            keep
        });
    }

    pub fn get(&self, key: MeshKey) -> Option<&VkMesh> {
        self.meshes.get(&key)
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        for (_, mesh) in self.meshes.drain() {
            unsafe { mesh.destroy(device) };
        }
    }
}
