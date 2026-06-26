use crate::render::Vertex;
use crate::render::vk::VkMesh;

// ── Push-constant layout for the grating pipeline ────────────────────────────

/// Must match the `PushConstants` struct in `shaders/grating.wgsl` (std430).
///
/// Layout (96 bytes):
///   offset  0: screen_half     [f32; 2]
///   offset  8: center_px       [f32; 2]
///   offset 16: half_size       [f32; 2]
///   offset 24: sf              f32
///   offset 28: phase           f32
///   offset 32: ori_rad         f32
///   offset 36: contrast        f32
///   offset 40: global_opacity  f32      ← global alpha multiplier
///   offset 44: _pad_color      u32      ← padding (vec4 requires 16-byte alignment)
///   offset 48: fore_color      [f32; 4] ← rgba peak colour
///   offset 64: back_color      [f32; 4] ← rgba trough colour
///   offset 80: waveform        u32
///   offset 84: mask_type       u32
///   offset 88: mask_param      f32   (SD for gauss; fringe width for raisedCos; 0 = use default)
///   offset 92: _pad            u32
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GratingPushConstants {
    pub screen_half: [f32; 2],
    pub center_px: [f32; 2],
    pub half_size: [f32; 2],
    pub sf: f32,
    pub phase: f32,
    pub ori_rad: f32,
    pub contrast: f32,
    pub global_opacity: f32,
    pub _pad_color: u32,
    pub fore_color: [f32; 4], // rgba peak colour
    pub back_color: [f32; 4], // rgba trough colour
    pub waveform: u32,
    pub mask_type: u32,
    pub mask_param: f32,
    pub _pad: u32,
}

// ── Grating pipeline ──────────────────────────────────────────────────────────

pub struct VkGratingPipeline {
    pub pipeline: ash::vk::Pipeline,
    pub layout: ash::vk::PipelineLayout,
    /// Unit quad [-1,1]×[-1,1] shared by all grating draw calls.
    /// The vertex shader positions it per-grating via push constants.
    pub quad: VkMesh,
}

impl VkGratingPipeline {
    pub fn new(
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: ash::vk::PhysicalDevice,
        render_pass: ash::vk::RenderPass,
        polygon_mode: ash::vk::PolygonMode,
    ) -> Self {
        let spv_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/grating.spv"));
        let spv_u32: Vec<u32> = spv_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let shader_info = ash::vk::ShaderModuleCreateInfo::default().code(&spv_u32);
        let shader_module = unsafe {
            device
                .create_shader_module(&shader_info, None)
                .expect("grating: shader module")
        };

        let entry_vs = c"vs_main";
        let entry_fs = c"fs_main";
        let shader_stages = [
            ash::vk::PipelineShaderStageCreateInfo::default()
                .stage(ash::vk::ShaderStageFlags::VERTEX)
                .module(shader_module)
                .name(entry_vs),
            ash::vk::PipelineShaderStageCreateInfo::default()
                .stage(ash::vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module)
                .name(entry_fs),
        ];

        // Vertex input — same layout as the solid pipeline.
        let binding = ash::vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(ash::vk::VertexInputRate::VERTEX);
        let attributes = [
            ash::vk::VertexInputAttributeDescription::default()
                .location(0)
                .binding(0)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            ash::vk::VertexInputAttributeDescription::default()
                .location(1)
                .binding(0)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(12),
            ash::vk::VertexInputAttributeDescription::default()
                .location(2)
                .binding(0)
                .format(ash::vk::Format::R32G32_SFLOAT)
                .offset(24),
            ash::vk::VertexInputAttributeDescription::default()
                .location(3)
                .binding(0)
                .format(ash::vk::Format::R32G32B32A32_SFLOAT)
                .offset(32),
        ];
        let vertex_input = ash::vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding))
            .vertex_attribute_descriptions(&attributes);

        let input_assembly = ash::vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(ash::vk::PrimitiveTopology::TRIANGLE_LIST);

        let dynamic_states = [
            ash::vk::DynamicState::VIEWPORT,
            ash::vk::DynamicState::SCISSOR,
        ];
        let dynamic_state =
            ash::vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let viewport_state = ash::vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasteriser = ash::vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(polygon_mode)
            .cull_mode(ash::vk::CullModeFlags::NONE)
            .front_face(ash::vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample = ash::vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(ash::vk::SampleCountFlags::TYPE_1);

        let blend_attachment = ash::vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(ash::vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(ash::vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(ash::vk::BlendOp::ADD)
            .src_alpha_blend_factor(ash::vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(ash::vk::BlendFactor::ZERO)
            .alpha_blend_op(ash::vk::BlendOp::ADD)
            .color_write_mask(ash::vk::ColorComponentFlags::RGBA);
        let blend_state = ash::vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&blend_attachment));

        // Push constant range covers the full GratingPushConstants struct.
        let push_range = ash::vk::PushConstantRange::default()
            .stage_flags(ash::vk::ShaderStageFlags::VERTEX | ash::vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<GratingPushConstants>() as u32);
        let layout_info = ash::vk::PipelineLayoutCreateInfo::default()
            .push_constant_ranges(std::slice::from_ref(&push_range));
        let layout = unsafe {
            device
                .create_pipeline_layout(&layout_info, None)
                .expect("grating: pipeline layout")
        };

        let pipeline_info = ash::vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasteriser)
            .multisample_state(&multisample)
            .color_blend_state(&blend_state)
            .dynamic_state(&dynamic_state)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0);
        let pipeline = unsafe {
            device
                .create_graphics_pipelines(ash::vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("grating: graphics pipeline")[0]
        };

        unsafe { device.destroy_shader_module(shader_module, None) };

        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let quad = Self::create_quad(device, mem_props);

        Self {
            pipeline,
            layout,
            quad,
        }
    }

    fn create_quad(
        device: &ash::Device,
        mem_props: ash::vk::PhysicalDeviceMemoryProperties,
    ) -> VkMesh {
        let n = [0.0f32, 0.0, 1.0];
        let uv = [0.0f32; 2];
        let verts: [Vertex; 4] = [
            Vertex {
                position: [-1.0, -1.0, 0.0],
                normal: n,
                uv,
                color: crate::Color::TRANSPARENT,
            },
            Vertex {
                position: [1.0, -1.0, 0.0],
                normal: n,
                uv,
                color: crate::Color::TRANSPARENT,
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal: n,
                uv,
                color: crate::Color::TRANSPARENT,
            },
            Vertex {
                position: [-1.0, 1.0, 0.0],
                normal: n,
                uv,
                color: crate::Color::TRANSPARENT,
            },
        ];
        let idxs: [u32; 6] = [0, 1, 2, 0, 2, 3];
        let (vb, vm) = Self::alloc_buf(
            device,
            mem_props,
            ash::vk::BufferUsageFlags::VERTEX_BUFFER,
            bytemuck::cast_slice(&verts),
        );
        let (ib, im) = Self::alloc_buf(
            device,
            mem_props,
            ash::vk::BufferUsageFlags::INDEX_BUFFER,
            bytemuck::cast_slice(&idxs),
        );
        VkMesh::from_raw(vb, vm, ib, im, 6)
    }

    fn alloc_buf(
        device: &ash::Device,
        mem_props: ash::vk::PhysicalDeviceMemoryProperties,
        usage: ash::vk::BufferUsageFlags,
        data: &[u8],
    ) -> (ash::vk::Buffer, ash::vk::DeviceMemory) {
        let size = data.len() as ash::vk::DeviceSize;
        let buf = unsafe {
            device
                .create_buffer(
                    &ash::vk::BufferCreateInfo::default()
                        .size(size)
                        .usage(usage)
                        .sharing_mode(ash::vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .expect("grating quad: create buffer")
        };
        let reqs = unsafe { device.get_buffer_memory_requirements(buf) };
        let mem_type = (0..mem_props.memory_type_count)
            .find(|&i| {
                (reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize].property_flags.contains(
                        ash::vk::MemoryPropertyFlags::HOST_VISIBLE
                            | ash::vk::MemoryPropertyFlags::HOST_COHERENT,
                    )
            })
            .expect("grating quad: no HOST_VISIBLE|HOST_COHERENT memory");
        let mem = unsafe {
            device
                .allocate_memory(
                    &ash::vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(mem_type),
                    None,
                )
                .expect("grating quad: allocate memory")
        };
        unsafe {
            device.bind_buffer_memory(buf, mem, 0).unwrap();
            let ptr = device
                .map_memory(mem, 0, size, ash::vk::MemoryMapFlags::empty())
                .expect("grating quad: map memory") as *mut u8;
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            device.unmap_memory(mem);
        }
        (buf, mem)
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            self.quad.destroy(device);
        }
    }
}
