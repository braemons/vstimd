//! The per-frame animation engine: advance one animation by a single frame,
//! applying its stimulus effects and start/final actions.
//!
//! These are free functions (not `SceneState` methods) so the borrow checker can
//! see that `animations` and `stimuli` are disjoint fields being borrowed
//! independently. `SceneState::advance_animations` drives them once per frame.

use super::{AnimState, Animation, CancelAction, FinalAction, StartAction};
use crate::scene::SceneState;
use crate::vtl_state::{VtlEdge, VtlBit, VtlEdges, VtlOutputs};
use vtl::VtlKind;

/// Pick the edge set (input vs. output) that a trigger's kind addresses.
fn edges_for<'a>(bit: VtlBit, input: &'a VtlEdges, output: &'a VtlEdges) -> &'a VtlEdges {
    match bit.kind {
        VtlKind::Input => input,
        VtlKind::Output => output,
    }
}

fn edge_fired(input: &VtlEdges, output: &VtlEdges, bit: VtlBit, edge: VtlEdge) -> bool {
    let edges = edges_for(bit, input, output);
    let bank = match edge {
        VtlEdge::Rising => edges.rising[bit.bank],
        VtlEdge::Falling => edges.falling[bit.bank],
    };
    (bank >> bit.bit) & 1 != 0
}

/// Advance a single animation by one frame and apply stimulus effects.
///
/// The animation may vanish between steps only if another thread mutated the
/// scene, which cannot happen while the caller holds the write lock — but every
/// re-fetch still handles a missing handle by returning, so a stale handle is a
/// no-op rather than a panic.
pub(crate) fn advance_one(
    handle: u32,
    scene: &mut SceneState,
    input_edges: &VtlEdges,
    output_edges: &VtlEdges,
    outputs: &mut VtlOutputs<'_>,
) {
    // ── 0. Cancel trigger (Armed or Running) ──────────────────────────────────
    // Evaluated before anything else so a pending (Armed) animation can be
    // cancelled before it ever starts, and a Running one aborts this frame.
    {
        let Some(entry) = scene.config.animations.get(&handle) else {
            return;
        };
        let cancellable = matches!(entry.state, AnimState::Armed | AnimState::Running { .. });
        if cancellable
            && let Some((bit, edge)) = entry.cancel_trigger
            && edge_fired(input_edges, output_edges, bit, edge)
        {
            cancel_one(handle, scene, outputs);
            return;
        }
    }

    // ── 1. Armed → Running ────────────────────────────────────────────────────
    {
        let Some(entry) = scene.config.animations.get(&handle) else {
            return;
        };
        if entry.state == AnimState::Armed {
            let fires = match &entry.start_trigger {
                None => true,
                Some((bit, edge)) => edge_fired(input_edges, output_edges, *bit, *edge),
            };
            if fires {
                // Snapshot user_enabled for RESTORE_VISIBILITY before modifying anything.
                // Either a final-action or cancel-action RESTORE_VISIBILITY needs the capture.
                let captures_state = entry.final_action.contains(FinalAction::RESTORE_VISIBILITY)
                    || entry.cancel_action.contains(CancelAction::RESTORE_VISIBILITY);
                let stim_handles: Vec<u32> = entry.config.target.stimuli().to_vec();
                let start_action = entry.start_action;
                let start_action_trigger_line = entry.start_action_trigger_line;
                // Only meaningful with DONE_LEVEL; None otherwise, so the clear
                // below is a no-op for animations that do not use the level.
                let done_level_line = entry
                    .final_action
                    .contains(FinalAction::DONE_LEVEL)
                    .then_some(entry.final_action_level_line)
                    .flatten();

                if captures_state {
                    let captured: Vec<bool> = stim_handles
                        .iter()
                        .map(|&sh| {
                            scene
                                .config
                                .stimuli
                                .get(&sh)
                                .is_some_and(|e| e.stimulus.flags().enabled)
                        })
                        .collect();
                    if let Some(entry) = scene.config.animations.get_mut(&handle) {
                        entry.captured_user_enabled = Some(captured);
                    }
                }

                // FlashForNFrames enables stimuli at start; FlickerForNFrames sets initial phase_cycles.
                let Some(entry) = scene.config.animations.get(&handle) else {
                    return;
                };
                match &entry.animation {
                    Animation::FlashForNFrames { .. } => {
                        for &sh in &stim_handles {
                            if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                                e.stimulus.flags_mut().enabled = true;
                                e.stimulus.flags_mut().mark_dirty();
                            }
                        }
                    }
                    Animation::FlickerForNFrames { start_on_phase, .. } => {
                        let on = *start_on_phase;
                        for &sh in &stim_handles {
                            if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                                e.stimulus.flags_mut().anim_enabled = on;
                                e.stimulus.flags_mut().mark_dirty();
                            }
                        }
                    }
                    _ => {}
                }

                // Apply start_action bits.
                if start_action.contains(StartAction::ENABLE) {
                    for &sh in &stim_handles {
                        if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                            e.stimulus.flags_mut().enabled = true;
                            e.stimulus.flags_mut().mark_dirty();
                        }
                    }
                }
                if start_action.contains(StartAction::TOGGLE_PHOTODIODE) {
                    scene.photodiode.lit = !scene.photodiode.lit;
                }
                if start_action.contains(StartAction::START_ACTION_TRIGGER_LINE)
                    && let Some(bit) = start_action_trigger_line
                {
                    outputs.pulse(bit);
                }
                // Starting clears the "finished" level from the previous run,
                // so the line answers for this run rather than the last one.
                if let Some(bit) = done_level_line {
                    outputs.clear_level(bit);
                }

                if let Some(entry) = scene.config.animations.get_mut(&handle) {
                    entry.state = AnimState::Running { frame_counter: 0 };
                }
            }
        }
    }

    // ── 2. Advance Running ────────────────────────────────────────────────────
    let (frame_counter, stim_handles) = {
        let Some(entry) = scene.config.animations.get(&handle) else {
            return;
        };
        match entry.state {
            AnimState::Running { frame_counter } => {
                (frame_counter, entry.config.target.stimuli().to_vec())
            }
            _ => return,
        }
    };

    let done: bool = {
        let Some(entry) = scene.config.animations.get(&handle) else {
            return;
        };
        match &entry.animation {
            Animation::CoupleVisibilityToTriggerLine { trigger, polarity } => {
                let edges = edges_for(*trigger, input_edges, output_edges);
                let level = (edges.current[trigger.bank] >> trigger.bit) & 1 != 0;
                let anim_en = polarity.is_asserted(level);
                for &sh in &stim_handles {
                    if let Some(e) = scene.config.stimuli.get_mut(&sh)
                        && e.stimulus.flags().anim_enabled != anim_en
                    {
                        e.stimulus.flags_mut().anim_enabled = anim_en;
                        e.stimulus.flags_mut().mark_dirty();
                    }
                }
                false
            }

            Animation::EnableOnTriggerEdge {
                trigger,
                edge,
                enabled,
            } => {
                let fired = edge_fired(input_edges, output_edges, *trigger, *edge);
                if fired {
                    let en = *enabled;
                    for &sh in &stim_handles {
                        if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                            e.stimulus.flags_mut().enabled = en;
                            e.stimulus.flags_mut().mark_dirty();
                        }
                    }
                }
                fired
            }

            Animation::FlashForNFrames { duration_frames } => frame_counter + 1 >= *duration_frames,

            Animation::FlickerForNFrames {
                on_frames,
                off_frames,
                total_frames,
                start_on_phase,
            } => {
                let period = on_frames + off_frames;
                let phase_frame = frame_counter % period;
                let is_on = if *start_on_phase {
                    phase_frame < *on_frames
                } else {
                    phase_frame >= *off_frames
                };
                for &sh in &stim_handles {
                    if let Some(e) = scene.config.stimuli.get_mut(&sh)
                        && e.stimulus.flags().anim_enabled != is_on
                    {
                        e.stimulus.flags_mut().anim_enabled = is_on;
                        e.stimulus.flags_mut().mark_dirty();
                    }
                }
                total_frames.is_some_and(|tf| frame_counter + 1 >= tf)
            }

            Animation::MoveAlongPath2D { coords_px } => {
                let idx = frame_counter as usize;
                if idx < coords_px.len() {
                    let [x, y] = coords_px[idx];
                    for &sh in &stim_handles {
                        if let Some(e) = scene.config.stimuli.get_mut(&sh)
                            && e.stimulus.move_to_2d(false, x, y).is_err()
                        {
                            // A 2-D path animation over a 3-D stimulus is a
                            // config error; dropping the frame silently would
                            // leave the stimulus frozen mid-animation with no
                            // trace. Warn once per frame, per stimulus.
                            log::warn!(
                                "animation #{handle}: stimulus #{sh} is 3-D; \
                                 MoveAlongPath2D only moves 2-D stimuli"
                            );
                        }
                    }
                }
                frame_counter + 1 >= coords_px.len() as u32
            }
            Animation::MoveAlongSegments2D {
                waypoints_px,
                speed_px_per_sec,
            } => {
                if waypoints_px.len() < 2 || *speed_px_per_sec <= 0.0 {
                    true
                } else {
                    // Compute cumulative lengths along each segment.
                    let seg_lens: Vec<f32> = waypoints_px
                        .windows(2)
                        .map(|w| {
                            let dx = w[1][0] - w[0][0];
                            let dy = w[1][1] - w[0][1];
                            (dx * dx + dy * dy).sqrt()
                        })
                        .collect();
                    let total_len: f32 = seg_lens.iter().sum();
                    // Nominal rate, not the measured one: the measurement drifts,
                    // and this is recomputed every tick, so a jittering divisor
                    // would move the stimulus differently on each run (#120).
                    let total_frames = (total_len / speed_px_per_sec
                        * scene.runtime.nominal_frame_rate_hz)
                        .ceil() as u32;
                    let total_frames = total_frames.max(1);

                    // How far along the path are we at this frame?
                    let t = frame_counter as f32 / (total_frames - 1).max(1) as f32;
                    let dist = t * total_len;

                    // Walk segments to find the current interpolated position.
                    let mut accum = 0.0f32;
                    let mut pos_px = waypoints_px[0];
                    for (i, &seg_len) in seg_lens.iter().enumerate() {
                        if accum + seg_len >= dist || i + 1 == seg_lens.len() {
                            let local_t = if seg_len > 0.0 {
                                (dist - accum) / seg_len
                            } else {
                                0.0
                            };
                            let local_t = local_t.clamp(0.0, 1.0);
                            let a = waypoints_px[i];
                            let b = waypoints_px[i + 1];
                            pos_px = [
                                a[0] + (b[0] - a[0]) * local_t,
                                a[1] + (b[1] - a[1]) * local_t,
                            ];
                            break;
                        }
                        accum += seg_len;
                    }
                    for &sh in &stim_handles {
                        if let Some(e) = scene.config.stimuli.get_mut(&sh)
                            && e.stimulus.move_to_2d(false, pos_px[0], pos_px[1]).is_err()
                        {
                            log::warn!(
                                "animation #{handle}: stimulus #{sh} is 3-D; \
                                 MoveAlongSegments2D only moves 2-D stimuli"
                            );
                        }
                    }
                    frame_counter + 1 >= total_frames
                }
            }
            // TODO(#84): unimplemented. The shm segment is never opened and the stimulus
            // never moves, yet CreateAnimation reports success. Implement the per-frame
            // read (mapping the segment on the ZMQ thread, never here) or reject the
            // command at create time.
            //
            // External position is driven by an external process; never self-terminates.
            Animation::ExternalPosition2D { .. } => false,
        }
    };

    // Increment frame counter.
    if let Some(AnimState::Running { frame_counter }) = scene
        .config
        .animations
        .get_mut(&handle)
        .map(|e| &mut e.state)
    {
        *frame_counter += 1;
    }

    // ── 3. Final actions ──────────────────────────────────────────────────────
    if done {
        let (action, trigger_line, level_line) = {
            let Some(entry) = scene.config.animations.get(&handle) else {
                return;
            };
            (
                entry.final_action,
                entry.final_action_trigger_line,
                entry.final_action_level_line,
            )
        };
        finalize(handle, scene, &stim_handles, outputs, action, trigger_line, level_line, true, true);
    }
}

/// Cancel an animation: distinct from disarm. Applies the animation's
/// `cancel_action` (independent of `final_action`) — leaving visibility in a
/// defined state via `RESTORE_VISIBILITY` / `DISABLE`, pulsing any cancel trigger
/// line, toggling the photodiode — and always ends in `Done` (`RESTART` is not a
/// cancel action). An empty `cancel_action` is a hard abort that leaves state
/// as-is. Works while `Running` (the `anim_enabled` hold is released) or `Armed`
/// (never started: no hold to release, and `RESTORE_VISIBILITY` is a no-op with no
/// capture). `Idle`/`Done` are a no-op. Returns false if the handle is unknown.
pub(crate) fn cancel_one(
    handle: u32,
    scene: &mut SceneState,
    outputs: &mut VtlOutputs<'_>,
) -> bool {
    let Some(entry) = scene.config.animations.get(&handle) else {
        return false;
    };
    match entry.state {
        AnimState::Running { .. } | AnimState::Armed => {
            let running = matches!(entry.state, AnimState::Running { .. });
            let stim_handles = entry.config.target.stimuli().to_vec();
            let action = entry.cancel_action.as_final_action();
            let trigger_line = entry.cancel_action_trigger_line;
            // Cancel has no level of its own; DONE_LEVEL is not a cancel action.
            let level_line = None;
            // Release the anim_enabled hold only if it was actually Running; an
            // Armed animation never grabbed it. RESTART is never honored.
            finalize(
                handle,
                scene,
                &stim_handles,
                outputs,
                action,
                trigger_line,
                level_line,
                false,
                running,
            );
        }
        // Idle (never armed) or already Done: nothing to tear down.
        _ => {}
    }
    true
}

/// Shared teardown for both normal completion and cancel. Applies `action`
/// (a [`FinalAction`] bitset — cancel converts its `CancelAction` via
/// `as_final_action`), pulsing `trigger_line` for the trigger-line bit. When
/// `allow_restart` is false, `RESTART` and `REARM` are both ignored and the
/// animation lands in `Done` — cancel is always terminal. When
/// `release_anim_hold` is false, the `anim_enabled` reset is skipped (used for
/// Armed cancel, which never grabbed the hold).
#[allow(clippy::too_many_arguments)]
fn finalize(
    handle: u32,
    scene: &mut SceneState,
    stim_handles: &[u32],
    outputs: &mut VtlOutputs<'_>,
    final_action: FinalAction,
    trigger_line: Option<VtlBit>,
    level_line: Option<VtlBit>,
    allow_restart: bool,
    release_anim_hold: bool,
) {
    let (captured, restart, rearm) = {
        let Some(entry) = scene.config.animations.get(&handle) else {
            return;
        };
        let cap = entry.captured_user_enabled.clone();
        let restart = allow_restart && final_action.contains(FinalAction::RESTART);
        // RESTART wins: it is the stronger statement (run again now), and with
        // no start_trigger the two are the same thing anyway.
        let rearm = allow_restart && !restart && final_action.contains(FinalAction::REARM);
        (cap, restart, rearm)
    };

    if final_action.contains(FinalAction::RESTORE_VISIBILITY) {
        if let Some(caps) = &captured {
            for (&sh, &was_enabled) in stim_handles.iter().zip(caps.iter()) {
                if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                    e.stimulus.flags_mut().enabled = was_enabled;
                    e.stimulus.flags_mut().mark_dirty();
                }
            }
        }
    } else if final_action.contains(FinalAction::DISABLE) {
        for &sh in stim_handles {
            if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                e.stimulus.flags_mut().enabled = false;
                e.stimulus.flags_mut().mark_dirty();
            }
        }
    }

    // Reset anim_enabled for animations that held it during execution.
    {
        let anim_held = release_anim_hold
            && matches!(
                scene.config.animations.get(&handle).map(|e| &e.animation),
                Some(Animation::FlickerForNFrames { .. })
                    | Some(Animation::CoupleVisibilityToTriggerLine { .. })
            );
        if anim_held {
            for &sh in stim_handles {
                if let Some(e) = scene.config.stimuli.get_mut(&sh)
                    && !e.stimulus.flags().anim_enabled
                {
                    e.stimulus.flags_mut().anim_enabled = true;
                    e.stimulus.flags_mut().mark_dirty();
                }
            }
        }
    }

    if final_action.contains(FinalAction::TOGGLE_PHOTODIODE) {
        scene.photodiode.lit = !scene.photodiode.lit;
    }

    if final_action.contains(FinalAction::FINAL_ACTION_TRIGGER_LINE)
        && let Some(bit) = trigger_line
    {
        outputs.pulse(bit);
    }

    // The level says "finished"; it is cleared when the animation next starts.
    if final_action.contains(FinalAction::DONE_LEVEL)
        && let Some(bit) = level_line
    {
        outputs.set_level(bit);
    }

    if final_action.contains(FinalAction::END_DEFERRED) {
        scene.runtime.pending_flip = true;
        scene.runtime.deferred_mode = false;
    }

    if let Some(entry) = scene.config.animations.get_mut(&handle) {
        if restart {
            entry.state = AnimState::Running { frame_counter: 0 };
            entry.captured_user_enabled = None;
        } else if rearm {
            // Back to waiting: with a start_trigger the animation fires again on
            // the next edge; without one it starts again on the next frame.
            entry.state = AnimState::Armed;
            entry.captured_user_enabled = None;
        } else {
            entry.state = AnimState::Done;
        }
    }
}
