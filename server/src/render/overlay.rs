use crate::scene::SceneState;
use crate::timing::FrameSummary;

pub struct OverlayRenderer {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    winit_state: egui_winit::State,
    /// Stored between `prepare()` and `paint()` each frame.
    prepared: Option<egui::FullOutput>,
}

impl OverlayRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        window: &winit::window::Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> Self {
        let ctx = egui::Context::default();
        let renderer = egui_wgpu::Renderer::new(
            device,
            surface_format,
            egui_wgpu::RendererOptions::default(),
        );
        let winit_state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            None,
            None,
            None,
        );
        Self { ctx, renderer, winit_state, prepared: None }
    }

    pub fn on_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.winit_state.on_window_event(window, event)
    }

    /// Run the egui UI and store the output for `paint()`.
    ///
    /// Call this while the scene read lock is held — it reads `scene` directly
    /// by reference, no data is copied out. The closure passed to `ctx.run()`
    /// is synchronous so the lock is held only for the duration of UI layout
    /// (< 0.5 ms for typical scene sizes).
    ///
    /// Returns the handles of stimuli whose enabled checkbox was toggled.
    /// The caller applies these in the next `update()` under the write lock.
    pub fn prepare(
        &mut self,
        window: &winit::window::Window,
        stats: &FrameSummary,
        scene: &SceneState,
    ) -> Vec<u32> {
        let mut toggles: Vec<u32> = Vec::new();

        let raw_input = self.winit_state.take_egui_input(window);
        let full_output = self.ctx.run(raw_input, |ctx| {
            // ── Frame Timing ──────────────────────────────────────────────────
            egui::Window::new("Frame Timing")
                .default_pos([8.0, 8.0])
                .resizable(false)
                .show(ctx, |ui| {
                    let fps_color = if stats.fps < 55.0 {
                        egui::Color32::RED
                    } else if stats.fps < 58.0 {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::GREEN
                    };
                    ui.colored_label(fps_color, format!("FPS:    {:5.1}", stats.fps));

                    let jitter_color = if stats.std_ms > 1.0 {
                        egui::Color32::RED
                    } else if stats.std_ms > 0.3 {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::GREEN
                    };
                    ui.colored_label(
                        jitter_color,
                        format!("Jitter: {:5.2} ms (std)", stats.std_ms),
                    );
                    ui.label(format!("Mean:   {:5.2} ms", stats.mean_ms));
                    ui.label(format!("Min:    {:5.2} ms", stats.min_ms));
                    ui.label(format!("Max:    {:5.2} ms", stats.max_ms));

                    let drop_color = if stats.drop_count >= 3 {
                        egui::Color32::RED
                    } else if stats.drop_count >= 1 {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::GREEN
                    };
                    ui.colored_label(drop_color, format!("Drops:  {}", stats.drop_count));
                    ui.label(format!("Frame:  {}", stats.frame_index));
                });

            // ── Stimuli ───────────────────────────────────────────────────────
            egui::Window::new("Stimuli")
                .default_pos([8.0, 180.0])
                .resizable(true)
                .show(ctx, |ui| {
                    if scene.stimuli.is_empty() {
                        ui.label("(none)");
                        return;
                    }
                    egui::Grid::new("stim_table")
                        .striped(true)
                        .num_columns(5)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.strong("H");
                            ui.strong("Type");
                            ui.strong("On");
                            ui.strong("X");
                            ui.strong("Y");
                            ui.end_row();

                            for (handle, stim) in &scene.stimuli {
                                ui.monospace(handle.to_string());
                                ui.label(stim.type_name());
                                let mut enabled = stim.flags().enabled;
                                if ui.checkbox(&mut enabled, "").changed() {
                                    toggles.push(*handle);
                                }
                                let [x, y] = stim.get_pos();
                                ui.monospace(format!("{:7.1}", x));
                                ui.monospace(format!("{:7.1}", y));
                                ui.end_row();
                            }
                        });
                });

            // ── Commands ──────────────────────────────────────────────────────
            egui::Window::new("Commands")
                .default_pos([8.0, 370.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Total: {}   Errors: {}",
                        scene.command_log_total, scene.command_log_errors
                    ));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            egui::Grid::new("cmd_table")
                                .striped(true)
                                .num_columns(3)
                                .spacing([8.0, 2.0])
                                .show(ui, |ui| {
                                    for entry in scene.command_log.iter().rev() {
                                        ui.monospace(format!("{:9.1}ms", entry.elapsed_ms));
                                        let color = if entry.ok {
                                            egui::Color32::LIGHT_GRAY
                                        } else {
                                            egui::Color32::RED
                                        };
                                        let prefix = if entry.handle == 0 {
                                            "sys".to_string()
                                        } else {
                                            format!("h={}", entry.handle)
                                        };
                                        ui.colored_label(
                                            color,
                                            format!("{}: {}", prefix, entry.summary),
                                        );
                                        let resp = if entry.ok {
                                            format!("→ {}", entry.response)
                                        } else {
                                            "ERR".into()
                                        };
                                        ui.monospace(resp);
                                        ui.end_row();
                                    }
                                });
                        });
                });
        });

        self.winit_state.handle_platform_output(window, full_output.platform_output.clone());
        self.prepared = Some(full_output);
        toggles
    }

    /// Upload GPU buffers and draw the overlay.
    ///
    /// Call this after the scene read lock has been released — it only touches
    /// GPU resources.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &winit::window::Window,
        pixels_per_point: f32,
    ) {
        let Some(full_output) = self.prepared.take() else { return };

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
            let mut rp = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
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
                })
                .forget_lifetime();
            self.renderer.render(&mut rp, &tris, &screen_desc);
        }

        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }
}
