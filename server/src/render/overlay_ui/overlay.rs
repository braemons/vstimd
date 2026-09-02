//! Overlay chrome: lays out one `Panel::left` per visible group inside a
//! shared top panel, draws each group's title bar, and dispatches the body to
//! the matching module in [`super::panels`]. The dialogs (stimulus, animation,
//! file browser) are driven from here too, after the panels are drawn.

use std::sync::{Arc, Mutex, RwLock};

use super::animation_dialog::TriggerLine;
use super::file_browser::BrowserMode;
use super::overlay_state::{OverlayGroup, OverlayState};
use super::panels::animations_panel::animations_panel;
use super::panels::benchmarks_panel::benchmarks_panel;
use super::panels::scene_config_panel::scene_config_panel;
use super::panels::log_panel::log_panel;
use super::panels::stimuli_panel::stimuli_panel;
use super::panels::system_panel::{frame_timing, system_panel};
use super::panels::vtl_panel::vtl_panel;
use crate::scene_config_file::{load_config, save_config};
use crate::log_buffer::LogBuffer;
use crate::scene::{LoadMode, SceneState};
use crate::system_metrics::SystemMetrics;
use crate::timing::{FramePhases, FrameStats};
use crate::vtl_state::{VtlConfig, VtlState};

use crate::render::StimulusDisplayInfo;
use crate::system_info::SystemInfo;

const FOCUS_STROKE: egui::Color32 = egui::Color32::from_rgb(90, 160, 255);

/// One dark background color per panel slot (indices match `OverlayGroup::index()`).
const PANEL_COLORS: [egui::Color32; 12] = [
    egui::Color32::from_rgb(25, 28, 65),  // 0 Stimuli    — indigo
    egui::Color32::from_rgb(15, 50, 50),  // 1 Log        — teal
    egui::Color32::from_rgb(15, 55, 20),  // 2 VTL        — forest green
    egui::Color32::from_rgb(60, 35, 12),  // 3 Animations — amber
    egui::Color32::from_rgb(12, 30, 62),  // 4 System     — navy
    egui::Color32::from_rgb(55, 20, 50),  // 5 Scene-config — magenta
    egui::Color32::from_rgb(50, 50, 12),  // 6 Benchmarks — olive
    egui::Color32::from_rgb(55, 18, 18),  // 7            — crimson
    egui::Color32::from_rgb(15, 42, 35),  // 8            — sea green
    egui::Color32::from_rgb(35, 15, 58),  // 9            — violet
    egui::Color32::from_rgb(58, 35, 15),  // 10           — sienna
    egui::Color32::from_rgb(25, 45, 15),  // 11           — moss
];

fn group_frame(group: OverlayGroup, style: &egui::Style) -> egui::Frame {
    // Use the standard side-panel frame so inner_margin is sized correctly,
    // then override only the fill colour. No custom stroke — the Panel's own
    // separator line provides the visual division between panels.
    egui::Frame::side_top_panel(style)
        .fill(PANEL_COLORS[group.index() % PANEL_COLORS.len()])
}

pub struct OverlayArgs<'a> {
    pub scene: &'a Arc<RwLock<SceneState>>,
    pub vtl: Option<&'a Mutex<VtlState>>,
    pub frame_stats: &'a mut FrameStats,
    pub last_phases: FramePhases,
    pub sys: &'a SystemInfo,
    pub display: &'a StimulusDisplayInfo,
    pub wireframe: Option<bool>,
    pub metrics: &'a SystemMetrics,
    pub log_buffer: &'a LogBuffer,
    pub overlay: &'a mut OverlayState,
}

/// Render the title bar and content of one group inside a `Panel::left` that
/// the caller already opened. Paints a focus-accent border when `is_focused`.
fn group_panel_header(
    ui: &mut egui::Ui,
    group: OverlayGroup,
    is_focused: bool,
    want_focus: bool,
    closed: &mut bool,
    add: impl FnOnce(&mut egui::Ui, bool),
) {
    if is_focused {
        // clip_rect is the physical panel rect (outside the content inner_margin),
        // so the border sits in the margin gap rather than on top of content.
        ui.painter().rect_stroke(
            ui.clip_rect(),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(2.0_f32, FOCUS_STROKE),
            egui::StrokeKind::Inside,
        );
    }
    ui.horizontal(|ui| {
        if is_focused {
            ui.label(egui::RichText::new("▶").color(FOCUS_STROKE));
        }
        ui.label(
            egui::RichText::new(format!("{} [{}]", group.title(), group.fkey_label()))
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("x").clicked() {
                *closed = true;
            }
        });
    });
    ui.separator();
    add(ui, want_focus);
}

pub fn build_overlay_ui(ctx: &egui::Context, args: &mut OverlayArgs<'_>) {
    let OverlayArgs {
        scene, vtl, frame_stats, last_phases, sys, display, wireframe, metrics, log_buffer, overlay,
    } = args;
    let last_phases = *last_phases;

    overlay.benchmark.tick(scene, frame_stats);

    let focused = overlay.focused;
    let focus_now = overlay.pending_focus;
    overlay.pending_focus = false;

    let OverlayState {
        master_visible,
        visible,
        focused: _,
        pending_focus: _,
        wireframe_toggle_requested,
        file_browser,
        benchmark,
        stimulus_dialog,
        animation_dialog,
    } = &mut **overlay;

    let want = |g: OverlayGroup| focus_now && focused == g;
    let foc  = |g: OverlayGroup| focused == g;

    // Quick-load requested from the inline scene-config list; applied after the panels
    // are drawn (like the file-browser result) so we never write the scene mid-draw.
    let mut quick_load: Option<(BrowserMode, std::path::PathBuf)> = None;

    // ── Top panel — each visible group is a Panel::left inside ───────────────
    // Panel::left fills the full height of Panel::top, so no circular height
    // dependency. The top panel auto-sizes from the tallest left panel.
    const GROUP_W: f32 = 310.0;
    let style = ctx.global_style();
    #[allow(deprecated)]
    egui::Panel::top("overlay_panel")
        .resizable(true)
        .default_size(360.0)
        .show(ctx, |ui| {

        // ── Stimuli ───────────────────────────────────────────────────────────
        if visible[OverlayGroup::Stimuli.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_stimuli").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::Stimuli, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::Stimuli,
                    foc(OverlayGroup::Stimuli), want(OverlayGroup::Stimuli), &mut closed,
                    |ui, want_focus| {
                    stimuli_panel(ui, want_focus, scene, stimulus_dialog);
                });
            });
            if closed { visible[OverlayGroup::Stimuli.index()] = false; }
        }

        // ── Log ───────────────────────────────────────────────────────────────
        if visible[OverlayGroup::Log.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_log").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::Log, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::Log,
                    foc(OverlayGroup::Log), want(OverlayGroup::Log), &mut closed,
                    |ui, _| {
                    log_panel(ui, scene, log_buffer);
                });
            });
            if closed { visible[OverlayGroup::Log.index()] = false; }
        }

        // ── VTL ───────────────────────────────────────────────────────────────
        if visible[OverlayGroup::Vtl.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_vtl").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::Vtl, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::Vtl,
                    foc(OverlayGroup::Vtl), want(OverlayGroup::Vtl), &mut closed,
                    |ui, want_focus| {
                    vtl_panel(ctx, ui, want_focus, *vtl);
                });
            });
            if closed { visible[OverlayGroup::Vtl.index()] = false; }
        }

        // ── Animations ────────────────────────────────────────────────────────
        if visible[OverlayGroup::Animations.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_anim").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::Animations, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::Animations,
                    foc(OverlayGroup::Animations), want(OverlayGroup::Animations), &mut closed,
                    |ui, want_focus| {
                    animations_panel(ui, want_focus, scene, *vtl, animation_dialog);
                });
            });
            if closed { visible[OverlayGroup::Animations.index()] = false; }
        }

        // ── System ────────────────────────────────────────────────────────────
        if visible[OverlayGroup::System.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_system").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::System, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::System,
                    foc(OverlayGroup::System), want(OverlayGroup::System), &mut closed,
                    |ui, _| {
                    system_panel(ui, sys, display, *wireframe, metrics, scene,
                        wireframe_toggle_requested);
                    ui.separator();
                    frame_timing(ui, frame_stats, last_phases);
                });
            });
            if closed { visible[OverlayGroup::System.index()] = false; }
        }

        // ── Scene-config ──────────────────────────────────────────────────────
        if visible[OverlayGroup::SceneConfig.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_scene_config").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::SceneConfig, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::SceneConfig,
                    foc(OverlayGroup::SceneConfig), want(OverlayGroup::SceneConfig), &mut closed,
                    |ui, want_focus| {
                    quick_load = scene_config_panel(ui, want_focus, scene, file_browser);
                });
            });
            if closed { visible[OverlayGroup::SceneConfig.index()] = false; }
        }

        // ── Benchmarks ────────────────────────────────────────────────────────
        if visible[OverlayGroup::Benchmarks.index()] {
            let mut closed = false;
            egui::Panel::left("ovl_bench").resizable(false).default_size(GROUP_W)
                .frame(group_frame(OverlayGroup::Benchmarks, &style))
                .show_inside(ui, |ui| {
                group_panel_header(ui, OverlayGroup::Benchmarks,
                    foc(OverlayGroup::Benchmarks), want(OverlayGroup::Benchmarks), &mut closed,
                    |ui, want_focus| {
                    benchmarks_panel(ui, want_focus, benchmark, scene, frame_stats, display);
                });
            });
            if closed { visible[OverlayGroup::Benchmarks.index()] = false; }
        }

        // Central panel consumes remaining space so egui doesn't complain about
        // unoccupied area inside the top panel.
        egui::CentralPanel::default().show_inside(ui, |_| {});
    }); // Panel::top

    // Hide master when all groups were closed via x button.
    if !visible.iter().any(|&v| v) {
        *master_visible = false;
    }

    // ── Dialogs (modal floating windows) ────────────────────────────────────────
    stimulus_dialog.show(ctx);
    if let Some(entry) = stimulus_dialog.take_result() {
        scene.write().unwrap().add_stimulus(entry);
    }

    let (stim_list, trigger_lines) = collect_dialog_inputs(scene, *vtl);
    animation_dialog.show(ctx, &stim_list, &trigger_lines);
    if let Some(entry) = animation_dialog.take_result() {
        scene.write().unwrap().add_animation(entry);
    }

    file_browser.show(ctx);
    if let Some((mode, path)) = file_browser.take_result() {
        handle_file_result(mode, path, scene, *vtl);
    }
    if let Some((mode, path)) = quick_load {
        handle_file_result(mode, path, scene, *vtl);
    }
}

/// Gather the stimulus list and named VTL trigger lines the animation dialog
/// offers as choices.
fn collect_dialog_inputs(
    scene: &Arc<RwLock<SceneState>>,
    vtl: Option<&Mutex<VtlState>>,
) -> (Vec<(u32, String)>, Vec<TriggerLine>) {
    let stim_list: Vec<(u32, String)> = scene
        .try_read()
        .map(|sc| {
            sc.stimuli.iter().map(|(&h, e)| {
                let label = e
                    .identity
                    .name
                    .clone()
                    .unwrap_or_else(|| e.stimulus.type_name().to_string());
                (h, label)
            }).collect()
        })
        .unwrap_or_default();

    let trigger_lines: Vec<TriggerLine> = vtl
        .and_then(|v| v.try_lock().ok())
        .map(|vst| {
            vst.names.iter().map(|e| TriggerLine {
                label: format!("{} ({}/{}, {:?})", e.name, e.bank, e.bit, e.kind),
                bit: crate::scene::VtlBit {
                    bank: e.bank as usize,
                    bit: e.bit,
                    kind: e.kind,
                },
            }).collect()
        })
        .unwrap_or_default();

    (stim_list, trigger_lines)
}

fn handle_file_result(
    mode: BrowserMode,
    path: std::path::PathBuf,
    scene: &Arc<RwLock<SceneState>>,
    vtl: Option<&Mutex<VtlState>>,
) {
    match mode {
        BrowserMode::Save => {
            let scene_guard = scene.read().unwrap();
            let default_vtl = VtlConfig::default();
            let vtl_guard = vtl.and_then(|v| v.try_lock().ok());
            let vtl_cfg = vtl_guard.as_ref().map(|v| &v.config).unwrap_or(&default_vtl);
            if let Err(e) = save_config(&scene_guard.config, vtl_cfg, &path) {
                log::error!("Scene-config save failed: {e}");
            } else {
                log::info!("Scene-config saved to {:?}", path);
            }
        }
        BrowserMode::OpenReplace | BrowserMode::OpenAdditive => {
            let load_mode = if matches!(mode, BrowserMode::OpenReplace) {
                LoadMode::Replace
            } else {
                LoadMode::Additive
            };
            match load_config(&path) {
                Ok((scene_cfg, sections)) => {
                    if let Some(v) = vtl
                        && let Ok(mut v) = v.lock() {
                            v.config.names = sections.vtl.names;
                            v.sync_names_to_shm();
                        }
                    scene.write().unwrap().load_snapshot(scene_cfg, load_mode);
                    log::info!("Config loaded from {:?}", path);
                }
                Err(e) => log::error!("Config load failed: {e}"),
            }
        }
    }
}
