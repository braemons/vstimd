use crate::timing::FrameSummary;

pub struct OverlayRenderer {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    winit_state: egui_winit::State,
}

impl OverlayRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        window: &winit::window::Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> Self {
        let ctx = egui::Context::default();
        let renderer = egui_wgpu::Renderer::new(device, surface_format, None, 1, false);
        let winit_state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            None,
            None,
            None,
        );
        Self { ctx, renderer, winit_state }
    }

    pub fn on_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.winit_state.on_window_event(window, event)
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &winit::window::Window,
        stats: &FrameSummary,
        pixels_per_point: f32,
    ) {
        let raw_input = self.winit_state.take_egui_input(window);
        let full_output = self.ctx.run(raw_input, |ctx| {
            egui::Window::new("Frame Timing")
                .default_pos([8.0, 8.0])
                .resizable(false)
                .show(ctx, |ui| {
                    let fps_color = if stats.fps < 55.0 { egui::Color32::RED }
                        else if stats.fps < 58.0 { egui::Color32::YELLOW }
                        else { egui::Color32::GREEN };
                    ui.colored_label(fps_color, format!("FPS:    {:5.1}", stats.fps));

                    let jitter_color = if stats.std_ms > 1.0 { egui::Color32::RED }
                        else if stats.std_ms > 0.3 { egui::Color32::YELLOW }
                        else { egui::Color32::GREEN };
                    ui.colored_label(jitter_color, format!("Jitter: {:5.2} ms (std)", stats.std_ms));
                    ui.label(format!("Mean:   {:5.2} ms", stats.mean_ms));
                    ui.label(format!("Min:    {:5.2} ms", stats.min_ms));
                    ui.label(format!("Max:    {:5.2} ms", stats.max_ms));

                    let drop_color = if stats.drop_count >= 3 { egui::Color32::RED }
                        else if stats.drop_count >= 1 { egui::Color32::YELLOW }
                        else { egui::Color32::GREEN };
                    ui.colored_label(drop_color, format!("Drops:  {}", stats.drop_count));
                    ui.label(format!("Frame:  {}", stats.frame_index));
                });
        });

        self.winit_state.handle_platform_output(window, full_output.platform_output);
        let tris = self.ctx.tessellate(full_output.shapes, pixels_per_point);
        for (id, delta) in full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, id, &delta);
        }
        let screen_desc = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [window.inner_size().width, window.inner_size().height],
            pixels_per_point,
        };
        self.renderer.update_buffers(device, queue, encoder, &tris, &screen_desc);
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.renderer.render(&mut rp, &tris, &screen_desc);
        }
        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }
}
