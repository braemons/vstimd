//! Recording the 3-D pass. Called from `render_frame` only on frames that have
//! something 3-D to draw; a pure 2-D frame never reaches this file.

use ash::vk;

use crate::render::vk::cache::{Mesh3dCache, SplatCache};
use crate::render::vk::{
    Mesh3dPushConstants, Mesh3dRenderer, Pass3d, SceneUniform, SplatPushConstants, SplatRenderer,
    VeilPushConstants, VkContext,
};
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
    splat: Option<&SplatRenderer>,
    splats: &SplatCache,
) {
    let extent = ctx.extent;
    let aspect = extent.width as f32 / extent.height.max(1) as f32;
    let camera = scene.camera.live;
    let lighting = scene.lighting.live;
    mesh3d.uniforms.write(
        frame_slot,
        &SceneUniform {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            camera_pos: camera.position_cm.0.to_array(),
            ambient: lighting.ambient_color,
            sun_dir: lighting.sun_direction_normalized().to_array(),
            sun_color: lighting.sun_color,
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
            let key = m.geometry.live.mesh_key();
            let Some(mesh) = meshes.get(key) else {
                continue;
            };
            if bound != Some(key) {
                device.cmd_bind_vertex_buffers(cb, 0, &[mesh.vertex_buffer], &[0]);
                device.cmd_bind_index_buffer(cb, mesh.index_buffer, 0, vk::IndexType::UINT32);
                bound = Some(key);
            }
            let material = m.material.live;
            let albedo = material.albedo.scaled_alpha(stim.opacity().live);
            let shading = match material.shading {
                Shading3D::Unlit => 0,
                Shading3D::Phong => 1,
            };
            // One draw per instance: a corridor is tens of planes sharing the
            // bound mesh, so each costs one push-constant write.
            m.for_each_instance(|model, tint| {
                let pc = Mesh3dPushConstants {
                    model: model.to_cols_array_2d(),
                    albedo: [
                        albedo.r * tint.r,
                        albedo.g * tint.g,
                        albedo.b * tint.b,
                        albedo.a * tint.a,
                    ],
                    emissive: material.emissive,
                    shading,
                };
                device.cmd_push_constants(
                    cb,
                    pipe.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&pc),
                );
                device.cmd_draw_indexed(cb, mesh.index_count, 1, 0, 0, 0);
            });
        }

        // Splats after every mesh: they test against the meshes' depth but do
        // not write it, and blend over whatever is behind them.
        if let Some(sr) = splat {
            let view = camera.view_matrix();
            let proj = camera.proj_matrix(aspect);
            let proj_params = [proj.x_axis.x, proj.y_axis.y, proj.z_axis.z, proj.w_axis.z];
            let mut bound = false;
            for (handle, entry) in scene.stimuli.iter() {
                let stim = &entry.stimulus;
                let Some(g) = stim.gaussian_splat() else { continue };
                if !stim.is_visible() {
                    continue;
                }
                let Some(cloud) = splats.get(*handle) else { continue };
                if !bound {
                    device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, sr.pipeline);
                    device.cmd_set_viewport(cb, 0, std::slice::from_ref(&viewport));
                    device.cmd_set_scissor(cb, 0, std::slice::from_ref(&render_area));
                    bound = true;
                }
                device.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    sr.layout,
                    0,
                    &[cloud.sets[frame_slot]],
                    &[],
                );
                let pc = SplatPushConstants {
                    model_view: (view * g.transform.live.model_matrix(glam::Vec3::ONE))
                        .to_cols_array_2d(),
                    proj: proj_params,
                    viewport_px: [extent.width as f32, extent.height as f32],
                    opacity: stim.opacity().live,
                    _pad: 0.0,
                };
                device.cmd_push_constants(
                    cb,
                    sr.layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    bytemuck::bytes_of(&pc),
                );
                device.cmd_draw(cb, 4, cloud.count, 0, 0);
            }

            // The veil: the whole 3-D view faded towards the background, over
            // splats and meshes alike, under every 2-D stimulus.
            let fade = scene.view_fade_3d();
            if fade > 0.0 {
                let [r, g, b, _] = background.float32;
                device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, sr.veil_pipeline);
                device.cmd_set_viewport(cb, 0, std::slice::from_ref(&viewport));
                device.cmd_set_scissor(cb, 0, std::slice::from_ref(&render_area));
                device.cmd_push_constants(
                    cb,
                    sr.veil_layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&VeilPushConstants { color: [r, g, b, fade.min(1.0)] }),
                );
                device.cmd_draw(cb, 3, 1, 0, 0);
            }
        }

        ctx.cmd_end_label(cb);
        device.cmd_end_render_pass(cb);
    }
}
