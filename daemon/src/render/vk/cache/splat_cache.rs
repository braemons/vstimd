//! GPU state for Gaussian splat stimuli, keyed by stimulus handle.
//!
//! Per stimulus, three stages:
//! 1. **Loading** — the file is read on its own thread (`splat::load`), since a
//!    scene is tens to hundreds of megabytes. Nothing draws.
//! 2. **Ready** — the splats sit in a device-local storage buffer, uploaded once
//!    (the frame that uploads pays for it, like a new 3-D mesh). A
//!    [`SplatSorter`] keeps the back-to-front order current on its own thread;
//!    each frame slot has a host-visible order buffer that is refreshed whenever
//!    a newer order is ready.
//! 3. **Failed** — logged once and never retried for that path.
//!
//! Keyed by handle rather than path: two stimuli on one file load it twice.
//! That is the interim cost of path-addressed files; the asset store replaces
//! the key (`dev/design/GAUSSIAN_SPLAT_PLAN.md` §5).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;

use ash::vk;
use glam::Mat4;

use crate::render::vk::buffers::{find_memory_type, upload_device_local};
use crate::splat::{GpuSplat, SplatSorter};

struct OrderBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped: *mut u32,
    /// Generation of the sorter's order last copied in; 0 = the identity order.
    generation: u64,
}

pub struct SplatCloud {
    pub count: u32,
    splat_buffer: vk::Buffer,
    splat_memory: vk::DeviceMemory,
    orders: Vec<OrderBuffer>,
    pool: vk::DescriptorPool,
    /// One per frame slot.
    pub sets: Vec<vk::DescriptorSet>,
    sorter: SplatSorter,
}

enum Entry {
    Loading(mpsc::Receiver<Result<Vec<GpuSplat>, String>>),
    Ready(SplatCloud),
    Failed,
}

struct Slot {
    path: String,
    entry: Entry,
}

pub struct SplatCache {
    mem_props: vk::PhysicalDeviceMemoryProperties,
    frames_in_flight: usize,
    slots: HashMap<u32, Slot>,
}

impl SplatCache {
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        frames_in_flight: usize,
    ) -> Self {
        Self {
            mem_props: unsafe { instance.get_physical_device_memory_properties(physical_device) },
            frames_in_flight,
            slots: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The cloud ready to draw for `handle`, if its file has loaded.
    pub fn get(&self, handle: u32) -> Option<&SplatCloud> {
        match self.slots.get(&handle)?.entry {
            Entry::Ready(ref cloud) => Some(cloud),
            _ => None,
        }
    }

    /// Follow the scene's splat stimuli: start loading new ones, upload finished
    /// loads, drop removed ones, and for every ready cloud post the view axis to
    /// its sorter and copy in the newest order for `frame_slot`.
    ///
    /// `stimuli` yields `(handle, path, model_view)` for every splat stimulus in
    /// the scene, visible or not, so a hidden one keeps its upload.
    #[allow(clippy::too_many_arguments)]
    pub fn sync<'a>(
        &mut self,
        device: &ash::Device,
        pool: vk::CommandPool,
        queue: vk::Queue,
        set_layout: vk::DescriptorSetLayout,
        frame_slot: usize,
        stimuli: impl Iterator<Item = (u32, &'a str, Mat4)>,
        present: &mut Vec<u32>,
    ) {
        present.clear();
        for (handle, path, model_view) in stimuli {
            present.push(handle);
            let slot = self.slots.get(&handle).filter(|s| s.path == path);
            if slot.is_none() {
                if let Some(mut old) = self.slots.remove(&handle) {
                    old.destroy(device);
                }
                self.slots.insert(handle, Slot { path: path.to_owned(), entry: spawn_load(path) });
            }
            let slot = self.slots.get_mut(&handle).expect("inserted above");

            if let Entry::Loading(rx) = &slot.entry {
                match rx.try_recv() {
                    Ok(Ok(splats)) => {
                        let t0 = std::time::Instant::now();
                        let cloud = upload(
                            &self.mem_props,
                            self.frames_in_flight,
                            device,
                            pool,
                            queue,
                            set_layout,
                            splats,
                        );
                        log::info!(
                            "vstimd: uploaded {} splats from {} in {} ms",
                            cloud.count,
                            slot.path,
                            t0.elapsed().as_millis()
                        );
                        slot.entry = Entry::Ready(cloud);
                    }
                    Ok(Err(e)) => {
                        log::error!("vstimd: cannot load splats: {e}");
                        slot.entry = Entry::Failed;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        log::error!("vstimd: splat loader for {} died", slot.path);
                        slot.entry = Entry::Failed;
                    }
                }
            }

            if let Entry::Ready(cloud) = &mut slot.entry {
                // Row 2 of the model-view's linear part: view-space depth per
                // cloud-space unit, which is all the order depends on.
                let m = model_view;
                cloud.sorter.request(glam::Vec3::new(m.x_axis.z, m.y_axis.z, m.z_axis.z));
                let order = &mut cloud.orders[frame_slot];
                // SAFETY: mapped for the cloud's lifetime, `count` u32s long, and
                // this slot's previous frame has finished (its fence was waited on).
                let dst = unsafe { std::slice::from_raw_parts_mut(order.mapped, cloud.count as usize) };
                if let Some(generation) = cloud.sorter.copy_latest(order.generation, dst) {
                    order.generation = generation;
                }
            }
        }

        self.slots.retain(|handle, slot| {
            let keep = present.contains(handle);
            if !keep {
                slot.destroy(device);
            }
            keep
        });
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        for (_, mut slot) in self.slots.drain() {
            slot.destroy(device);
        }
    }
}

impl Slot {
    /// Free the GPU half. Leaves the slot `Failed`; callers drop it.
    fn destroy(&mut self, device: &ash::Device) {
        if let Entry::Ready(cloud) = std::mem::replace(&mut self.entry, Entry::Failed) {
            unsafe {
                for o in &cloud.orders {
                    device.unmap_memory(o.memory);
                    device.destroy_buffer(o.buffer, None);
                    device.free_memory(o.memory, None);
                }
                device.destroy_descriptor_pool(cloud.pool, None);
                device.destroy_buffer(cloud.splat_buffer, None);
                device.free_memory(cloud.splat_memory, None);
            }
            // Dropping the sorter stops its thread without waiting for it.
        }
        // A load still running finishes into a closed channel and exits.
    }
}

fn spawn_load(path: &str) -> Entry {
    let (tx, rx) = mpsc::channel();
    let path = path.to_owned();
    let spawned = std::thread::Builder::new().name("vstimd-splat-load".into()).spawn(move || {
        let t0 = std::time::Instant::now();
        let result = crate::splat::load(std::path::Path::new(&path)).map_err(|e| e.to_string());
        if let Ok(splats) = &result {
            log::info!(
                "vstimd: read {} splats from {path} in {} ms",
                splats.len(),
                t0.elapsed().as_millis()
            );
        }
        let _ = tx.send(result);
    });
    match spawned {
        Ok(_) => Entry::Loading(rx),
        Err(e) => {
            log::error!("vstimd: cannot start the splat loader: {e}");
            Entry::Failed
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn upload(
mem_props: &vk::PhysicalDeviceMemoryProperties,
frames_in_flight: usize,
    device: &ash::Device,
    pool: vk::CommandPool,
    queue: vk::Queue,
    set_layout: vk::DescriptorSetLayout,
    splats: Vec<GpuSplat>,
) -> SplatCloud {
    let count = splats.len() as u32;
    let (splat_buffer, splat_memory) = upload_device_local(
        mem_props,
        device,
        pool,
        queue,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        bytemuck::cast_slice(&splats),
    );

    let order_bytes = vk::DeviceSize::from(count.max(1)) * 4;
    let orders: Vec<OrderBuffer> = (0..frames_in_flight)
        .map(|_| {
            let (buffer, memory, mapped) = alloc_mapped(mem_props, device, order_bytes);
            // Identity until the first sort lands: drawable, just unsorted.
            let dst = unsafe { std::slice::from_raw_parts_mut(mapped, count as usize) };
            for (i, v) in dst.iter_mut().enumerate() {
                *v = i as u32;
            }
            OrderBuffer { buffer, memory, mapped, generation: 0 }
        })
        .collect();

    let slots = frames_in_flight as u32;
    let pool_size = vk::DescriptorPoolSize {
        ty: vk::DescriptorType::STORAGE_BUFFER,
        descriptor_count: 2 * slots,
    };
    let descriptor_pool = unsafe {
        device
            .create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(slots)
                    .pool_sizes(std::slice::from_ref(&pool_size)),
                None,
            )
            .expect("failed to create splat descriptor pool")
    };
    let layouts = vec![set_layout; frames_in_flight];
    let sets = unsafe {
        device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&layouts),
            )
            .expect("failed to allocate splat descriptor sets")
    };
    for (&set, order) in sets.iter().zip(&orders) {
        let splat_info = [vk::DescriptorBufferInfo::default()
            .buffer(splat_buffer)
            .range(vk::WHOLE_SIZE)];
        let order_info = [vk::DescriptorBufferInfo::default()
            .buffer(order.buffer)
            .range(vk::WHOLE_SIZE)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&splat_info),
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&order_info),
        ];
        unsafe { device.update_descriptor_sets(&writes, &[]) };
    }

    SplatCloud {
        count,
        splat_buffer,
        splat_memory,
        orders,
        pool: descriptor_pool,
        sets,
        sorter: SplatSorter::spawn(Arc::from(splats)),
    }
}

fn alloc_mapped(
mem_props: &vk::PhysicalDeviceMemoryProperties,
    device: &ash::Device,
    size: vk::DeviceSize,
) -> (vk::Buffer, vk::DeviceMemory, *mut u32) {
    unsafe {
        let buffer = device
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .expect("failed to create splat order buffer");
        let reqs = device.get_buffer_memory_requirements(buffer);
        let mem_type = find_memory_type(
            mem_props,
            reqs.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("no HOST_VISIBLE|HOST_COHERENT memory for the splat order");
        let memory = device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(reqs.size)
                    .memory_type_index(mem_type),
                None,
            )
            .expect("failed to allocate splat order memory");
        device.bind_buffer_memory(buffer, memory, 0).unwrap();
        let mapped = device
            .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
            .expect("failed to map splat order") as *mut u32;
        (buffer, memory, mapped)
    }
}
