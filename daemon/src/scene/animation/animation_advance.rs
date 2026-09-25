//! The per-frame animation engine: advance one animation by a single frame,
//! applying its stimulus effects and start/final actions.
//!
//! These are free functions (not `SceneState` methods) so the borrow checker can
//! see that `animations` and `stimuli` are disjoint fields being borrowed
//! independently. `SceneState::advance_animations` drives them once per frame.

use super::{AnimState, Animation, CancelAction, FinalAction, StartAction};
use crate::scene::SceneState;
use crate::scene::units::Pos2Px;
use super::animation_input::{self, AxisMap, TransformChannel, Update};
use super::AnimationTarget;
use crate::scene::{Camera3D, Stimulus};
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
                    entry.distance_travelled_cm = 0.0;
                    entry.nav_position_cm = None;
                    entry.track_progress = Default::default();
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
                            && e.stimulus.move_to_2d(false, Pos2Px::new(x, y)).is_err()
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
                            && e.stimulus.move_to_2d(false, Pos2Px(pos_px)).is_err()
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
            // Driven by an external process through an input device; never finishes.
            // A stale device holds its last sample, so the stimulus holds still.
            Animation::ExternalPosition2D { shm_name, x_offset_px, y_offset_px } => {
                if let Some(dev) = animation_input::find_device_opt(&scene.runtime.input, shm_name) {
                    let pos = Pos2Px::new(
                        dev.frame[0].value as f32 + *x_offset_px,
                        dev.frame[1].value as f32 + *y_offset_px,
                    );
                    for &sh in &stim_handles {
                        if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                            let _ = e.stimulus.move_to_2d(false, pos);
                        }
                    }
                }
                false
            }

            Animation::DeviceDrivenTransform { device, axes } => {
                let dt_s = scene.frame_dt_s();
                let camera = matches!(entry.config.target, AnimationTarget::Camera);
                for map in axes {
                    let Some(update) =
                        animation_input::axis_update(&scene.runtime.input, device, map, dt_s)
                    else {
                        continue;
                    };
                    if camera {
                        apply_to_camera(&mut scene.config.camera.live, map, update);
                        apply_to_camera(&mut scene.config.camera.copy, map, update);
                    } else {
                        for &sh in &stim_handles {
                            if let Some(e) = scene.config.stimuli.get_mut(&sh) {
                                apply_to_stimulus(&mut e.stimulus, map, update);
                            }
                        }
                    }
                }
                false
            }

            Animation::LinearNav3D { speed_cm_per_s, wrap_period_cm, source, track } => {
                let step_cm = match source {
                    // Nominal rate, like MoveAlongSegments2D: a scripted run covers
                    // the same distance in the same number of frames every time (#120).
                    None => f64::from(*speed_cm_per_s)
                        / f64::from(scene.runtime.nominal_frame_rate_hz.max(1.0)),
                    // A device's speed is real: integrate over real frame time.
                    Some(src) => {
                        animation_input::nav_step_cm(&scene.runtime.input, src, scene.frame_dt_s())
                    }
                };
                let wrap = *wrap_period_cm;
                let track = *track;
                let nav = entry.nav_position_cm;
                let mut progress = entry.track_progress;
                let (nav, moved_cm) = match track {
                    None => (advance_camera_along_forward(scene, nav, step_cm, wrap), step_cm),
                    Some(track) => advance_on_track(scene, nav, step_cm, track, &mut progress),
                };
                if let Some(entry) = scene.config.animations.get_mut(&handle) {
                    entry.distance_travelled_cm += moved_cm;
                    entry.nav_position_cm = Some(nav);
                    entry.track_progress = progress;
                }
                false
            }
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

/// Move the camera `step_cm` along its horizontal forward direction, wrapping
/// `z` into `[0, wrap_period_cm)` when given. Returns the new integration state.
///
/// The position is integrated in `f64` (`nav`) and only narrowed to write the
/// camera, so the rendered position stays exact however far the camera goes.
/// If the camera is not where this animation last left it — a `SetCamera`, say —
/// integration restarts from where it now is.
///
/// The position is written to both the live and the staged camera: an animation
/// is not a staged command, and a deferred block that flipped the stale staged
/// position back in would jump the camera backwards. Every other camera field in
/// the staged copy is left as the client staged it.
fn advance_camera_along_forward(
    scene: &mut SceneState,
    nav: Option<super::NavPosition>,
    step_cm: f64,
    wrap_period_cm: Option<f32>,
) -> super::NavPosition {
    let camera = &mut scene.config.camera;
    let current = camera.live.position_cm.0;
    let (x, z) = match nav {
        Some(n) if n.written == current => (n.x, n.z),
        _ => (f64::from(current.x), f64::from(current.z)),
    };
    let yaw = f64::from(camera.live.yaw_deg).to_radians();
    // Forward is -Z at yaw 0; positive yaw turns left, towards -X.
    let x = x - yaw.sin() * step_cm;
    let mut z = z - yaw.cos() * step_cm;
    if let Some(period) = wrap_period_cm.filter(|p| *p > 0.0) {
        z = z.rem_euclid(f64::from(period));
    }
    let written = glam::Vec3::new(x as f32, current.y, z as f32);
    camera.live.position_cm = crate::scene::units::Pos3Cm(written);
    camera.copy.position_cm = crate::scene::units::Pos3Cm(written);
    super::NavPosition { x, z, written }
}

/// One frame of a finite track: move, fade out at the end, jump back to the
/// start, fade in. Returns the integration state and how far the camera moved.
///
/// The end is measured along the heading the camera started with, so strafing
/// or turning never ends the track early. The jump restores the start's yaw as
/// well as its position: the next lap begins facing down the track again.
fn advance_on_track(
    scene: &mut SceneState,
    nav: Option<super::NavPosition>,
    step_cm: f64,
    track: super::Track3D,
    progress: &mut super::TrackProgress,
) -> (super::NavPosition, f64) {
    use super::TrackPhase;
    let start = *progress.start.get_or_insert_with(|| {
        let c = &scene.config.camera.live;
        super::animation_entry::TrackStart {
            x: f64::from(c.position_cm.0.x),
            z: f64::from(c.position_cm.0.z),
            yaw_deg: c.yaw_deg,
        }
    });
    let fade_frames = track.fade_frames;

    if let TrackPhase::FadingOut(left) = progress.phase {
        if left > 1 {
            progress.phase = TrackPhase::FadingOut(left - 1);
            return (hold_camera(scene, nav), 0.0);
        }
        let nav = jump_to_track_start(scene, start);
        progress.laps += 1;
        progress.phase = TrackPhase::FadingIn(fade_frames);
        return (nav, 0.0);
    }

    let nav = advance_camera_along_forward(scene, nav, step_cm, None);
    if let TrackPhase::FadingIn(left) = progress.phase {
        progress.phase = match left {
            0 | 1 => TrackPhase::Moving,
            _ => TrackPhase::FadingIn(left - 1),
        };
    }
    let yaw = f64::from(start.yaw_deg).to_radians();
    // Forward is -Z at yaw 0; positive yaw turns left, towards -X.
    let along_cm = -(nav.x - start.x) * yaw.sin() - (nav.z - start.z) * yaw.cos();
    if along_cm >= f64::from(track.length_cm) && progress.phase == TrackPhase::Moving {
        if fade_frames == 0 {
            progress.laps += 1;
            return (jump_to_track_start(scene, start), step_cm);
        }
        progress.phase = TrackPhase::FadingOut(fade_frames);
    }
    (nav, step_cm)
}

/// The integration state for a frame the camera does not move.
fn hold_camera(scene: &SceneState, nav: Option<super::NavPosition>) -> super::NavPosition {
    let current = scene.config.camera.live.position_cm.0;
    match nav {
        Some(n) if n.written == current => n,
        _ => super::NavPosition {
            x: f64::from(current.x),
            z: f64::from(current.z),
            written: current,
        },
    }
}

/// Put the camera back at a track's start, in both the live and staged camera
/// (see [`advance_camera_along_forward`]).
fn jump_to_track_start(
    scene: &mut SceneState,
    start: super::animation_entry::TrackStart,
) -> super::NavPosition {
    let camera = &mut scene.config.camera;
    let y = camera.live.position_cm.0.y;
    let written = glam::Vec3::new(start.x as f32, y, start.z as f32);
    for cam in [&mut camera.live, &mut camera.copy] {
        cam.position_cm = crate::scene::units::Pos3Cm(written);
        cam.yaw_deg = start.yaw_deg;
    }
    super::NavPosition { x: start.x, z: start.z, written }
}

/// Apply one device-driven update to a camera.
fn apply_to_camera(cam: &mut Camera3D, map: &AxisMap, update: Update) {
    use animation_input::apply;
    let p = &mut cam.position_cm.0;
    match map.channel {
        TransformChannel::PosX => p.x = apply(p.x, update, map),
        TransformChannel::PosY => p.y = apply(p.y, update, map),
        TransformChannel::PosZ => p.z = apply(p.z, update, map),
        TransformChannel::Yaw => cam.yaw_deg = apply(cam.yaw_deg, update, map),
        TransformChannel::Pitch => cam.pitch_deg = apply(cam.pitch_deg, update, map),
        TransformChannel::Roll => cam.roll_deg = apply(cam.roll_deg, update, map),
        TransformChannel::Forward | TransformChannel::Strafe => {
            // Validation admits only rate and cumulative axes here, so this is
            // always a movement.
            let Update::Add(d) = update else { return };
            let yaw = cam.yaw_deg.to_radians();
            // Forward is -Z at yaw 0; right is +X. Positive yaw turns left.
            let (dx, dz) = match map.channel {
                TransformChannel::Forward => (-yaw.sin(), -yaw.cos()),
                _ => (yaw.cos(), -yaw.sin()),
            };
            p.x += dx * d as f32;
            p.z += dz * d as f32;
            if let Some(w) = map.wrap {
                p.z = p.z.rem_euclid(w);
            }
        }
        TransformChannel::ScaleX
        | TransformChannel::ScaleY
        | TransformChannel::ScaleZ
        | TransformChannel::ScaleUniform => {}
    }
}

/// Apply one device-driven update to a stimulus: position and rotation for a
/// 2-D stimulus (in pixels and degrees), position, rotation and scale for a 3-D
/// one. Channels the stimulus does not have are ignored.
fn apply_to_stimulus(stim: &mut Stimulus, map: &AxisMap, update: Update) {
    use animation_input::apply;
    if let Some(transform) = stim.transform3d_mut() {
        let t = &mut transform.live;
        let (p, r, s) = (&mut t.position_cm.0, &mut t.rotation_deg, &mut t.scale);
        match map.channel {
            TransformChannel::PosX => p.x = apply(p.x, update, map),
            TransformChannel::PosY => p.y = apply(p.y, update, map),
            TransformChannel::PosZ => p.z = apply(p.z, update, map),
            TransformChannel::Yaw => r.x = apply(r.x, update, map),
            TransformChannel::Pitch => r.y = apply(r.y, update, map),
            TransformChannel::Roll => r.z = apply(r.z, update, map),
            TransformChannel::ScaleX => s.x = apply(s.x, update, map),
            TransformChannel::ScaleY => s.y = apply(s.y, update, map),
            TransformChannel::ScaleZ => s.z = apply(s.z, update, map),
            TransformChannel::ScaleUniform => {
                let v = apply(s.x, update, map);
                *s = glam::Vec3::splat(v);
            }
            TransformChannel::Forward | TransformChannel::Strafe => {}
        }
        return;
    }
    if !animation_input::applies_to_2d(map.channel) {
        return;
    }
    let Some(t) = stim.transform2d().map(|t| t.live) else { return };
    let _ = match map.channel {
        TransformChannel::PosX => stim.move_to_2d(
            false,
            Pos2Px::new(apply(t.pos_px.x(), update, map), t.pos_px.y()),
        ),
        TransformChannel::PosY => stim.move_to_2d(
            false,
            Pos2Px::new(t.pos_px.x(), apply(t.pos_px.y(), update, map)),
        ),
        _ => stim.set_angle_2d(false, apply(t.angle_deg, update, map)),
    };
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

    // Same rule as `SceneState::end_deferred`: with nothing staged there is
    // nothing to flip, and flipping stale copies would revert the scene.
    if final_action.contains(FinalAction::END_DEFERRED) && scene.runtime.deferred_mode {
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
