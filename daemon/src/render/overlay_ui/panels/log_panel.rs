//! Log group — the in-process server log plus the IPC command log.

use std::sync::{Arc, RwLock};

use crate::log_buffer::LogBuffer;
use crate::scene::SceneState;

pub(in crate::render::overlay_ui) fn log_panel(
    ui: &mut egui::Ui,
    scene: &Arc<RwLock<SceneState>>,
    log_buffer: &LogBuffer,
) {
    ui.label(egui::RichText::new("Server log").strong());
    let entries = log_buffer.lock()
        .map(|buf| buf.iter().map(|e| {
            let color = match e.level {
                log::Level::Error => egui::Color32::RED,
                log::Level::Warn  => egui::Color32::YELLOW,
                log::Level::Info  => egui::Color32::WHITE,
                _                 => egui::Color32::GRAY,
            };
            (color, format!("[{:>8.1}ms] {:5} {}", e.elapsed_ms, e.level, e.message))
        }).collect::<Vec<_>>())
        .unwrap_or_default();
    egui::ScrollArea::vertical().id_salt("server_log")
        .stick_to_bottom(true).max_height(160.0).show(ui, |ui| {
        for (color, text) in entries { ui.colored_label(color, text); }
    });
    ui.separator();
    if let Ok(sc) = scene.try_read() {
        ui.label(egui::RichText::new(format!(
            "IPC commands: {}  errors: {}",
            sc.runtime.command_log_total, sc.runtime.command_log_errors,
        )).strong());
        egui::ScrollArea::vertical().id_salt("ipc_log")
            .stick_to_bottom(true).max_height(140.0).show(ui, |ui| {
            for entry in &sc.runtime.command_log {
                let color = if entry.ok {
                    egui::Color32::from_rgb(80, 200, 80)
                } else { egui::Color32::RED };
                ui.colored_label(color, format!(
                    "[{:>8.1}ms] #{} {} → {}",
                    entry.elapsed_ms, entry.handle, entry.summary,
                    if entry.ok { format!("ok ({})", entry.response) }
                    else { "err".to_string() },
                ));
            }
        });
    }
}
