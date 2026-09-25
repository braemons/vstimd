//! Benchmarks group — controls for the grating stress test.

use std::sync::{Arc, RwLock};

use crate::benchmark::BenchmarkState;
use crate::render::StimulusDisplayInfo;
use crate::scene::SceneState;
use crate::timing::FrameStats;

pub(in crate::render::overlay_ui) fn benchmarks_panel(
    ui: &mut egui::Ui,
    want_focus: bool,
    benchmark: &mut BenchmarkState,
    scene: &Arc<RwLock<SceneState>>,
    frame_stats: &mut FrameStats,
    display: &StimulusDisplayInfo,
) {
    ui.heading("Grating stress test");
    if benchmark.is_running() {
        let remaining = benchmark.remaining_frames(frame_stats).unwrap_or(0);
        ui.label(format!("Running… {remaining} frames remaining"));
    } else {
        let run = ui.button("Run (200 gratings, 300 frames)");
        if want_focus { run.request_focus(); }
        if run.clicked() {
            benchmark.start_grating_stress(scene, frame_stats,
                (display.width_px, display.height_px), 20, 10, 300);
        }
        if let Some(r) = benchmark.last_result() {
            ui.separator();
            ui.label(format!(
                "{} gratings × {} frames → {} dropped",
                r.grating_count, r.duration_frames, r.drop_count,
            ));
        }
    }
}
