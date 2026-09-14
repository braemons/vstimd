//! The mesh3d pipeline and the per-frame scene uniform it reads.
//!
//! Created lazily with [`Pass3d`](super::vk_pass3d::Pass3d), against its
//! `[colour, depth]` render pass — so, unlike the 2-D pipelines, it cannot exist
//! before the first 3-D frame.

use ash::vk;

use crate::render::Vertex;
use crate::render::vk::VkMesh;
use crate::render::vk::buffers::find_memory_type;

/// Must match `struct Scene` in `shaders/mesh3d.wgsl` (uniform, std140).
///
/// Layout (128 bytes):
///   offset   0: view_proj   [[f32; 4]; 4]
///   offset  64: camera_pos  [f32; 3]   + _pad0 f32
///   offset  80: ambient     [f32; 3]   + _pad1 f32   ← lighting, #71
///   offset  96: sun_dir     [f32; 3]   + _pad2 f32   ← lighting, #71
///   offset 112: sun_color   [f32; 3]   + _pad3 f32   ← lighting, #71
///
/// The lighting fields are declared now so the layout does not churn when #71
/// starts reading them.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _pad0: f32,
    pub ambient: [f32; 3],
    pub _pad1: f32,
    pub sun_dir: [f32; 3],
    pub _pad2: f32,
    pub sun_color: [f32; 3],
    pub _pad3: f32,
}

/// Must match `struct Object` in `shaders/mesh3d.wgsl` (push constants, std430).
///
/// Layout (96 bytes, of the 128 Vulkan guarantees):
///   offset  0: model     [[f32; 4]; 4]
///   offset 64: albedo    [f32; 4]
///   offset 80: emissive  [f32; 3]
///   offset 92: shading   u32          ← 0 = Unlit
///
/// No normal matrix: an inverse-transpose `mat3` is 48 bytes and would not fit,
/// so the vertex shader derives it from `model`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mesh3dPushConstants {
    pub model: [[f32; 4]; 4],
    pub albedo: [f32; 4],
    pub emissive: [f32; 3],
    pub shading: u32,
}

const PUSH_STAGES: vk::ShaderStageFlags = vk::ShaderStageFlags::from_raw(
    vk::ShaderStageFlags::VERTEX.as_raw() | vk::ShaderStageFlags::FRAGMENT.as_raw(),
);

pub struct VkMesh3dPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
}

impl VkMesh3dPipeline {
    pub fn new(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        scene_set_layout: vk::DescriptorSetLayout,
        polygon_mode: vk::PolygonMode,
    ) -> Self {
        let spv_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/mesh3d.spv"));
        let spv_u32: Vec<u32> = spv_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let shader_module = unsafe {
            device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&spv_u32), None)
                .expect("failed to create mesh3d shader module")
        };
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(shader_module)
                .name(c"vs_main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(shader_module)
                .name(c"fs_main"),
        ];

        // The shared `Vertex` layout, unchanged from the 2-D pipelines.
        let binding = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX);
        let attribute = |location, format, offset| {
            vk::VertexInputAttributeDescription::default()
                .location(location)
                .binding(0)
                .format(format)
                .offset(offset)
        };
        let attributes = [
            attribute(0, vk::Format::R32G32B32_SFLOAT, 0),
            attribute(1, vk::Format::R32G32B32_SFLOAT, 12),
            attribute(2, vk::Format::R32G32_SFLOAT, 24),
            attribute(3, vk::Format::R32G32B32A32_SFLOAT, 32),
        ];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding))
            .vertex_attribute_descriptions(&attributes);
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasteriser = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(polygon_mode)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS);

        // Same blend as the solid pipeline, so an opaque unlit surface writes
        // exactly the colour a 2-D shape would. Sorted transparency is §B.6.
        let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&blend_attachment));

        let push_range = vk::PushConstantRange::default()
            .stage_flags(PUSH_STAGES)
            .offset(0)
            .size(std::mem::size_of::<Mesh3dPushConstants>() as u32);
        let set_layouts = [scene_set_layout];
        let layout = unsafe {
            device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts)
                        .push_constant_ranges(std::slice::from_ref(&push_range)),
                    None,
                )
                .expect("failed to create mesh3d pipeline layout")
        };

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
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
        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("failed to create mesh3d pipeline")[0]
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

/// One host-visible, persistently mapped uniform buffer and descriptor set per
/// frame-in-flight slot, so frame N+1's uniform is never written while frame N's
/// command buffer may still be reading its own.
pub struct SceneUniforms {
    pub set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    pub sets: Vec<vk::DescriptorSet>,
    buffers: Vec<vk::Buffer>,
    memory: Vec<vk::DeviceMemory>,
    mapped: Vec<*mut SceneUniform>,
}

impl SceneUniforms {
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        slots: usize,
    ) -> Self {
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(PUSH_STAGES);
        let set_layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(std::slice::from_ref(&binding)),
                    None,
                )
                .expect("failed to create scene descriptor set layout")
        };
        let pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: slots as u32,
        };
        let pool = unsafe {
            device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(slots as u32)
                        .pool_sizes(std::slice::from_ref(&pool_size)),
                    None,
                )
                .expect("failed to create scene descriptor pool")
        };
        let layouts = vec![set_layout; slots];
        let sets = unsafe {
            device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&layouts),
                )
                .expect("failed to allocate scene descriptor sets")
        };

        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let size = std::mem::size_of::<SceneUniform>() as vk::DeviceSize;
        let mut buffers = Vec::with_capacity(slots);
        let mut memory = Vec::with_capacity(slots);
        let mut mapped = Vec::with_capacity(slots);
        for &set in &sets {
            let buffer = unsafe {
                device
                    .create_buffer(
                        &vk::BufferCreateInfo::default()
                            .size(size)
                            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                            .sharing_mode(vk::SharingMode::EXCLUSIVE),
                        None,
                    )
                    .expect("failed to create scene uniform buffer")
            };
            let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
            let mem_type = find_memory_type(
                &mem_props,
                reqs.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
            .expect("no HOST_VISIBLE|HOST_COHERENT memory for the scene uniform");
            let mem = unsafe {
                device
                    .allocate_memory(
                        &vk::MemoryAllocateInfo::default()
                            .allocation_size(reqs.size)
                            .memory_type_index(mem_type),
                        None,
                    )
                    .expect("failed to allocate scene uniform memory")
            };
            let ptr = unsafe {
                device.bind_buffer_memory(buffer, mem, 0).unwrap();
                device
                    .map_memory(mem, 0, size, vk::MemoryMapFlags::empty())
                    .expect("failed to map scene uniform") as *mut SceneUniform
            };
            let info = vk::DescriptorBufferInfo::default()
                .buffer(buffer)
                .offset(0)
                .range(size);
            let write = vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(std::slice::from_ref(&info));
            unsafe { device.update_descriptor_sets(&[write], &[]) };
            buffers.push(buffer);
            memory.push(mem);
            mapped.push(ptr);
        }
        Self {
            set_layout,
            pool,
            sets,
            buffers,
            memory,
            mapped,
        }
    }

    /// Write `slot`'s uniform. The caller has waited on that slot's fence.
    pub fn write(&self, slot: usize, value: &SceneUniform) {
        // SAFETY: mapped for the lifetime of `self`, sized for one SceneUniform,
        // and only the render thread touches it.
        unsafe { self.mapped[slot].write_unaligned(*value) };
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            for (&buffer, &mem) in self.buffers.iter().zip(&self.memory) {
                device.unmap_memory(mem);
                device.destroy_buffer(buffer, None);
                device.free_memory(mem, None);
            }
            device.destroy_descriptor_pool(self.pool, None);
            device.destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}

/// The render-thread half of 3-D: pipelines and the scene uniform. Lives on
/// `SceneRenderer` as `Option<Mesh3dRenderer>`, created with `Pass3d`.
pub struct Mesh3dRenderer {
    pub uniforms: SceneUniforms,
    pub pipeline: VkMesh3dPipeline,
    pub wireframe_pipeline: VkMesh3dPipeline,
    /// Temporary (#68): the hardcoded cube behind `VSTIMD_DEBUG_3D_CUBE`, until
    /// #69/#70 give 3-D stimuli a scene type and a geometry-keyed mesh cache.
    pub debug_cube: VkMesh,
}

impl Mesh3dRenderer {
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        render_pass: vk::RenderPass,
        slots: usize,
        supports_wireframe: bool,
    ) -> Self {
        let uniforms = SceneUniforms::new(instance, physical_device, device, slots);
        let pipeline = VkMesh3dPipeline::new(
            device,
            render_pass,
            uniforms.set_layout,
            vk::PolygonMode::FILL,
        );
        let wf_mode = if supports_wireframe {
            vk::PolygonMode::LINE
        } else {
            vk::PolygonMode::FILL
        };
        let wireframe_pipeline =
            VkMesh3dPipeline::new(device, render_pass, uniforms.set_layout, wf_mode);

        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let debug_cube = debug_cube_mesh(device, &mem_props);
        Self {
            uniforms,
            pipeline,
            wireframe_pipeline,
            debug_cube,
        }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe { self.debug_cube.destroy(device) };
        self.wireframe_pipeline.destroy(device);
        self.pipeline.destroy(device);
        self.uniforms.destroy(device);
    }
}

/// A unit cube with a distinct colour per face, so it reads as a solid under the
/// unlit shader. Host-visible: device-local upload belongs to #70's mesh cache.
fn debug_cube_mesh(device: &ash::Device, mem_props: &vk::PhysicalDeviceMemoryProperties) -> VkMesh {
    use crate::Color;
    use crate::render::vk::buffers::alloc_upload_bytes;
    const FACE_COLORS: [Color; 6] = [
        Color::new(0.9, 0.2, 0.2, 1.0),
        Color::new(0.2, 0.9, 0.2, 1.0),
        Color::new(0.2, 0.2, 0.9, 1.0),
        Color::new(0.9, 0.9, 0.2, 1.0),
        Color::new(0.2, 0.9, 0.9, 1.0),
        Color::new(0.9, 0.2, 0.9, 1.0),
    ];
    let (mut verts, idxs) = crate::render::tess3d::unit_cube(Color::WHITE);
    for (face, chunk) in verts.chunks_exact_mut(4).enumerate() {
        chunk.iter_mut().for_each(|v| v.color = FACE_COLORS[face]);
    }
    let (vb, vm) = alloc_upload_bytes(
        mem_props,
        device,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        bytemuck::cast_slice(&verts),
    );
    let (ib, im) = alloc_upload_bytes(
        mem_props,
        device,
        vk::BufferUsageFlags::INDEX_BUFFER,
        bytemuck::cast_slice(&idxs),
    );
    VkMesh::from_raw(vb, vm, ib, im, idxs.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_and_push_constant_sizes_match_the_shader() {
        assert_eq!(std::mem::size_of::<SceneUniform>(), 128);
        assert_eq!(std::mem::offset_of!(SceneUniform, camera_pos), 64);
        assert_eq!(std::mem::offset_of!(SceneUniform, sun_color), 112);
        assert_eq!(std::mem::size_of::<Mesh3dPushConstants>(), 96);
        assert_eq!(std::mem::offset_of!(Mesh3dPushConstants, emissive), 80);
        assert_eq!(std::mem::offset_of!(Mesh3dPushConstants, shading), 92);
    }
}
