//! Config group — save/load of the scene + VTL configuration.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::render::overlay_ui::file_browser::{BrowserMode, FileBrowser};
use crate::scene::SceneState;

/// Returns a quick-load request when one of the inline list buttons was hit.
/// The caller applies it *after* the panels are drawn, so the scene is never
/// written mid-draw while the stimuli panel may hold a write guard.
pub(in crate::render::overlay_ui) fn config_panel(
    ui: &mut egui::Ui,
    want_focus: bool,
    scene: &Arc<RwLock<SceneState>>,
    file_browser: &mut FileBrowser,
) -> Option<(BrowserMode, PathBuf)> {
    let mut quick_load = None;

    ui.label("Save or load the scene + VTL configuration.");
    ui.horizontal(|ui| {
        let save = ui.button("Save…");
        if want_focus { save.request_focus(); }
        if save.clicked() { file_browser.open_save(); }
        if ui.button("Open (replace)…").clicked() {
            file_browser.open_load_replace();
        }
        if ui.button("Open (additive)…").clicked() {
            file_browser.open_load_additive();
        }
    });

    // Inline listing of the config directory with quick-load buttons.
    ui.separator();
    ui.label(egui::RichText::new("Saved configs").strong());
    match scene.try_read().ok().map(|sc| sc.runtime.config_dir.clone()) {
        None => {
            ui.label(egui::RichText::new("(scene busy)").color(egui::Color32::DARK_GRAY));
        }
        Some(dir) => {
            let names = crate::scene_config_file::list_config_names(&dir).unwrap_or_default();
            if names.is_empty() {
                ui.label(egui::RichText::new("(none)").color(egui::Color32::DARK_GRAY));
            } else {
                egui::ScrollArea::vertical().max_height(1000.0).show(ui, |ui| {
                    egui::Grid::new("config_list").num_columns(3).spacing([8.0, 2.0])
                        .show(ui, |ui| {
                        for name in &names {
                            let path = dir.join(format!("vstimd_{name}.config.json"));
                            ui.label(name);
                            if ui.button("Load").clicked() {
                                quick_load = Some((BrowserMode::OpenReplace, path.clone()));
                            }
                            if ui.button("+").on_hover_text("Load additive").clicked() {
                                quick_load = Some((BrowserMode::OpenAdditive, path));
                            }
                            ui.end_row();
                        }
                    });
                });
            }
        }
    }

    quick_load
}
