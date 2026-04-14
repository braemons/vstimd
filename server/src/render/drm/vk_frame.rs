use std::sync::{Arc, RwLock};

use ash::vk;

use crate::scene::SceneState;
use crate::timing::FrameStats;

use super::input::{AppKey, InputState};
use super::vk_buffers::GpuBuffers;
use super::vk_init::VkContext;
use super::vk_pipeline::VkPipeline;
use crate::render::tess;

/// Run the compositor-free render loop until ESC is pressed or the process is
/// signalled. Returns when the loop exits cleanly.
pub fn run_loop(
    ctx: &VkContext,
    pipeline: &VkPipeline,
    gpu_buffers: &mut GpuBuffers,
    scene: Arc<RwLock<SceneState>>,
    input: &mut InputState,
    frame_stats: &mut FrameStats,
    show_overlay: &mut bool,
) {
    let mut frame_index: usize = 0;

    loop {
        // ── Input ─────────────────────────────────────────────────────────────
        for key in input.poll() {
            match key {
                AppKey::Escape => return,
                AppKey::F1 => *show_overlay = !*show_overlay,
                AppKey::D => spawn_demo_stimuli(&scene),
            }
        }

        // ── Update: tessellate scene into GPU buffers ─────────────────────────
        {
            let fps = frame_stats.summary().fps as f32;
            let screen_size = (ctx.extent.width, ctx.extent.height);

            let mut sc = scene.write().expect("scene lock poisoned");
            if sc.pending_flip {
                sc.apply_flip();
            }
            sc.screen_size = screen_size;
            sc.frame_rate = fps;

            gpu_buffers.meshes.retain(|h, _| sc.stimuli.contains_key(h));

            let handles: Vec<u32> = sc.stimuli.keys().copied().collect();
            for handle in handles {
                let (verts, idxs) =
                    tess::tessellate_stimulus(&sc.stimuli[&handle], screen_size);
                gpu_buffers.upload(handle, &ctx.device, &verts, &idxs);
            }
        } // write lock dropped — ZMQ thread can run

        let frame = &ctx.frames[frame_index % ctx.frames.len()];

        // ── Wait for this frame slot's previous GPU work to finish ────────────
        unsafe {
            ctx.device
                .wait_for_fences(&[frame.in_flight], true, u64::MAX)
                .expect("fence wait failed");
            ctx.device.reset_fences(&[frame.in_flight]).expect("fence reset failed");
        }

        // ── Acquire swapchain image ───────────────────────────────────────────
        let (image_index, _suboptimal) = unsafe {
            ctx.swapchain_loader.acquire_next_image(
                ctx.swapchain,
                u64::MAX,
                frame.image_available,
                vk::Fence::null(),
            )
        }
        .expect("failed to acquire swapchain image");

        // ── Record command buffer ─────────────────────────────────────────────
        let cb = frame.command_buffer;
        unsafe {
            ctx.device
                .reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())
                .expect("command buffer reset failed");

            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            ctx.device.begin_command_buffer(cb, &begin_info).expect("begin_command_buffer failed");

            // Clear colour from the scene's background field.
            let bg = {
                let sc = scene.read().expect("scene lock poisoned");
                let b = sc.background.live;
                vk::ClearColorValue { float32: b }
            };

            let clear_value = vk::ClearValue { color: bg };
            let render_area =
                vk::Rect2D { offset: vk::Offset2D::default(), extent: ctx.extent };

            let rp_info = vk::RenderPassBeginInfo::default()
                .render_pass(ctx.render_pass)
                .framebuffer(ctx.framebuffers[image_index as usize])
                .render_area(render_area)
                .clear_values(std::slice::from_ref(&clear_value));

            ctx.device.cmd_begin_render_pass(cb, &rp_info, vk::SubpassContents::INLINE);
            ctx.device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);

            // Dynamic viewport + scissor covering the full display.
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: ctx.extent.width as f32,
                height: ctx.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            ctx.device.cmd_set_viewport(cb, 0, std::slice::from_ref(&viewport));
            ctx.device.cmd_set_scissor(cb, 0, std::slice::from_ref(&render_area));

            // Draw stimuli in insertion order (= draw order).
            let sc = scene.read().expect("scene lock poisoned");
            for (handle, _) in &sc.stimuli {
                if let Some(mesh) = gpu_buffers.meshes.get(handle) {
                    if mesh.index_count > 0 {
                        ctx.device.cmd_bind_vertex_buffers(
                            cb,
                            0,
                            &[mesh.vertex_buffer],
                            &[0],
                        );
                        ctx.device.cmd_bind_index_buffer(
                            cb,
                            mesh.index_buffer,
                            0,
                            vk::IndexType::UINT32,
                        );
                        ctx.device.cmd_draw_indexed(cb, mesh.index_count, 1, 0, 0, 0);
                    }
                }
            }
            drop(sc); // release read lock before submit

            ctx.device.cmd_end_render_pass(cb);
            ctx.device.end_command_buffer(cb).expect("end_command_buffer failed");
        }

        // ── Submit ────────────────────────────────────────────────────────────
        let wait_semaphores = [frame.image_available];
        let signal_semaphores = [frame.render_done];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [cb];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores);

        unsafe {
            ctx.device
                .queue_submit(ctx.graphics_queue, &[submit_info], frame.in_flight)
                .expect("queue_submit failed");
        }

        // ── Present (FIFO blocks here at vblank) ──────────────────────────────
        let swapchains = [ctx.swapchain];
        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            ctx.swapchain_loader
                .queue_present(ctx.graphics_queue, &present_info)
                .expect("queue_present failed");
        }

        frame_stats.on_present();
        frame_index = frame_index.wrapping_add(1);
    }
}

// ── Demo stimuli ──────────────────────────────────────────────────────────────

fn spawn_demo_stimuli(scene: &Arc<RwLock<SceneState>>) {
    use crate::scene::{
        Deferred, DiscStimulus, RectStimulus, ShapeAppearance, Stimulus, StimulusFlags, Transform2D,
    };

    let mut sc = scene.write().expect("scene lock poisoned");

    let h1 = sc.alloc_stim_handle();
    sc.stimuli.insert(
        h1,
        Stimulus::Disc(DiscStimulus {
            flags: StimulusFlags { enabled: true, ..Default::default() },
            transform: Deferred::new(Transform2D { pos: [-150.0, 0.0], angle: 0.0 }),
            appearance: Deferred::new(ShapeAppearance {
                fill_color: [0.0, 0.8, 0.8, 1.0],
                ..Default::default()
            }),
            radius: Deferred::new(80.0),
        }),
    );

    let h2 = sc.alloc_stim_handle();
    sc.stimuli.insert(
        h2,
        Stimulus::Rect(RectStimulus {
            flags: StimulusFlags { enabled: true, ..Default::default() },
            transform: Deferred::new(Transform2D { pos: [150.0, 0.0], angle: 30.0 }),
            appearance: Deferred::new(ShapeAppearance {
                fill_color: [0.8, 0.0, 0.8, 1.0],
                ..Default::default()
            }),
            size: Deferred::new([120.0, 50.0]),
        }),
    );

    eprintln!("Demo: spawned disc (handle {h1}) and rect (handle {h2})");
}
