use ash::vk;

pub(super) fn find_memory_type(
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

pub(super) fn alloc_upload_bytes(
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
