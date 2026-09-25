//! Scene-config group — save/load of the scene + VTL configuration.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::render::overlay_ui::file_browser::{BrowserMode, FileBrowser};
use crate::scene::SceneState;

/// Returns a quick-load request when one of the inline list buttons was hit.
/// The caller applies it *after* the panels are drawn, so the scene is never
/// written mid-draw while the stimuli panel may hold a write guard.
pub(in crate::render::overlay_ui) fn scene_config_panel(
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

    // Inline listing of every project's scene-configs, with quick-load buttons.
    // Grouped by project so an operator at the rig can see which study a scene
    // belongs to before loading it.
    ui.separator();
    ui.label(egui::RichText::new("Saved scene-configs").strong());
    match scene.try_read().ok().map(|sc| sc.runtime.storage_dir.clone()) {
        None => {
            ui.label(egui::RichText::new("(scene busy)").color(egui::Color32::DARK_GRAY));
        }
        Some(storage_dir) => {
            let projects =
                crate::scene_config_file::list_projects(&storage_dir).unwrap_or_default();
            let mut any = false;
            egui::ScrollArea::vertical().max_height(1000.0).show(ui, |ui| {
                for project in &projects {
                    let names =
                        crate::scene_config_file::list_scene_config_names(&storage_dir, project)
                            .unwrap_or_default();
                    if names.is_empty() {
                        continue;
                    }
                    any = true;
                    ui.label(egui::RichText::new(project).strong());
                    let dir = crate::scene_config_file::scene_config_dir(&storage_dir, project);
                    egui::Grid::new(format!("scene_config_list_{project}"))
                        .num_columns(3)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            for name in &names {
                                let path = dir.join(format!("{name}.config.json"));
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
                }
            });
            if !any {
                ui.label(egui::RichText::new("(none)").color(egui::Color32::DARK_GRAY));
            }
        }
    }

    quick_load
}
