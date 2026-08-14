//! Steps shared by every backend's render loop.
//!
//! The DRM, evdi, and desktop backends differ at the two ends of a frame —
//! how the frame clock is derived, and how the finished image reaches the
//! display — but the middle is the same everywhere: apply app-level keys,
//! build the overlay's raw input, advance animations against the VTL. Those
//! live here so a backend's loop is only the part that is actually specific
//! to it.

use crate::vtl_state::VtlOutputs;
use std::sync::{Arc, Mutex, RwLock};

use crate::render::AppKey;
use crate::render::render_state::RenderState;
use crate::scene::SceneState;
use crate::vtl_state::VtlState;

/// What the caller still has to do after [`apply_app_key`] handled a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyOutcome {
    /// Fully handled — nothing left for the backend to do.
    Handled,
    /// A VT switch was requested. Backend-specific: the DRM backend must
    /// route it through its VT guard (it holds the VT in `VT_PROCESS` mode,
    /// so the switch needs an explicit release handshake), while a backend
    /// that owns no VT can ignore it or forward it directly.
    SwitchVt(u16),
}

/// Apply one app-level key that means the same thing in every backend.
///
/// Everything that only touches overlay state, the scene, or the shutdown
/// flag is handled here; anything needing backend-specific machinery comes
/// back through [`KeyOutcome`].
pub(crate) fn apply_app_key(key: AppKey, rs: &mut RenderState) -> KeyOutcome {
    log::debug!("vstimd: app key {key:?}");
    match key {
        AppKey::Quit => crate::shutdown::request(),
        // Esc never quits — it closes a dialog or hides the overlay.
        // (Quit via Ctrl+Q, SIGINT, or a VT switch away then kill.)
        AppKey::Escape => {
            if let Some(ui) = &mut rs.ui {
                ui.overlay.handle_escape();
            }
        }
        AppKey::ToggleOverlay => {
            if let Some(ui) = &mut rs.ui {
                ui.overlay.toggle_master();
            }
        }
        AppKey::ShowGroup(group) => {
            if let Some(ui) = &mut rs.ui {
                ui.overlay.show_group(group);
            }
        }
        AppKey::HideGroup(group) => {
            if let Some(ui) = &mut rs.ui {
                ui.overlay.hide_group(group);
            }
        }
        AppKey::SwitchVt(n) => return KeyOutcome::SwitchVt(n),
        // Demo spawn only when the overlay is hidden, so 'd' types into
        // dialog fields while the overlay is up.
        AppKey::D => {
            let overlay_up = rs.ui.as_ref().is_some_and(|ui| ui.overlay.master_visible);
            if !overlay_up {
                crate::render::spawn_demo_stimuli(&rs.scene_renderer.scene);
            }
        }
    }
    KeyOutcome::Handled
}

/// Build the overlay's `egui::RawInput` for this frame, or `None` when the
/// overlay is hidden (in which case `render_frame` skips the whole egui pass).
///
/// Screen rect comes from the swapchain extent — neither bare-console backend
/// has a window system to report a size or a DPI scale.
pub(crate) fn overlay_raw_input(
    rs: &RenderState,
    nav_events: Vec<egui::Event>,
) -> Option<egui::RawInput> {
    if !rs.ui.as_ref().is_some_and(|ui| ui.overlay.master_visible) {
        return None;
    }
    Some(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(rs.ctx.extent.width as f32, rs.ctx.extent.height as f32),
        )),
        viewports: std::iter::once((
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(1.0), // TODO: compute from EDID DPI or make configurable
                ..Default::default()
            },
        ))
        .collect(),
        events: nav_events,
        ..Default::default()
    })
}

/// Commit staged VTL outputs, poll inputs, and advance scene animations by
/// one frame.
///
/// The VTL lock is taken twice on purpose, and never across the scene write
/// lock: the output channels are copied out, animations run holding only the
/// scene lock, then the updated channels are written back. Holding both at once
/// would put the ZMQ thread behind the render thread's scene lock.
///
/// `commit_staged` has just published (and cleared) the previous frame's
/// pulses, so `pulses` starts this frame empty and collects only what the
/// animations mark now — that one-frame life is what keeps an event mark an
/// edge rather than a line that latches HIGH on the first trial.
pub(crate) fn advance_frame(vtl: Option<&Arc<Mutex<VtlState>>>, scene: &Arc<RwLock<SceneState>>) {
    let (input_edges, output_edges, mut levels, mut pulses) = vtl
        .map(|v| {
            let mut g = v.lock().expect("vtl lock poisoned");
            g.commit_staged();
            let input_edges = g.poll();
            let output_edges = g.output_edges();
            (input_edges, output_edges, g.staged, g.pulses)
        })
        .unwrap_or_default();

    scene
        .write()
        .expect("scene lock poisoned")
        .advance_animations(
            &input_edges,
            &output_edges,
            &mut VtlOutputs { levels: &mut levels, pulses: &mut pulses },
        );

    if let Some(v) = vtl {
        let mut g = v.lock().expect("vtl lock poisoned");
        g.staged = levels;
        g.pulses = pulses;
    }
}
