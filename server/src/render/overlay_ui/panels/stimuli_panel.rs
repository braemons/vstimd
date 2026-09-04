//! Stimuli group — spawn dialog plus a live table of the scene's stimuli.

use std::sync::{Arc, RwLock};

use crate::render::overlay_ui::stimulus_dialog::StimulusDialog;
use crate::scene::{SceneState, ShapeGeometry, StimulusBody};

/// The active-condition strip: what is active, and the two buttons that step
/// through the protocol. It sits above the stimulus table because that table is
/// where the effect shows — a stimulus the active condition excludes is drawn
/// greyed, with its own `enabled` checkbox untouched.
fn condition_bar(ui: &mut egui::Ui, sc: &mut SceneState) {
    ui.horizontal(|ui| {
        ui.label("Condition:");
        ui.label(
            egui::RichText::new(sc.conditions.active_label())
                .strong()
                .color(egui::Color32::from_rgb(120, 180, 255)),
        );
        let active = sc.conditions.active;
        if ui
            .add_enabled(active > 0, egui::Button::new("◀").small())
            .on_hover_text("Previous condition")
            .clicked()
        {
            sc.set_condition(active - 1);
        }
        if ui.small_button("▶").on_hover_text("Next condition").clicked() {
            sc.set_condition(active + 1);
        }
        let declared = sc.conditions.declared.len();
        if declared > 0 {
            ui.label(
                egui::RichText::new(format!("({declared} declared)"))
                    .color(egui::Color32::DARK_GRAY),
            );
        }
    });
}

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
        condition_bar(ui, &mut sc);
        ui.separator();
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
                        // 3-D stimuli have no pixel position to show; the panel
                        // is a 2-D scene table for now.
                        let pos_px = stim.transform2d().map(|t| t.live.pos_px);
                        let wh = |[w, h]: [f32; 2]| format!("{}×{}", w as i32, h as i32);
                        let size_label = match &stim.body {
                            StimulusBody::Grating(s) => wh(s.size_px.live),
                            StimulusBody::Text(s) => wh(s.box_size_px.live),
                            StimulusBody::Shape(s) => match s.geometry.live {
                                ShapeGeometry::Rect { size_px }
                                | ShapeGeometry::Ellipse { size_px } => wh(size_px),
                                ShapeGeometry::Circle { diameter_px } => {
                                    format!("d={}", diameter_px as i32)
                                }
                            },
                            // The field, not the aperture: it is the extent the
                            // dots occupy, and the aperture is a mask over it.
                            StimulusBody::Dots(d) => {
                                format!("{} dots in {}", d.params.live.dot_count,
                                        wh(d.params.live.field_size_px))
                            }
                            StimulusBody::Mesh3d(_) => "3-D".to_string(),
                        };
                        let name_label = entry.name().to_string();
                        let uuid_str = entry.id().to_string();
                        let conditions = entry.conditions.clone();
                        let flags = entry.stimulus.flags_mut();
                        // Grey the row when the active condition excludes it:
                        // the checkbox still shows what the operator asked for,
                        // which is not what is on screen.
                        let cond_on = flags.cond_enabled;
                        ui.checkbox(&mut flags.enabled, "");
                        let handle_label = egui::RichText::new(format!("#{h} {type_name}"));
                        let handle_label = if cond_on {
                            handle_label
                        } else {
                            handle_label.color(egui::Color32::DARK_GRAY)
                        };
                        let cond_hover = if conditions.is_empty() {
                            "conditions: all".to_string()
                        } else {
                            format!("conditions: {conditions:?}")
                        };
                        ui.label(handle_label).on_hover_text(cond_hover);
                        let disp = if name_label.is_empty() {
                            &uuid_str[..8]
                        } else { name_label.as_str() };
                        ui.label(egui::RichText::new(disp).color(
                            if name_label.is_empty() {
                                egui::Color32::DARK_GRAY
                            } else { egui::Color32::WHITE }
                        )).on_hover_text(&uuid_str);
                        ui.label(
                            egui::RichText::new(match pos_px {
                                Some(p) => format!("{:>6.0},{:>6.0}", p[0], p[1]),
                                None => "     —,     —".to_string(),
                            })
                            .monospace(),
                        );
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
