//! Animations group — the animation list with arm/disarm/cancel/trigger controls.

use std::sync::{Arc, Mutex, RwLock};

use crate::render::overlay_ui::animation_dialog::AnimationDialog;
use crate::scene::{AnimState, SceneState};
use crate::vtl_state::VtlState;

pub(in crate::render::overlay_ui) fn animations_panel(
    ui: &mut egui::Ui,
    want_focus: bool,
    scene: &Arc<RwLock<SceneState>>,
    vtl: Option<&Mutex<VtlState>>,
    animation_dialog: &mut AnimationDialog,
) {
    let new_btn = ui.button("➕ New animation");
    if want_focus { new_btn.request_focus(); }
    if new_btn.clicked() { animation_dialog.open(); }
    ui.separator();
    if let Ok(mut sc) = scene.try_write() {
        let handles: Vec<u32> = sc.animations.keys().copied().collect();
        if handles.is_empty() {
            ui.label(egui::RichText::new("(no animations)")
                .color(egui::Color32::DARK_GRAY));
        }
        let mut arm: Option<u32> = None;
        let mut disarm: Option<u32> = None;
        let mut cancel: Option<u32> = None;
        let mut trigger: Option<u32> = None;
        let mut delete: Option<u32> = None;
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for h in &handles {
                if let Some(entry) = sc.animations.get(h) {
                    let (state_txt, state_col) = match entry.state {
                        AnimState::Idle           => ("Idle",    egui::Color32::GRAY),
                        AnimState::Armed          => ("Armed",   egui::Color32::YELLOW),
                        AnimState::Running { .. } => ("Running", egui::Color32::from_rgb(80,200,80)),
                        AnimState::Done           => ("Done",    egui::Color32::DARK_GRAY),
                    };
                    let name = if entry.name.is_empty() {
                        format!("anim #{h}")
                    } else { format!("#{h} {}", entry.name) };
                    ui.horizontal(|ui| {
                        ui.colored_label(state_col, format!("● {state_txt}"));
                        ui.label(format!("{name}  ({} stim)", entry.target.stimuli().len()));
                        // An animation the active condition excludes does not
                        // advance whatever its state says, so say so here
                        // rather than leave "Armed" looking like it is waiting.
                        if !entry.cond_enabled {
                            ui.colored_label(
                                egui::Color32::DARK_GRAY,
                                format!("[cond {:?}]", entry.conditions),
                            )
                            .on_hover_text("inactive: outside the active condition");
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("Arm").clicked() { arm = Some(*h); }
                        if ui.small_button("Disarm").clicked() { disarm = Some(*h); }
                        if ui.small_button("Cancel")
                            .on_hover_text("Clean teardown (applies configured cancel action)").clicked() {
                            cancel = Some(*h);
                        }
                        if ui.small_button("Trigger")
                            .on_hover_text("Fire start trigger or run now").clicked() {
                            trigger = Some(*h);
                        }
                        if ui.small_button("x")
                            .on_hover_text("Delete animation").clicked() {
                            delete = Some(*h);
                        }
                    });
                    ui.separator();
                }
            }
        });
        if let Some(h) = arm    { sc.arm_animation(h); }
        if let Some(h) = disarm { sc.disarm_animation(h); }
        if let Some(h) = cancel {
            // Seed from the staged levels so a cancel_action
            // level change is applied, then commit changed banks
            // back. Any pulse the cancel produces is handed to
            // VtlState::pulses, which the next commit publishes
            // for one frame.
            let mut levels = vtl
                .and_then(|v| v.try_lock().ok().map(|g| g.staged))
                .unwrap_or([0u64; ::vtl::MAX_BANKS]);
            let mut pulses = [0u64; ::vtl::MAX_BANKS];
            sc.cancel_animation(
                h,
                &mut crate::vtl_state::VtlOutputs {
                    levels: &mut levels,
                    pulses: &mut pulses,
                },
            );
            if let Some(v) = vtl
                && let Ok(mut g) = v.try_lock()
            {
                for (bank, &p) in pulses.iter().enumerate() {
                    g.pulses[bank] |= p;
                }
                for (bank, &val) in levels.iter().enumerate() {
                    if g.staged[bank] != val {
                        g.set_staged_bank(bank, val);
                    }
                }
            }
        }
        if let Some(h) = delete { sc.delete_animation(h); }
        if let Some(h) = trigger {
            let start_trigger = sc.animations.get(&h)
                .and_then(|e| e.start_trigger);
            sc.arm_animation(h);
            if let (Some((bit, edge)), Some(v)) = (start_trigger, vtl)
                && let Ok(vst) = v.try_lock()
            {
                let owner = vst.owner();
                let mask = 1u64 << bit.bit;
                match edge {
                    crate::scene::VtlEdge::Rising => {
                        owner.set_input_bit(bit.bank, bit.bit);
                        owner.set_input_rise(bit.bank, mask);
                    }
                    crate::scene::VtlEdge::Falling => {
                        owner.clear_input_bit(bit.bank, bit.bit);
                        owner.set_input_fall(bit.bank, mask);
                    }
                }
            }
        }
    }
}
