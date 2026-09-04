//! Per-stimulus dot instance buffers.
//!
//! A dot field is the one stimulus whose GPU data changes *every* frame: the
//! positions move, so there is nothing to cache in the sense the mesh caches mean
//! it. What is cached is the allocation. Each field gets a persistently mapped,
//! host-coherent buffer sized by its dot count, and the per-frame update is a
//! memcpy into it — no allocation, no map/unmap, nothing for the render thread to
//! block on.
//!
//! **One buffer per frame in flight.** The renderer keeps several frames in flight
//! and waits only on the fence of the slot it is about to reuse, so a single buffer
//! rewritten each frame would be overwritten while the GPU was still reading the
//! previous frame's dots. Indexing by slot makes the fence already waited on at the
//! top of `render_frame` the exact guarantee this needs.

use std::collections::HashMap;

use ash::vk;

use crate::render::vk::buffers::find_memory_type;
use crate::scene::stimulus::dots::{DotInstance, Dots};

/// One field's instance buffer for one frame-in-flight slot.
pub struct DotsInstanceBuffer {
    pub buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    /// Kept mapped for the buffer's whole life. Host-coherent, so writes are
    /// visible to the device without an explicit flush.
    mapped: *mut DotInstance,
    /// Dots the allocation can hold.
    capacity: u32,
    /// Dots actually written for the current frame — fewer than the field's dot
    /// count whenever the aperture culls by dot centre.
    pub instance_count: u32,
}

impl DotsInstanceBuffer {
    fn new(
        device: &ash::Device,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
        capacity: u32,
    ) -> Self {
        let size = (capacity as usize * std::mem::size_of::<DotInstance>()) as vk::DeviceSize;
        let buffer = unsafe {
            device
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size)
                        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .expect("dots: create instance buffer")
        };
        let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let mem_type = find_memory_type(
            mem_props,
            reqs.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("dots: no HOST_VISIBLE|HOST_COHERENT memory");
        let memory = unsafe {
            device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(mem_type),
                    None,
                )
                .expect("dots: allocate instance memory")
        };
        let mapped = unsafe {
            device.bind_buffer_memory(buffer, memory, 0).unwrap();
            device
                .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
                .expect("dots: map instance buffer") as *mut DotInstance
        };
        Self { buffer, memory, mapped, capacity, instance_count: 0 }
    }

    unsafe fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.unmap_memory(self.memory);
            device.destroy_buffer(self.buffer, None);
            device.free_memory(self.memory, None);
        }
    }
}

/// The instance buffers of every dot field, per frame-in-flight slot.
pub struct DotsInstanceCache {
    /// `slots[slot][handle]`.
    slots: Vec<HashMap<u32, DotsInstanceBuffer>>,
    mem_props: vk::PhysicalDeviceMemoryProperties,
}

impl DotsInstanceCache {
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        frames_in_flight: usize,
    ) -> Self {
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        Self {
            slots: (0..frames_in_flight.max(1)).map(|_| HashMap::new()).collect(),
            mem_props,
        }
    }

    fn slot_index(&self, slot: usize) -> usize {
        slot % self.slots.len()
    }

    /// Write `dots` into `handle`'s buffer for this frame slot, growing the
    /// allocation if the dot count has risen.
    ///
    /// Safe to reallocate here: the caller has already waited on this slot's fence,
    /// so nothing the GPU is reading lives in the buffer being replaced.
    pub fn write(&mut self, handle: u32, slot: usize, device: &ash::Device, dots: &Dots) {
        let slot = self.slot_index(slot);
        let needed = dots.live_count() as u32;
        let mem_props = self.mem_props;
        let map = &mut self.slots[slot];
        let entry = map.entry(handle);
        let buf = match entry {
            std::collections::hash_map::Entry::Occupied(o) => {
                let o = o.into_mut();
                if o.capacity < needed {
                    unsafe { o.destroy(device) };
                    *o = DotsInstanceBuffer::new(device, &mem_props, needed.max(1));
                }
                o
            }
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(DotsInstanceBuffer::new(device, &mem_props, needed.max(1)))
            }
        };
        // SAFETY: `mapped` points at `capacity` instances of host-coherent memory
        // that stays mapped for the buffer's life, and `capacity >= needed`.
        let out = unsafe {
            std::slice::from_raw_parts_mut(buf.mapped, buf.capacity as usize)
        };
        buf.instance_count = dots.write_instances(out);
    }

    pub fn get(&self, handle: u32, slot: usize) -> Option<&DotsInstanceBuffer> {
        self.slots[self.slot_index(slot)].get(&handle)
    }

    /// Drop the buffers of stimuli that no longer exist.
    pub fn retain(&mut self, device: &ash::Device, keep: impl Fn(u32) -> bool) {
        for map in &mut self.slots {
            map.retain(|h, buf| {
                if keep(*h) {
                    true
                } else {
                    unsafe { buf.destroy(device) };
                    false
                }
            });
        }
    }

    pub fn destroy_all(&mut self, device: &ash::Device) {
        for map in &mut self.slots {
            for buf in map.values() {
                unsafe { buf.destroy(device) };
            }
            map.clear();
        }
    }
}
