//! The Gaussian splat pipeline and the veil pipeline, both drawn in the 3-D pass.
//!
//! Created lazily with [`Pass3d`](super::vk_pass3d::Pass3d), like the mesh3d
//! pipeline, against its `[colour, depth]` render pass.
//!
//! Splats are depth-tested against the opaque meshes drawn before them but do
//! not write depth: they are sorted and blended instead (`splat::SplatSorter`).
//! The veil is a full-screen colour drawn last, for fading the 3-D view.

use ash::vk;

/// Must match `struct Push` in `shaders/splat.wgsl` (push constants, std430).
///
/// Layout (96 bytes, of the 128 Vulkan guarantees):
///   offset  0: model_view   [[f32; 4]; 4]
///   offset 64: proj         [f32; 4]   ← [m00, m11, m22, m32] of the projection
///   offset 80: viewport_px  [f32; 2]
///   offset 88: opacity      f32        + _pad f32
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SplatPushConstants {
    pub model_view: [[f32; 4]; 4],
    pub proj: [f32; 4],
    pub viewport_px: [f32; 2],
    pub opacity: f32,
    pub _pad: f32,
}

/// Must match `struct Push` in `shaders/veil.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VeilPushConstants {
    pub color: [f32; 4],
}

fn shader_module(device: &ash::Device, spv_bytes: &[u8], what: &str) -> vk::ShaderModule {
    let spv_u32: Vec<u32> = spv_bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    unsafe {
        device
            .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&spv_u32), None)
            .unwrap_or_else(|e| panic!("failed to create {what} shader module: {e}"))
    }
}

/// Everything but the shader, the blend and the depth test, which is what the
/// two pipelines here differ in: no vertex input, dynamic viewport, no culling.
#[allow(clippy::too_many_arguments)]
fn build_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    module: vk::ShaderModule,
    topology: vk::PrimitiveTopology,
    depth_test: bool,
    blend: vk::PipelineColorBlendAttachmentState,
    what: &str,
) -> vk::Pipeline {
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(module)
            .name(c"vs_main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(module)
            .name(c"fs_main"),
    ];
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(topology);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasteriser = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(depth_test)
        .depth_write_enable(false)
        .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
    let blend_state = vk::PipelineColorBlendStateCreateInfo::default()
        .attachments(std::slice::from_ref(&blend));
    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasteriser)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&blend_state)
        .dynamic_state(&dynamic_state)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);
    unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
            .unwrap_or_else(|e| panic!("failed to create {what} pipeline: {e:?}"))[0]
    }
}

fn blend(src_color: vk::BlendFactor) -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(src_color)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA)
}

/// The render-thread half of splatting: both pipelines and the descriptor set
/// layout every splat cloud allocates its sets against. Lives on
/// `SceneRenderer` next to `Mesh3dRenderer`.
pub struct SplatRenderer {
    pub set_layout: vk::DescriptorSetLayout,
    pub layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
    pub veil_layout: vk::PipelineLayout,
    pub veil_pipeline: vk::Pipeline,
}

impl SplatRenderer {
    pub fn new(device: &ash::Device, render_pass: vk::RenderPass) -> Self {
        let storage = |binding| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
        };
        let bindings = [storage(0), storage(1)];
        let set_layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .expect("failed to create splat descriptor set layout")
        };
        let push = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .size(std::mem::size_of::<SplatPushConstants>() as u32);
        let layout = unsafe {
            device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(std::slice::from_ref(&set_layout))
                        .push_constant_ranges(std::slice::from_ref(&push)),
                    None,
                )
                .expect("failed to create splat pipeline layout")
        };
        let module = shader_module(
            device,
            include_bytes!(concat!(env!("OUT_DIR"), "/splat.spv")),
            "splat",
        );
        let pipeline = build_pipeline(
            device,
            render_pass,
            layout,
            module,
            vk::PrimitiveTopology::TRIANGLE_STRIP,
            true,
            blend(vk::BlendFactor::ONE),
            "splat",
        );
        unsafe { device.destroy_shader_module(module, None) };

        let veil_push = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .size(std::mem::size_of::<VeilPushConstants>() as u32);
        let veil_layout = unsafe {
            device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .push_constant_ranges(std::slice::from_ref(&veil_push)),
                    None,
                )
                .expect("failed to create veil pipeline layout")
        };
        let veil_module = shader_module(
            device,
            include_bytes!(concat!(env!("OUT_DIR"), "/veil.spv")),
            "veil",
        );
        let veil_pipeline = build_pipeline(
            device,
            render_pass,
            veil_layout,
            veil_module,
            vk::PrimitiveTopology::TRIANGLE_LIST,
            false,
            blend(vk::BlendFactor::SRC_ALPHA),
            "veil",
        );
        unsafe { device.destroy_shader_module(veil_module, None) };

        Self {
            set_layout,
            layout,
            pipeline,
            veil_layout,
            veil_pipeline,
        }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.veil_pipeline, None);
            device.destroy_pipeline_layout(self.veil_layout, None);
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_constant_sizes_match_the_shaders() {
        assert_eq!(std::mem::size_of::<SplatPushConstants>(), 96);
        assert_eq!(std::mem::offset_of!(SplatPushConstants, proj), 64);
        assert_eq!(std::mem::offset_of!(SplatPushConstants, viewport_px), 80);
        assert_eq!(std::mem::offset_of!(SplatPushConstants, opacity), 88);
        assert_eq!(std::mem::size_of::<VeilPushConstants>(), 16);
    }
}
