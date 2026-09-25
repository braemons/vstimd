//! The dot pipeline: one instanced unit quad per dot.
//!
//! A dot field is a few hundred to a few thousand identical quads that differ only
//! in position, so the whole field is one instanced draw: a shared unit quad in the
//! vertex buffer, and a per-stimulus instance buffer of [`DotInstance`] rewritten
//! each frame. Round dots come out of a distance test in the fragment shader, so
//! there is no texture and no per-dot mesh.

use crate::render::Vertex;
use crate::render::vk::VkMesh;

// ── Instance data ─────────────────────────────────────────────────────────────

/// One dot, as the instance buffer carries it.
///
/// Twelve bytes: everything else about a dot — its size, its two colours, the
/// aperture — is the same for every dot in the field and travels in the push
/// constants instead. `alt_color` is a flag rather than an RGBA precisely so this
/// stays small, since it is the only thing rewritten every frame.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DotInstance {
    /// Field-local pixels, origin at the field centre.
    pub pos_px: [f32; 2],
    /// 0 → `dot_color`, 1 → `dot_color_alt`. A float rather than an integer
    /// attribute because it is consumed as a `mix` factor.
    pub alt_color: f32,
}

// ── Push constants ────────────────────────────────────────────────────────────

/// Must match the `PushConstants` struct in `shaders/dots.wgsl` (std430).
///
/// Everything here is per *field*: the instance buffer carries only what differs
/// between dots. Layout (96 bytes):
///   offset  0: screen_half        [f32; 2]
///   offset  8: field_center_px    [f32; 2] ← the stimulus position, screen space
///   offset 16: aperture_offset_px [f32; 2] ← field-local, as `Aperture` stores it
///   offset 24: aperture_half      [f32; 2] ← half-extents: halving is a shader detail
///   offset 32: dot_radius_px      f32      ← half of `dot_size_px`, likewise
///   offset 36: dot_shape          u32      ← 0 Round, 1 Square
///   offset 40: aperture_shape     u32      ← 0 Rect, 1 Circle
///   offset 44: aperture_invert    u32
///   offset 48: clip_per_pixel     u32      ← 0 when the CPU already culled by centre
///   offset 52: global_opacity     f32
///   offset 56: _pad               [u32; 2] ← vec4 wants 16-byte alignment
///   offset 64: dot_color          [f32; 4]
///   offset 80: alt_color          [f32; 4]
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DotsPushConstants {
    pub screen_half: [f32; 2],
    pub field_center_px: [f32; 2],
    pub aperture_offset_px: [f32; 2],
    pub aperture_half: [f32; 2],
    pub dot_radius_px: f32,
    pub dot_shape: u32,
    pub aperture_shape: u32,
    pub aperture_invert: u32,
    pub clip_per_pixel: u32,
    pub global_opacity: f32,
    pub _pad: [u32; 2],
    pub dot_color: [f32; 4],
    pub alt_color: [f32; 4],
}

// ── Pipeline ──────────────────────────────────────────────────────────────────

pub struct VkDotsPipeline {
    pub pipeline: ash::vk::Pipeline,
    pub layout: ash::vk::PipelineLayout,
    /// Unit quad [-1,1]² shared by every dot of every field; the vertex shader
    /// scales it to `dot_radius_px` and offsets it by the instance position.
    pub quad: VkMesh,
}

impl VkDotsPipeline {
    pub fn new(
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: ash::vk::PhysicalDevice,
        render_pass: ash::vk::RenderPass,
        polygon_mode: ash::vk::PolygonMode,
    ) -> Self {
        let spv_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/dots.spv"));
        let spv_u32: Vec<u32> = spv_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let shader_info = ash::vk::ShaderModuleCreateInfo::default().code(&spv_u32);
        let shader_module = unsafe {
            device
                .create_shader_module(&shader_info, None)
                .expect("dots: shader module")
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

        // Two bindings: the shared quad per vertex, the dots per instance.
        let bindings = [
            ash::vk::VertexInputBindingDescription::default()
                .binding(0)
                .stride(std::mem::size_of::<Vertex>() as u32)
                .input_rate(ash::vk::VertexInputRate::VERTEX),
            ash::vk::VertexInputBindingDescription::default()
                .binding(1)
                .stride(std::mem::size_of::<DotInstance>() as u32)
                .input_rate(ash::vk::VertexInputRate::INSTANCE),
        ];
        let attributes = [
            // Quad corner, as [-1,1]² in `position.xy`.
            ash::vk::VertexInputAttributeDescription::default()
                .location(0)
                .binding(0)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            // Per-instance dot centre.
            ash::vk::VertexInputAttributeDescription::default()
                .location(1)
                .binding(1)
                .format(ash::vk::Format::R32G32_SFLOAT)
                .offset(0),
            // Per-instance colour selector.
            ash::vk::VertexInputAttributeDescription::default()
                .location(2)
                .binding(1)
                .format(ash::vk::Format::R32_SFLOAT)
                .offset(8),
        ];
        let vertex_input = ash::vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
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

        let push_range = ash::vk::PushConstantRange::default()
            .stage_flags(ash::vk::ShaderStageFlags::VERTEX | ash::vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<DotsPushConstants>() as u32);
        let layout_info = ash::vk::PipelineLayoutCreateInfo::default()
            .push_constant_ranges(std::slice::from_ref(&push_range));
        let layout = unsafe {
            device
                .create_pipeline_layout(&layout_info, None)
                .expect("dots: pipeline layout")
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
                .expect("dots: graphics pipeline")[0]
        };

        unsafe { device.destroy_shader_module(shader_module, None) };

        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let quad = VkMesh::unit_quad(device, &mem_props);

        Self { pipeline, layout, quad }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            self.quad.destroy(device);
        }
    }
}
