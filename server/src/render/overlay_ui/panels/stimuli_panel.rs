//! Stimuli group — spawn dialog plus a live table of the scene's stimuli.

use std::sync::{Arc, RwLock};

use crate::render::overlay_ui::stimulus_dialog::StimulusDialog;
use crate::scene::{SceneState, Stimulus};

pub(in crate::render::overlay_ui) fn stimuli_panel(
    ui: &mut egui::Ui,
    want_focus: bool,
    scene: &Arc<RwLock<SceneState>>,
    stimulus_dialog: &mut StimulusDialog,
) {
    ui.horizontal(|ui| {
        let new_btn = ui.button("➕ New stimulus");
        if want_focus { new_btn.request_focus(); }
        if new_btn.clicked() { stimulus_dialog.open(); }
        if ui.button("Spawn demo").clicked() {
            crate::render::spawn_demo_stimuli(scene);
        }
    });
    ui.separator();
    if let Ok(mut sc) = scene.try_write() {
        let handles: Vec<u32> = sc.stimuli.keys().copied().collect();
        let mut to_delete: Option<u32> = None;
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            egui::Grid::new("stimuli_grid").striped(true).num_columns(6)
                .spacing([8.0, 2.0]).show(ui, |ui| {
                ui.label(egui::RichText::new("En").strong());
                ui.label(egui::RichText::new("Handle / type").strong());
                ui.label(egui::RichText::new("Name").strong());
                ui.label(egui::RichText::new("Pos (px)").strong());
                ui.label(egui::RichText::new("Size (px)").strong());
                ui.label("");
                ui.end_row();
                for h in handles {
                    if let Some(entry) = sc.stimuli.get_mut(&h) {
                        let stim = &entry.stimulus;
                        let type_name = stim.type_name();
                        let pos = stim.transform().live.pos;
                        let size_label = match stim {
                            Stimulus::Grating(s) => {
                                let [w, h] = s.size.live;
                                format!("{}×{}", w as i32, h as i32)
                            }
                            Stimulus::Rect(s) => {
                                let [w, h] = s.size.live;
                                format!("{}×{}", w as i32, h as i32)
                            }
                            Stimulus::Circle(s) =>
                                format!("r={}", s.radius.live as i32),
                            Stimulus::Ellipse(s) => {
                                let [w, h] = s.size.live;
                                format!("{}×{}", w as i32, h as i32)
                            }
                            Stimulus::Text(s) => {
                                let [w, h] = s.box_size.live;
                                format!("{}×{}", w as i32, h as i32)
                            }
                        };
                        let name_label = entry.name.as_deref().unwrap_or("");
                        let uuid_str = entry.id.to_string();
                        let flags = entry.stimulus.flags_mut();
                        ui.checkbox(&mut flags.enabled, "");
                        ui.label(format!("#{h} {type_name}"));
                        let disp = if name_label.is_empty() {
                            &uuid_str[..8]
                        } else { name_label };
                        ui.label(egui::RichText::new(disp).color(
                            if name_label.is_empty() {
                                egui::Color32::DARK_GRAY
                            } else { egui::Color32::WHITE }
                        )).on_hover_text(&uuid_str);
                        ui.label(egui::RichText::new(
                            format!("{:>6.0},{:>6.0}", pos[0], pos[1])
                        ).monospace());
                        ui.label(size_label);
                        if ui.small_button("x")
                            .on_hover_text("Delete stimulus").clicked() {
                            to_delete = Some(h);
                        }
                        ui.end_row();
                    }
                }
            });
        });
        if let Some(h) = to_delete { sc.stimuli.shift_remove(&h); }
    }
}
