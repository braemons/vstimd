//! Recording the 3-D pass. Called from `render_frame` only on frames that have
//! something 3-D to draw; a pure 2-D frame never reaches this file.

use ash::vk;

use crate::render::vk::cache::Mesh3dCache;
use crate::render::vk::{Mesh3dPushConstants, Mesh3dRenderer, Pass3d, SceneUniform, VkContext};
use crate::scene::{SceneState, Shading3D};

/// Begin the `[colour, depth]` pass, draw every 3-D object, end it. Leaves the
/// colour image in `COLOR_ATTACHMENT_OPTIMAL` for the 2-D pass's `LOAD` flavour.
///
/// # Safety
/// `cb` must be recording, outside any render pass.
#[allow(clippy::too_many_arguments)]
pub unsafe fn record_3d_pass(
    ctx: &VkContext,
    cb: vk::CommandBuffer,
    pass: &Pass3d,
    mesh3d: &Mesh3dRenderer,
    image_index: u32,
    frame_slot: usize,
    background: vk::ClearColorValue,
    wireframe: bool,
    scene: &SceneState,
    meshes: &Mesh3dCache,
) {
    let extent = ctx.extent;
    let aspect = extent.width as f32 / extent.height.max(1) as f32;
    let camera = scene.camera.live;
    mesh3d.uniforms.write(
        frame_slot,
        &SceneUniform {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            camera_pos: camera.position_cm.to_array(),
            ..Default::default()
        },
    );

    let render_area = vk::Rect2D {
        offset: vk::Offset2D::default(),
        extent,
    };
    let clear_values = [
        vk::ClearValue { color: background },
        vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        },
    ];
    let viewport = vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: extent.width as f32,
        height: extent.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    let pipe = if wireframe {
        &mesh3d.wireframe_pipeline
    } else {
        &mesh3d.pipeline
    };
    let device = &ctx.device;

    unsafe {
        device.cmd_begin_render_pass(
            cb,
            &vk::RenderPassBeginInfo::default()
                .render_pass(pass.render_pass)
                .framebuffer(pass.framebuffers[image_index as usize])
                .render_area(render_area)
                .clear_values(&clear_values),
            vk::SubpassContents::INLINE,
        );
        ctx.cmd_begin_label(cb, "3-D", [1.0, 0.6, 0.2, 1.0]);
        device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipe.pipeline);
        device.cmd_set_viewport(cb, 0, std::slice::from_ref(&viewport));
        device.cmd_set_scissor(cb, 0, std::slice::from_ref(&render_area));
        device.cmd_bind_descriptor_sets(
            cb,
            vk::PipelineBindPoint::GRAPHICS,
            pipe.layout,
            0,
            &[mesh3d.uniforms.sets[frame_slot]],
            &[],
        );

        // Scene order, like the 2-D pass; the depth buffer sorts opaque geometry.
        // Vertex and index buffers are rebound only when the mesh changes, so a
        // run of same-geometry stimuli costs one push-constant update each.
        let mut bound = None;
        for entry in scene.stimuli.values() {
            let stim = &entry.stimulus;
            let Some(m) = stim.mesh3d() else { continue };
            if !stim.is_visible() {
                continue;
            }
            let geometry = m.geometry.live;
            let key = geometry.mesh_key();
            let Some(mesh) = meshes.get(key) else {
                continue;
            };
            if bound != Some(key) {
                device.cmd_bind_vertex_buffers(cb, 0, &[mesh.vertex_buffer], &[0]);
                device.cmd_bind_index_buffer(cb, mesh.index_buffer, 0, vk::IndexType::UINT32);
                bound = Some(key);
            }
            let material = m.material.live;
            let pc = Mesh3dPushConstants {
                model: m
                    .transform
                    .live
                    .model_matrix(geometry.model_scale())
                    .to_cols_array_2d(),
                albedo: material.albedo.scaled_alpha(stim.opacity().live).into(),
                emissive: material.emissive,
                shading: match material.shading {
                    Shading3D::Unlit => 0,
                    Shading3D::Phong => 1,
                },
            };
            device.cmd_push_constants(
                cb,
                pipe.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&pc),
            );
            device.cmd_draw_indexed(cb, mesh.index_count, 1, 0, 0, 0);
        }

        ctx.cmd_end_label(cb);
        device.cmd_end_render_pass(cb);
    }
}
