//! System group — host/display/clock identity, resource meters, and the
//! per-frame timing summary with its frame-duration histogram.

use std::sync::{Arc, RwLock};

use crate::render::StimulusDisplayInfo;
use crate::scene::SceneState;
use crate::system_info::{ClockSource, SystemInfo};
use crate::system_metrics::SystemMetrics;
use crate::timing::{FramePhases, FrameStats};

#[allow(clippy::too_many_arguments)]
pub(in crate::render::overlay_ui) fn system_panel(
    ui: &mut egui::Ui,
    sys: &SystemInfo,
    display: &StimulusDisplayInfo,
    wireframe: Option<bool>,
    metrics: &SystemMetrics,
    scene: &Arc<RwLock<SceneState>>,
    wireframe_toggle_requested: &mut bool,
) {
    ui.label(format!("HW: {}", sys.host.hardware_model));
    let mode_suffix = display.mode_index.map(|i| format!("  [mode {i}]")).unwrap_or_default();
    ui.label(format!(
        "Screen: {}×{}@{:.3} Hz{}",
        display.width_px, display.height_px, display.refresh_hz, mode_suffix,
    ));
    ui.label(format!("Host: {}  IP: {}  ZMQ: {}", sys.host.hostname, sys.host.local_ip, sys.host.zmq_port));
    ui.label(format!("Backend: {:?}", sys.backend));
    let (clock_label, clock_color) = match sys.clock_source {
        ClockSource::DrmVblank        => ("Clock: DRM vblank",               egui::Color32::from_rgb(80, 200, 80)),
        ClockSource::VkDisplayControl => ("Clock: VK_EXT_display_control",   egui::Color32::from_rgb(80, 200, 80)),
        ClockSource::PresentWait      => ("Clock: VK_KHR_present_wait",      egui::Color32::from_rgb(80, 200, 80)),
        ClockSource::DisplayTiming    => ("Clock: VK_GOOGLE_display_timing", egui::Color32::YELLOW),
        ClockSource::GpuCompletion    => ("Clock: GPU-completion (inaccurate)", egui::Color32::RED),
    };
    ui.colored_label(clock_color, clock_label);

    // Green only when everything asked for was actually granted — a requested
    // pin or priority that silently didn't take is the failure worth seeing.
    let sched = &sys.host.sched;
    let sched_color = if sched.has_failure() {
        egui::Color32::RED
    } else if sched.is_default() {
        egui::Color32::GRAY
    } else {
        egui::Color32::from_rgb(80, 200, 80)
    };
    ui.colored_label(sched_color, format!("Sched: {}", sched.summary()));

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("CPU:");
        ui.add(egui::ProgressBar::new(metrics.cpu_pct / 100.0).desired_width(80.0));
        ui.label(format!("{:.0}%  (proc {:.0}%)", metrics.cpu_pct, metrics.process_cpu_pct));
    });
    ui.horizontal(|ui| {
        let (used, total) = (metrics.ram_used_mb, metrics.ram_total_mb);
        let frac = if total > 0 { used as f32 / total as f32 } else { 0.0 };
        ui.label("RAM:");
        ui.add(egui::ProgressBar::new(frac).desired_width(80.0));
        ui.label(format!("{} / {} MB  (proc {} MB)", used, total, metrics.process_rss_mb));
    });
    if let Some(gpu_pct) = metrics.gpu_util_pct {
        ui.horizontal(|ui| {
            ui.label("GPU:");
            ui.add(egui::ProgressBar::new(gpu_pct / 100.0).desired_width(80.0));
            let vram_label = match (metrics.gpu_mem_used_mb, metrics.gpu_mem_total_mb) {
                (Some(used), Some(total)) => format!("{:.0}%  VRAM {}/{} MB", gpu_pct, used, total),
                _ => format!("{:.0}%", gpu_pct),
            };
            ui.label(vram_label);
        });
    }

    ui.separator();
    ui.horizontal(|ui| {
        if let Ok(mut sc) = scene.try_write() {
            let pd = sc.photodiode.enabled;
            if ui.button(if pd { "Photodiode: ON" } else { "Photodiode: off" }).clicked() {
                sc.photodiode.enabled = !sc.photodiode.enabled;
                sc.photodiode.flicker = true;
                sc.photodiode.lit = false;
            }
        }
        if let Some(wf) = wireframe {
            if ui.button(if wf { "Wireframe: ON" } else { "Wireframe: off" }).clicked() {
                *wireframe_toggle_requested = true;
            }
        } else {
            ui.add_enabled(false, egui::Button::new("Wireframe: n/a"));
        }
    });
}

pub(in crate::render::overlay_ui) fn frame_timing(
    ui: &mut egui::Ui,
    frame_stats: &mut FrameStats,
    last_phases: FramePhases,
) {
    ui.label(egui::RichText::new("Frame timing").strong());
    let s = frame_stats.summary();
    ui.label(format!("FPS: {:.1}  drops: {}", s.fps, s.drop_count));
    ui.label(format!("frame: {:.2} ms  jitter: ±{:.2} ms", s.mean_ms, s.std_ms));
    ui.label(format!("min: {:.2} ms  max: {:.2} ms", s.min_ms, s.max_ms));
    ui.label(format!(
        "phases µs: tess/upload {:>5}  fence {:>5}  acquire {:>5}  record {:>5}  submit {:>5}",
        last_phases.tessellate_us, last_phases.fence_us, last_phases.acquire_us,
        last_phases.record_us, last_phases.submit_us,
    ));

    let durations: Vec<f64> = frame_stats.durations_recent_ns().map(|d| d as f64 / 1_000_000.0).collect();
    if !durations.is_empty() {
        let expected_ms = frame_stats.expected_ns() as f64 / 1_000_000.0;
        // Guard a 0/invalid refresh rate so the graph still scales.
        let max_ms = durations
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(expected_ms * 2.5)
            .max(f64::MIN_POSITIVE);
        let desired = egui::vec2(ui.available_width(), 64.0);
        let (resp, painter) = ui.allocate_painter(desired, egui::Sense::hover());
        let r = resp.rect;
        painter.rect_filled(r, 0.0, egui::Color32::from_gray(20));
        let n = durations.len();
        let bar_w = r.width() / n as f32;
        for (i, &ms) in durations.iter().enumerate() {
            let frac = (ms / max_ms).min(1.0) as f32;
            let color = if expected_ms > 0.0 && ms > expected_ms * 1.25 {
                egui::Color32::RED
            } else {
                egui::Color32::from_rgb(80, 200, 80)
            };
            let x0 = r.left() + i as f32 * bar_w;
            let x1 = (x0 + bar_w - 1.0).max(x0);
            let y1 = r.bottom();
            let y0 = y1 - frac * r.height();
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)), 0.0, color);
        }
        if expected_ms > 0.0 {
            let exp_y = r.bottom() - (expected_ms / max_ms).min(1.0) as f32 * r.height();
            painter.line_segment(
                [egui::pos2(r.left(), exp_y), egui::pos2(r.right(), exp_y)],
                egui::Stroke::new(1.0_f32, egui::Color32::YELLOW),
            );
        }
    }
}
