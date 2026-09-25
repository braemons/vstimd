use ash::vk;

pub fn find_memory_type(
    mem_props: &vk::PhysicalDeviceMemoryProperties,
    filter: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..mem_props.memory_type_count).find(|&i| {
        (filter & (1 << i)) != 0
            && mem_props.memory_types[i as usize]
                .property_flags
                .contains(flags)
    })
}

pub fn alloc_upload_bytes(
    mem_props: &vk::PhysicalDeviceMemoryProperties,
    device: &ash::Device,
    usage: vk::BufferUsageFlags,
    data: &[u8],
) -> (vk::Buffer, vk::DeviceMemory) {
    let size = data.len() as vk::DeviceSize;
    let buf = unsafe {
        device
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .expect("failed to create buffer")
    };
    let reqs = unsafe { device.get_buffer_memory_requirements(buf) };
    let mem_type = find_memory_type(
        mem_props,
        reqs.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .expect("no HOST_VISIBLE|HOST_COHERENT memory");
    let mem = unsafe {
        device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(reqs.size)
                    .memory_type_index(mem_type),
                None,
            )
            .expect("failed to allocate buffer memory")
    };
    unsafe {
        device.bind_buffer_memory(buf, mem, 0).unwrap();
        let ptr = device
            .map_memory(mem, 0, size, vk::MemoryMapFlags::empty())
            .expect("failed to map buffer") as *mut u8;
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        device.unmap_memory(mem);
    }
    (buf, mem)
}

/// Record `record` into a one-time command buffer, submit it and wait for it.
///
/// Blocks the calling thread until the GPU finishes, so it is for one-off
/// uploads (a new mesh or texture), never per frame. `GlyphAtlas` has its own
/// copy of this pattern; the text path is left untouched rather than moved here.
pub fn run_one_time_commands(
    device: &ash::Device,
    pool: vk::CommandPool,
    queue: vk::Queue,
    record: impl FnOnce(vk::CommandBuffer),
) {
    unsafe {
        let cb = device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
            .expect("failed to allocate one-time command buffer")[0];
        device
            .begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .expect("begin one-time command buffer");
        record(cb);
        device.end_command_buffer(cb).expect("end one-time command buffer");
        let fence = device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .expect("create one-time fence");
        let cbs = [cb];
        device
            .queue_submit(queue, &[vk::SubmitInfo::default().command_buffers(&cbs)], fence)
            .expect("submit one-time command buffer");
        device
            .wait_for_fences(&[fence], true, u64::MAX)
            .expect("wait for one-time fence");
        device.destroy_fence(fence, None);
        device.free_command_buffers(pool, &cbs);
    }
}

/// Upload `data` into a new `DEVICE_LOCAL` buffer through a host-visible staging
/// buffer, for data that is written once and read by the GPU for a long time.
/// Blocks until the copy completes — see [`run_one_time_commands`].
pub fn upload_device_local(
    mem_props: &vk::PhysicalDeviceMemoryProperties,
    device: &ash::Device,
    pool: vk::CommandPool,
    queue: vk::Queue,
    usage: vk::BufferUsageFlags,
    data: &[u8],
) -> (vk::Buffer, vk::DeviceMemory) {
    let size = data.len() as vk::DeviceSize;
    let (staging, staging_mem) =
        alloc_upload_bytes(mem_props, device, vk::BufferUsageFlags::TRANSFER_SRC, data);
    let buf = unsafe {
        device
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(usage | vk::BufferUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .expect("failed to create device-local buffer")
    };
    let reqs = unsafe { device.get_buffer_memory_requirements(buf) };
    let mem_type = find_memory_type(
        mem_props,
        reqs.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .expect("no DEVICE_LOCAL memory");
    let mem = unsafe {
        device
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(reqs.size)
                    .memory_type_index(mem_type),
                None,
            )
            .expect("failed to allocate device-local buffer memory")
    };
    unsafe { device.bind_buffer_memory(buf, mem, 0).unwrap() };
    run_one_time_commands(device, pool, queue, |cb| unsafe {
        device.cmd_copy_buffer(
            cb,
            staging,
            buf,
            &[vk::BufferCopy::default().size(size)],
        );
    });
    unsafe {
        device.destroy_buffer(staging, None);
        device.free_memory(staging_mem, None);
    }
    (buf, mem)
}
