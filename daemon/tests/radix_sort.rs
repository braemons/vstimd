//! The GPU radix sort, against a CPU reference.
//!
//! Runs headless on whatever Vulkan device is present — a real GPU, or llvmpipe
//! on a machine with no display — so the piece of the tile rasteriser most
//! likely to be subtly wrong can be checked without a renderer, a window, or a
//! rig. Skips (rather than fails) where there is no Vulkan at all, so a
//! container without a driver does not turn into a red build.
//!
//! What is actually asserted is **stability**, not just sortedness: the tile
//! rasteriser depends on equal keys keeping their input order, because that is
//! what carries each tile's depth order without depth entering the key. A sort
//! that merely produced sorted output would pass a naive test and give a
//! stimulus that flickers.

use ash::vk;
use vstimd::render::vk_radix_sort::RadixSort;

/// Minimal headless Vulkan: instance, a device with a compute queue, and a
/// command pool. No surface, no swapchain, no extensions.
struct Compute {
    _entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,
    queue: vk::Queue,
    pool: vk::CommandPool,
    mem_props: vk::PhysicalDeviceMemoryProperties,
}

impl Compute {
    fn new() -> Option<Self> {
        let entry = unsafe { ash::Entry::load().ok()? };
        let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);
        let instance = unsafe {
            entry
                .create_instance(&vk::InstanceCreateInfo::default().application_info(&app), None)
                .ok()?
        };
        let physical = unsafe { instance.enumerate_physical_devices().ok()? };
        let (pd, qf) = physical.iter().find_map(|&pd| {
            let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
            families
                .iter()
                .position(|p| p.queue_flags.contains(vk::QueueFlags::COMPUTE))
                .map(|i| (pd, i as u32))
        })?;
        let priorities = [1.0f32];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(qf)
            .queue_priorities(&priorities);
        let device = unsafe {
            instance
                .create_device(
                    pd,
                    &vk::DeviceCreateInfo::default()
                        .queue_create_infos(std::slice::from_ref(&queue_info)),
                    None,
                )
                .ok()?
        };
        let queue = unsafe { device.get_device_queue(qf, 0) };
        let pool = unsafe {
            device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default().queue_family_index(qf),
                    None,
                )
                .ok()?
        };
        let mem_props = unsafe { instance.get_physical_device_memory_properties(pd) };
        Some(Self { _entry: entry, instance, device, queue, pool, mem_props })
    }

    /// A host-visible storage buffer holding `data`, so the test can write it
    /// and read it back without staging.
    fn buffer(&self, data: &[u32]) -> (vk::Buffer, vk::DeviceMemory, *mut u32) {
        let size = (data.len().max(1) * 4) as vk::DeviceSize;
        unsafe {
            let buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size)
                        .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .expect("create buffer");
            let reqs = self.device.get_buffer_memory_requirements(buffer);
            let flags = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let mem_type = (0..self.mem_props.memory_type_count)
                .find(|&i| {
                    (reqs.memory_type_bits & (1 << i)) != 0
                        && self.mem_props.memory_types[i as usize].property_flags.contains(flags)
                })
                .expect("host-visible memory");
            let memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(mem_type),
                    None,
                )
                .expect("allocate");
            self.device.bind_buffer_memory(buffer, memory, 0).unwrap();
            let mapped = self
                .device
                .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
                .expect("map") as *mut u32;
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len());
            (buffer, memory, mapped)
        }
    }

    /// Record `f` into a one-shot command buffer, submit it, and wait.
    fn run(&self, f: impl FnOnce(vk::CommandBuffer)) {
        unsafe {
            let cb = self
                .device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(self.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .expect("allocate cb")[0];
            self.device
                .begin_command_buffer(
                    cb,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
            f(cb);
            self.device.end_command_buffer(cb).unwrap();
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .unwrap();
            let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cb));
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit), fence)
                .unwrap();
            self.device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(self.pool, &[cb]);
        }
    }
}

impl Drop for Compute {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

/// Deterministic pseudo-random keys, so a failure is reproducible.
fn keys_for(n: usize, distinct: u32, seed: u64) -> Vec<u32> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as u32) % distinct
        })
        .collect()
}

/// Sort `(key, value)` on the GPU and return the pairs it produced.
fn gpu_sort(c: &Compute, keys: &[u32], vals: &[u32], key_bits: u32) -> Vec<(u32, u32)> {
    let n = keys.len() as u32;
    let (kb, km, kmap) = c.buffer(keys);
    let (vb, vm, vmap) = c.buffer(vals);
    let sort = RadixSort::new(&c.device, &c.mem_props, kb, vb, n.max(1), key_bits);
    c.run(|cb| unsafe { sort.record(&c.device, cb, n) });

    let out: Vec<(u32, u32)> = unsafe {
        (0..keys.len())
            .map(|i| (*kmap.add(i), *vmap.add(i)))
            .collect()
    };
    unsafe {
        c.device.device_wait_idle().unwrap();
        sort.destroy(&c.device);
        c.device.destroy_buffer(kb, None);
        c.device.free_memory(km, None);
        c.device.destroy_buffer(vb, None);
        c.device.free_memory(vm, None);
    }
    out
}

/// What a stable sort by key must produce.
fn cpu_reference(keys: &[u32], vals: &[u32]) -> Vec<(u32, u32)> {
    let mut pairs: Vec<(u32, u32)> = keys.iter().copied().zip(vals.iter().copied()).collect();
    pairs.sort_by_key(|&(k, _)| k); // Rust's sort is stable, which is the point
    pairs
}

fn check(c: &Compute, n: usize, distinct: u32, key_bits: u32, seed: u64) {
    let keys = keys_for(n, distinct, seed);
    // Values are the input positions, so the assert below reads directly as
    // "equal keys kept their input order".
    let vals: Vec<u32> = (0..n as u32).collect();
    let got = gpu_sort(c, &keys, &vals, key_bits);
    let want = cpu_reference(&keys, &vals);
    assert_eq!(
        got, want,
        "n={n} distinct={distinct} key_bits={key_bits}: GPU sort disagrees with a stable CPU sort"
    );
}

#[test]
fn sorts_stably_against_a_cpu_reference() {
    let Some(c) = Compute::new() else {
        eprintln!("no Vulkan device — skipping the GPU radix-sort test");
        return;
    };

    // Exactly one block, and one block plus a partial one: the tail is where
    // padding has to stay behind the live items sharing its digit.
    check(&c, 256, 16, 16, 1);
    check(&c, 257, 16, 16, 2);
    check(&c, 1000, 16, 16, 3);

    // Many duplicate keys is the case stability is about: with 14 400 tiles and
    // a million splats, every tile's key repeats hundreds of times.
    check(&c, 100_000, 64, 16, 4);
    check(&c, 100_000, 14_400, 16, 5);

    // One key for everything, and all keys distinct — the two ends.
    check(&c, 5_000, 1, 16, 6);
    check(&c, 5_000, 65_535, 16, 7);

    // A single pair, and a block boundary exactly.
    check(&c, 1, 16, 16, 8);
    check(&c, 512, 256, 16, 9);
}

#[test]
fn sorts_a_full_32_bit_key() {
    let Some(c) = Compute::new() else {
        eprintln!("no Vulkan device — skipping the GPU radix-sort test");
        return;
    };
    // The tile rasteriser only needs 16 bits, but the sort is the general
    // primitive, so the wide key has to work too.
    check(&c, 50_000, u32::MAX, 32, 11);
}
