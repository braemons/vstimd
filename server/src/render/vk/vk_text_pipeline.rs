/// One corner of a text glyph quad.
///
/// `position` is in NDC ([-1,1]×[-1,1], Y-up).
/// `uv` is the atlas texture coordinate ([0,1]×[0,1], Y-down).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

// ── Push constants ────────────────────────────────────────────────────────────

/// Must match `PushConstants` in `shaders/text.wgsl`.
///
/// Layout (16 bytes):
///   offset  0: color  [f32; 4]  rgba tint
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextPushConstants {
    pub color: [f32; 4],
}

// ── Pipeline ──────────────────────────────────────────────────────────────────

pub struct VkTextPipeline {
    pub pipeline: ash::vk::Pipeline,
    pub layout: ash::vk::PipelineLayout,
}

impl VkTextPipeline {
    /// Create the text pipeline.
    ///
    /// `atlas_dsl` is borrowed from `GlyphAtlas::descriptor_set_layout` — the
    /// atlas owns the layout object; this pipeline only references it during
    /// pipeline-layout construction and does not free it.
    pub fn new(
        device: &ash::Device,
        render_pass: ash::vk::RenderPass,
        atlas_dsl: ash::vk::DescriptorSetLayout,
    ) -> Self {
        let spv_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/text.spv"));
        let spv_u32: Vec<u32> = spv_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let shader_module = unsafe {
            device
                .create_shader_module(
                    &ash::vk::ShaderModuleCreateInfo::default().code(&spv_u32),
                    None,
                )
                .expect("text: shader module")
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

        let binding = ash::vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<TextVertex>() as u32)
            .input_rate(ash::vk::VertexInputRate::VERTEX);
        let attributes = [
            ash::vk::VertexInputAttributeDescription::default()
                .location(0)
                .binding(0)
                .format(ash::vk::Format::R32G32_SFLOAT)
                .offset(0),
            ash::vk::VertexInputAttributeDescription::default()
                .location(1)
                .binding(0)
                .format(ash::vk::Format::R32G32_SFLOAT)
                .offset(8),
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
            .polygon_mode(ash::vk::PolygonMode::FILL)
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

        let push_range = ash::vk::PushConstantRange::default()
            .stage_flags(ash::vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<TextPushConstants>() as u32);
        let set_layouts = [atlas_dsl];
        let layout_info = ash::vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(std::slice::from_ref(&push_range));
        let layout = unsafe {
            device
                .create_pipeline_layout(&layout_info, None)
                .expect("text: pipeline layout")
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
                .expect("text: graphics pipeline")[0]
        };

        unsafe { device.destroy_shader_module(shader_module, None) };
        Self { pipeline, layout }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
        }
    }
}
