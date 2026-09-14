//! Recording the 3-D pass. Called from `render_frame` only on frames that have
//! something 3-D to draw; a pure 2-D frame never reaches this file.

use ash::vk;
use glam::{Mat4, Quat, Vec3};

use crate::render::vk::{Mesh3dPushConstants, Mesh3dRenderer, Pass3d, SceneUniform, VkContext};
use crate::scene::SceneState;

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
    debug_cube: bool,
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

        if debug_cube {
            let cube = &mesh3d.debug_cube;
            device.cmd_bind_vertex_buffers(cb, 0, &[cube.vertex_buffer], &[0]);
            device.cmd_bind_index_buffer(cb, cube.index_buffer, 0, vk::IndexType::UINT32);
            for model in debug_cube_models(scene.runtime.frame_count) {
                let pc = Mesh3dPushConstants {
                    model: model.to_cols_array_2d(),
                    albedo: [1.0; 4],
                    emissive: [0.0; 3],
                    shading: 0,
                };
                device.cmd_push_constants(
                    cb,
                    pipe.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&pc),
                );
                device.cmd_draw_indexed(cb, cube.index_count, 1, 0, 0, 0);
            }
        }

        ctx.cmd_end_label(cb);
        device.cmd_end_render_pass(cb);
    }
}

/// Temporary (#68). A 20 cm cube spinning 60 cm in front of the default camera,
/// and a non-uniformly scaled box pushed through it, up and to the right — so
/// one look checks depth testing, culling, non-uniform scale and that world +Y
/// is screen up.
fn debug_cube_models(frame_count: u64) -> [Mat4; 2] {
    let t = frame_count as f32 / 60.0;
    [
        Mat4::from_scale_rotation_translation(
            Vec3::splat(10.0),
            Quat::from_rotation_y(t) * Quat::from_rotation_x(0.6 * t),
            Vec3::new(0.0, 0.0, -60.0),
        ),
        Mat4::from_scale_rotation_translation(
            Vec3::new(14.0, 3.0, 3.0),
            Quat::from_rotation_z(0.3),
            Vec3::new(10.0, 12.0, -60.0),
        ),
    ]
}
