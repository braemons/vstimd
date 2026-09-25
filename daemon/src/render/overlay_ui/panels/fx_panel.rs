//! FX group — graphics debugging. Wireframe, and the knobs that decide what a
//! Gaussian splat actually draws.
//!
//! Everything here is render tuning: none of it is saved with a scene-config,
//! and the defaults are what the renderer drew before this panel existed. The
//! splat knobs change what reaches the screen, which is why they are knobs to
//! try rather than values chosen for you — see `scene::FxSettings`.

use std::sync::{Arc, RwLock};

use crate::scene::{FxSettings, SceneState};

pub(in crate::render::overlay_ui) fn fx_panel(
    ui: &mut egui::Ui,
    wireframe: Option<bool>,
    wireframe_toggle_requested: &mut bool,
    scene: &Arc<RwLock<SceneState>>,
) {
    ui.label(egui::RichText::new("Geometry").strong());
    if let Some(wf) = wireframe {
        if ui.button(if wf { "Wireframe: ON" } else { "Wireframe: off" }).clicked() {
            *wireframe_toggle_requested = true;
        }
    } else {
        ui.add_enabled(false, egui::Button::new("Wireframe: n/a"));
    }

    ui.add_space(6.0);
    ui.separator();
    ui.label(egui::RichText::new("Gaussian splats").strong());

    // Read once, edit a copy, write back only on a change: the render thread
    // wants this lock every frame, so the panel must not hold it while egui
    // lays widgets out.
    let mut fx = scene.read().expect("scene lock poisoned").runtime.fx;
    let before = fx;
    let splats = visible_splat_stimuli(scene);

    ui.add(
        egui::Slider::new(&mut fx.splat_alpha_floor, FxSettings::ALPHA_FLOOR_RANGE)
            .text("alpha floor")
            .suffix("/255")
            .fixed_decimals(0),
    )
    .on_hover_text(
        "Drop a splat's fragment once it is fainter than this, and shrink its \
         quad to match. 1 draws everything that could show. Splat scenes are \
         limited by the fragments that blend, so this is the one knob that \
         reliably buys frame time — and it does change what is on screen.",
    );

    ui.add(
        egui::Slider::new(&mut fx.splat_max_axis_px, FxSettings::MAX_AXIS_RANGE)
            .text("max axis")
            .suffix(" px")
            .logarithmic(true)
            .fixed_decimals(0),
    )
    .on_hover_text(
        "Longest a single splat's quad may be drawn. Bounds what one splat \
         close to the camera can cost; too low and near surfaces clip into \
         rectangles. Weak on the room capture (5% even clamped to 32 px), \
         because its fill comes from many moderate splats, not a few huge \
         ones -- a scene where it matters looks different.",
    );

    ui.horizontal(|ui| {
        if ui.button("Reset").clicked() {
            fx = FxSettings::default();
        }
        ui.label(
            egui::RichText::new(match splats {
                0 => "no splat stimulus visible".to_string(),
                1 => "1 splat stimulus visible".to_string(),
                n => format!("{n} splat stimuli visible"),
            })
            .weak(),
        );
    });

    if fx != before {
        scene.write().expect("scene lock poisoned").runtime.fx = fx;
    }
}

/// Visible `GaussianSplat3D` stimuli. Zero says the knobs above currently
/// change nothing, which is worth saying — the splat count itself lives in the
/// GPU cache on the render thread and is not the scene's to report.
fn visible_splat_stimuli(scene: &Arc<RwLock<SceneState>>) -> usize {
    let s = scene.read().expect("scene lock poisoned");
    s.config
        .stimuli
        .values()
        .filter(|e| e.stimulus.is_visible() && e.stimulus.gaussian_splat().is_some())
        .count()
}
