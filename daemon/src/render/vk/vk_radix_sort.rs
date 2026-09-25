//! A stable GPU radix sort over 32-bit key/value pairs.
//!
//! Owns its pipelines and its ping-pong scratch, and nothing else: `record`
//! takes a device and a command buffer the caller is already building, so the
//! renderer drives it with its own queue and a test drives it with a headless
//! one. That is deliberate — the sort is the piece most likely to be subtly
//! wrong, so it is the piece that has to be testable without a display.
//!
//! **Stable**, which is the reason it exists rather than anything simpler. The
//! tile rasteriser sorts splats by tile id, having enumerated them in the global
//! back-to-front order `splat::SplatSorter` already maintains; a stable sort
//! therefore leaves every tile's run in depth order without depth ever entering
//! the key. An unstable sort would leave the order within a tile arbitrary, and
//! splats blend, so that is a stimulus whose pixels change between frames for no
//! reason at all.
//!
//! See `shaders/radix_sort.wgsl` for how stability is achieved in each kernel.

use ash::vk;

use super::buffers::find_memory_type;

/// Bits of key sorted per pass. Must match `RADIX_BITS` in the shader.
const RADIX_BITS: u32 = 4;
/// Buckets per pass: `1 << RADIX_BITS`. Must match `RADIX` in the shader.
const RADIX: u32 = 1 << RADIX_BITS;
/// Items per workgroup. Must match `BLOCK` in the shader and the
/// `@workgroup_size` on every entry point.
const BLOCK: u32 = 256;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Push {
    n: u32,
    num_blocks: u32,
    shift: u32,
    _pad: u32,
}

/// A buffer plus the memory behind it, freed together.
struct Owned {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

/// Sorts up to `capacity` pairs by the low `key_bits` of the key.
pub struct RadixSort {
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    histogram: vk::Pipeline,
    scan: vk::Pipeline,
    scatter: vk::Pipeline,
    pool: vk::DescriptorPool,
    /// `[a→b, b→a]`, alternated per pass so the shader never indexes a binding.
    sets: [vk::DescriptorSet; 2],
    /// Scratch the caller does not see: the second half of the ping-pong and
    /// the block histogram.
    keys_b: Owned,
    vals_b: Owned,
    hist: Owned,
    capacity: u32,
    key_bits: u32,
}

impl RadixSort {
    /// `keys_a`/`vals_a` are the caller's buffers: the sort reads them, and
    /// because it runs an even number of passes, leaves the result in them too.
    /// They must be `STORAGE_BUFFER` and hold at least `capacity` `u32`s.
    ///
    /// `key_bits` is rounded up to a whole number of passes; an odd pass count
    /// would finish in the scratch buffer instead, so it is rounded to even.
    pub fn new(
        device: &ash::Device,
        mem_props: &vk::PhysicalDeviceMemoryProperties,
        keys_a: vk::Buffer,
        vals_a: vk::Buffer,
        capacity: u32,
        key_bits: u32,
    ) -> Self {
        let passes = key_bits.div_ceil(RADIX_BITS).max(1);
        // Even, so the ping-pong ends where it started.
        let key_bits = passes.next_multiple_of(2) * RADIX_BITS;

        let storage = |binding| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
        };
        let bindings = [storage(0), storage(1), storage(2), storage(3), storage(4)];
        let set_layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .expect("failed to create radix-sort descriptor set layout")
        };
        let push = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .size(std::mem::size_of::<Push>() as u32);
        let layout = unsafe {
            device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(std::slice::from_ref(&set_layout))
                        .push_constant_ranges(std::slice::from_ref(&push)),
                    None,
                )
                .expect("failed to create radix-sort pipeline layout")
        };

        let module = shader_module(
            device,
            include_bytes!(concat!(env!("OUT_DIR"), "/radix_sort.spv")),
            "radix_sort",
        );
        let histogram = compute_pipeline(device, layout, module, c"histogram");
        let scan = compute_pipeline(device, layout, module, c"scan");
        let scatter = compute_pipeline(device, layout, module, c"scatter");
        unsafe { device.destroy_shader_module(module, None) };

        let num_blocks = capacity.div_ceil(BLOCK).max(1);
        let word = std::mem::size_of::<u32>() as vk::DeviceSize;
        let keys_b = alloc_storage(device, mem_props, u64::from(capacity).max(1) * word);
        let vals_b = alloc_storage(device, mem_props, u64::from(capacity).max(1) * word);
        let hist = alloc_storage(device, mem_props, u64::from(RADIX * num_blocks) * word);

        let pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 10,
        };
        let pool = unsafe {
            device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(2)
                        .pool_sizes(std::slice::from_ref(&pool_size)),
                    None,
                )
                .expect("failed to create radix-sort descriptor pool")
        };
        let layouts = [set_layout, set_layout];
        let allocated = unsafe {
            device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&layouts),
                )
                .expect("failed to allocate radix-sort descriptor sets")
        };
        let sets = [allocated[0], allocated[1]];
        // Set 0 reads a and writes b; set 1 the other way. Alternating them is
        // the ping-pong.
        write_set(device, sets[0], keys_a, vals_a, keys_b.buffer, vals_b.buffer, hist.buffer);
        write_set(device, sets[1], keys_b.buffer, vals_b.buffer, keys_a, vals_a, hist.buffer);

        Self {
            set_layout,
            layout,
            histogram,
            scan,
            scatter,
            pool,
            sets,
            keys_b,
            vals_b,
            hist,
            capacity,
            key_bits,
        }
    }

    /// Record the whole sort of `n` pairs into `cb`. The result lands back in
    /// the caller's `keys_a`/`vals_a`.
    ///
    /// # Safety
    /// `cb` must be recording, outside a render pass, and `n <= capacity`.
    pub unsafe fn record(&self, device: &ash::Device, cb: vk::CommandBuffer, n: u32) {
        assert!(n <= self.capacity, "radix sort: {n} pairs exceeds capacity {}", self.capacity);
        if n == 0 {
            return;
        }
        let num_blocks = n.div_ceil(BLOCK);
        let passes = self.key_bits / RADIX_BITS;

        for pass in 0..passes {
            let set = self.sets[(pass % 2) as usize];
            let push = Push {
                n,
                num_blocks,
                shift: pass * RADIX_BITS,
                _pad: 0,
            };
            unsafe {
                device.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::COMPUTE,
                    self.layout,
                    0,
                    &[set],
                    &[],
                );
                device.cmd_push_constants(
                    cb,
                    self.layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    bytemuck::bytes_of(&push),
                );

                device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.histogram);
                device.cmd_dispatch(cb, num_blocks, 1, 1);
                barrier(device, cb);

                // One workgroup: the scan carries its running total in shared
                // memory rather than needing a second pass over the blocks.
                device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.scan);
                device.cmd_dispatch(cb, 1, 1, 1);
                barrier(device, cb);

                device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, self.scatter);
                device.cmd_dispatch(cb, num_blocks, 1, 1);
                barrier(device, cb);
            }
        }
    }

    /// # Safety
    /// The device must be idle and the sort unused afterwards.
    pub unsafe fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.histogram, None);
            device.destroy_pipeline(self.scan, None);
            device.destroy_pipeline(self.scatter, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_pool(self.pool, None);
            device.destroy_descriptor_set_layout(self.set_layout, None);
            for b in [&self.keys_b, &self.vals_b, &self.hist] {
                device.destroy_buffer(b.buffer, None);
                device.free_memory(b.memory, None);
            }
        }
    }
}

/// A full barrier between kernels: each one reads what the last one wrote.
fn barrier(device: &ash::Device, cb: vk::CommandBuffer) {
    let b = vk::MemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE);
    unsafe {
        device.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::DependencyFlags::empty(),
            std::slice::from_ref(&b),
            &[],
            &[],
        );
    }
}

fn write_set(
    device: &ash::Device,
    set: vk::DescriptorSet,
    src_keys: vk::Buffer,
    src_vals: vk::Buffer,
    dst_keys: vk::Buffer,
    dst_vals: vk::Buffer,
    hist: vk::Buffer,
) {
    let infos: Vec<vk::DescriptorBufferInfo> = [src_keys, src_vals, dst_keys, dst_vals, hist]
        .iter()
        .map(|&buffer| {
            vk::DescriptorBufferInfo::default()
                .buffer(buffer)
                .range(vk::WHOLE_SIZE)
        })
        .collect();
    let writes: Vec<vk::WriteDescriptorSet> = infos
        .iter()
        .enumerate()
        .map(|(i, info)| {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(i as u32)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(std::slice::from_ref(info))
        })
        .collect();
    unsafe { device.update_descriptor_sets(&writes, &[]) };
}

fn alloc_storage(
    device: &ash::Device,
    mem_props: &vk::PhysicalDeviceMemoryProperties,
    size: vk::DeviceSize,
) -> Owned {
    unsafe {
        let buffer = device
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .expect("failed to create radix-sort scratch buffer");
        let reqs = device.get_buffer_memory_requirements(buffer);
        let mem_type = find_memory_type(mem_props, reqs.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)
            .expect("no DEVICE_LOCAL memory for radix-sort scratch");
        let memory = device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(reqs.size)
                    .memory_type_index(mem_type),
                None,
            )
            .expect("failed to allocate radix-sort scratch");
        device.bind_buffer_memory(buffer, memory, 0).unwrap();
        Owned { buffer, memory }
    }
}

fn shader_module(device: &ash::Device, spv_bytes: &[u8], what: &str) -> vk::ShaderModule {
    let spv_u32: Vec<u32> = spv_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_le_bytes(c))
        .collect();
    unsafe {
        device
            .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&spv_u32), None)
            .unwrap_or_else(|e| panic!("failed to create {what} shader module: {e}"))
    }
}

fn compute_pipeline(
    device: &ash::Device,
    layout: vk::PipelineLayout,
    module: vk::ShaderModule,
    entry: &std::ffi::CStr,
) -> vk::Pipeline {
    let stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::COMPUTE)
        .module(module)
        .name(entry);
    let info = vk::ComputePipelineCreateInfo::default().stage(stage).layout(layout);
    unsafe {
        device
            .create_compute_pipelines(vk::PipelineCache::null(), std::slice::from_ref(&info), None)
            .unwrap_or_else(|(_, e)| panic!("failed to create {entry:?} compute pipeline: {e}"))[0]
    }
}
