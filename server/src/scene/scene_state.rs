use super::animation::{AnimState, AnimationEntry};
use super::conditions::{Condition, ConditionAction, active_in};
use super::scene_config::{LoadMode, SceneConfig};
use super::stimulus::StimulusSceneEntry;
use crate::scene_config_file::{
    ARCHIVE_WARN_THRESHOLD, DEFAULT_PROJECT, LAST_SESSION_CONFIG, SESSION_PROJECT, SceneConfigRef,
    archive_timestamp_name, count_archive_configs, load_config, save_config, scene_config_path,
};
use crate::vtl_state::{VtlConfig, VtlState};
extern crate vtl;

// ── Command log (overlay feature only) ────────────────────────────────────────

/// One recorded ZMQ command, held in a capped ring buffer inside `SceneState`.
/// Written by the ZMQ thread (under the existing write lock) and read by
/// reference from the render thread (under the read lock) — no extra locking.
pub struct CommandEntry {
    /// Milliseconds since server start.
    pub elapsed_ms: f64,
    pub handle: u32,
    /// Short human-readable command name + key params, e.g. "CreateRect 100×50".
    pub summary: String,
    pub ok: bool,
    pub response: i32,
}

// ── Non-serializable runtime state ────────────────────────────────────────────

pub struct SceneRuntimeState {
    /// Root of the on-device storage tree — projects, and the scene-configs and
    /// assets inside them. Set from `--storage-dir`.
    pub storage_dir: std::path::PathBuf,
    /// True while commands should write into copy fields instead of live fields.
    pub deferred_mode: bool,
    /// Set by `DeferredMode{start:false}`; cleared by the render thread after flip.
    pub pending_flip: bool,
    /// Rolling mean of measured frame durations, updated by the render thread
    /// each frame. Telemetry only — it jitters, so nothing whose result has to
    /// be reproducible may compute with it (#120).
    pub frame_rate_hz: f32,
    /// Nominal refresh rate of the display mode, set once by the render loop.
    /// The rate animations convert against: a fixed property of the rig, so a
    /// config plays back the same way on every run.
    pub nominal_frame_rate_hz: f32,
    /// Set by the render thread on each frame. `None` until the first frame completes.
    pub screen_size: Option<(u32, u32)>,
    /// Screen size at which meshes were last tessellated. When this changes all
    /// stimuli are re-uploaded (NDC coordinates depend on screen dimensions).
    pub last_uploaded_size: (u32, u32),
    pub error_mask: u16,
    pub error_code: i16,
    /// Command ring buffer — written by ZMQ thread, read by overlay.
    pub command_log: std::collections::VecDeque<CommandEntry>,
    pub command_log_total: u64,
    pub command_log_errors: u64,
    pub server_start: std::time::Instant,
    /// Incremented by the render thread (or null loop) once per rendered frame.
    pub frame_count: u64,
    /// Notifies the ZMQ thread whenever `frame_count` advances.
    pub frame_notifier: std::sync::Arc<tokio::sync::watch::Sender<u64>>,
    /// Reusable buffer for the per-frame animation-handle snapshot in
    /// [`SceneState::advance_animations`]. Kept here so its allocation is reused
    /// across frames instead of being reallocated each tick.
    anim_scratch: Vec<u32>,
}

impl SceneRuntimeState {
    pub fn new_with_storage_dir(storage_dir: std::path::PathBuf) -> Self {
        let (tx, _rx) = tokio::sync::watch::channel(0u64);
        Self {
            storage_dir,
            deferred_mode: false,
            pending_flip: false,
            frame_rate_hz: 60.0,
            nominal_frame_rate_hz: 60.0,
            screen_size: None,
            last_uploaded_size: (0, 0),
            error_mask: 0,
            error_code: 0,
            command_log: std::collections::VecDeque::new(),
            command_log_total: 0,
            command_log_errors: 0,
            server_start: std::time::Instant::now(),
            frame_count: 0,
            frame_notifier: std::sync::Arc::new(tx),
            anim_scratch: Vec::new(),
        }
    }

    fn new() -> Self {
        Self::new_with_storage_dir(std::path::PathBuf::from("."))
    }
}

// ── SceneState ────────────────────────────────────────────────────────────────

/// All shared scene state. Wrapped in `Arc<RwLock<SceneState>>` and shared
/// between the render thread (read lock) and the ZMQ server thread (write lock).
///
/// # Thread-safety contract
///
/// `SceneState` itself does not contain any synchronisation primitives; all
/// locking is done by the caller via the outer `RwLock`.  Two threads access
/// the state concurrently:
///
/// | Thread | Lock | Duration |
/// |---|---|---|
/// | **ZMQ server** (`ipc.rs`) | **write** | One decoded request at a time |
/// | **Render** (`render/state.rs`) | **write** then **read** | One frame at a time |
///
/// The ZMQ thread holds the write lock only while dispatching a single
/// `handle_request()` call, so it releases it before the next ZMQ recv.
/// The render thread takes a write lock briefly in `update()` for
/// `apply_flip()` and scene bookkeeping, then drops it before drawing.
pub struct SceneState {
    pub config: SceneConfig,
    pub runtime: SceneRuntimeState,
}

impl std::ops::Deref for SceneState {
    type Target = SceneConfig;
    fn deref(&self) -> &SceneConfig {
        &self.config
    }
}

impl std::ops::DerefMut for SceneState {
    fn deref_mut(&mut self) -> &mut SceneConfig {
        &mut self.config
    }
}

impl SceneState {
    pub fn new() -> Self {
        Self {
            config: SceneConfig::default(),
            runtime: SceneRuntimeState::new(),
        }
    }

    pub fn new_with_storage_dir(storage_dir: std::path::PathBuf) -> Self {
        Self {
            config: SceneConfig::default(),
            runtime: SceneRuntimeState::new_with_storage_dir(storage_dir),
        }
    }

    // ── Handle allocation ─────────────────────────────────────────────────────

    pub fn alloc_stim_handle(&mut self) -> u32 {
        let h = self.next_stim_handle;
        self.next_stim_handle += 1;
        h
    }

    pub fn alloc_anim_handle(&mut self) -> u32 {
        let h = self.next_anim_handle;
        self.next_anim_handle += 1;
        h
    }

    /// Insert a `StimulusEntry` and return the allocated handle.
    /// The internal insertion path used by both `cmd_create_*` and tests.
    pub fn add_stimulus(&mut self, entry: super::stimulus::StimulusSceneEntry) -> u32 {
        let h = self.alloc_stim_handle();
        self.stimuli.insert(h, entry);
        h
    }

    /// Insert an `AnimationEntry` and return the allocated handle.
    /// The internal insertion path used by both `cmd_create_animation` and tests.
    pub fn add_animation(&mut self, entry: AnimationEntry) -> u32 {
        let h = self.alloc_anim_handle();
        self.animations.insert(h, entry);
        h
    }

    /// Arm an animation. Returns false if the handle is unknown. Shared by
    /// `cmd_arm_animation` and the overlay UI.
    pub fn arm_animation(&mut self, handle: u32) -> bool {
        match self.config.animations.get_mut(&handle) {
            Some(entry) => {
                entry.state = AnimState::Armed;
                true
            }
            None => false,
        }
    }

    /// Disarm an animation back to Idle, releasing any flicker `anim_enabled`
    /// hold it placed on its stimuli. Returns false if the handle is unknown.
    /// Shared by `cmd_disarm_animation` and the overlay UI.
    pub fn disarm_animation(&mut self, handle: u32) -> bool {
        let entry = match self.config.animations.get_mut(&handle) {
            Some(e) => e,
            None => return false,
        };
        let was_running = matches!(entry.state, AnimState::Running { .. });
        let stim_handles = entry.target.stimuli().to_vec();
        entry.state = AnimState::Idle;
        if was_running {
            self.release_anim_hold(&stim_handles);
        }
        true
    }

    /// Cancel an animation with a clean teardown, distinct from `disarm` — see
    /// [`super::animation::cancel_one`]. A `Running` animation applies its
    /// configured `cancel_action` (which may be empty for a hard abort) and
    /// ends in `Done`; an `Armed` one is stopped before it starts. For
    /// `CANCEL_ACTION_TRIGGER_LINE`, `outputs.pulses` receives the pulse on
    /// `cancel_action_trigger_line`.
    ///
    /// Callers outside the render loop seed `levels` from `VtlState::staged` and
    /// start `pulses` **empty**, then OR whatever was produced into
    /// `VtlState::pulses`. Seeding pulses from the live buffer instead would
    /// re-publish marks that are already on their way out, stretching a
    /// one-frame mark across two frames. Returns false if the handle is
    /// unknown.
    /// Shared by `cmd_cancel_animation` and the overlay UI.
    pub fn cancel_animation(
        &mut self,
        handle: u32,
        outputs: &mut crate::vtl_state::VtlOutputs<'_>,
    ) -> bool {
        super::animation::cancel_one(handle, self, outputs)
    }

    /// Remove an animation, releasing any flicker hold if it was running.
    /// Returns false if the handle is unknown. Shared by `cmd_delete_animation`
    /// and the overlay UI.
    pub fn delete_animation(&mut self, handle: u32) -> bool {
        let entry = match self.config.animations.shift_remove(&handle) {
            Some(e) => e,
            None => return false,
        };
        if matches!(entry.state, AnimState::Running { .. }) {
            self.release_anim_hold(&entry.config.target.stimuli().to_vec());
        }
        true
    }

    /// Release the `anim_enabled` hold a running animation placed on `stimuli`.
    /// Setting true when already true is a no-op, so this is safe unconditionally.
    fn release_anim_hold(&mut self, stimuli: &[u32]) {
        for &sh in stimuli {
            if let Some(se) = self.config.stimuli.get_mut(&sh) {
                se.stimulus.flags_mut().anim_enabled = true;
                se.stimulus.flags_mut().mark_dirty();
            }
        }
    }

    /// Advance all animations by one frame.  Called once per frame by the render
    /// thread at [S] (after output commit and input poll).
    ///
    /// `input_edges`    — rising/falling/current input lines from `VtlState::poll()`
    /// `output_edges`   — rising/falling/current output lines from `VtlState::output_edges()`,
    ///                    used to start/cancel/couple animations off output-line edges
    /// `outputs`        — the two output channels: `levels` (`VtlState::staged`, held until
    ///                    cleared) and `pulses` (one frame, then LOW). Both are passed by
    ///                    value and written back after all animations have run.
    pub fn advance_animations(
        &mut self,
        input_edges: &crate::vtl_state::VtlEdges,
        output_edges: &crate::vtl_state::VtlEdges,
        outputs: &mut crate::vtl_state::VtlOutputs<'_>,
    ) {
        // Snapshot the handles into a reused buffer: `advance_one` borrows the
        // whole `SceneState` mutably, so we can't iterate `self.animations`
        // directly. Taking the scratch Vec out lets us hand `self` to the callee.
        let mut handles = std::mem::take(&mut self.runtime.anim_scratch);
        handles.clear();
        // Animations outside the active condition do not advance: they observe
        // no trigger and hold nothing. They are filtered here rather than
        // inside `advance_one` so the skip is stated once, next to the reason.
        handles.extend(
            self.animations
                .iter()
                .filter(|(_, e)| e.cond_enabled)
                .map(|(&h, _)| h),
        );
        for &handle in &handles {
            super::animation::advance_one(
                handle,
                self,
                input_edges,
                output_edges,
                outputs,
            );
        }
        self.runtime.anim_scratch = handles;
    }

    // ── Conditions ────────────────────────────────────────────────────────────
    //
    // The active condition gates stimuli and animations through a derived flag
    // on each (`StimulusFlags::cond_enabled`, `AnimationEntry::cond_enabled`),
    // recomputed here whenever the active condition or a membership changes.
    // The type itself, and the "empty list means every condition" rule, live in
    // [`super::conditions`].

    /// Make `index` the active condition and re-derive every gate. A hard cut:
    /// the next frame is drawn from the new condition, with no cross-fade.
    ///
    /// Any index is accepted — declaring a condition is what gives it a *name*,
    /// not what brings it into existence, so a protocol that just counts
    /// upwards needs no declarations at all.
    pub fn set_condition(&mut self, index: u32) {
        self.config.conditions.active = index;
        self.apply_conditions();
    }

    /// Make the condition declared under `name` active. `Err` names the miss:
    /// an unknown name cannot be interpreted as an index, and guessing one
    /// would blank the screen for a typo.
    pub fn set_condition_by_name(&mut self, name: &str) -> Result<u32, String> {
        match self.config.conditions.index_of_name(name) {
            Some(index) => {
                self.set_condition(index);
                Ok(index)
            }
            None => Err(format!("no condition named {name:?}")),
        }
    }

    /// Replace the declared condition set. Does not change which condition is
    /// active, but does re-derive the gates: a rename never moves the active
    /// index, while re-declaring can change what a *name* resolves to later.
    pub fn declare_conditions(&mut self, declared: Vec<Condition>) -> Result<(), String> {
        self.config.conditions.declare(declared)?;
        self.apply_conditions();
        Ok(())
    }

    /// Set the conditions a stimulus is active in. Empty = every condition.
    /// Returns false if the handle is unknown.
    pub fn set_stimulus_conditions(&mut self, handle: u32, conditions: Vec<u32>) -> bool {
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                entry.conditions = conditions;
                self.apply_conditions();
                true
            }
            None => false,
        }
    }

    /// Set the conditions an animation is active in, and what a switch does to
    /// it. Empty = every condition. Returns false if the handle is unknown.
    pub fn set_animation_conditions(
        &mut self,
        handle: u32,
        conditions: Vec<u32>,
        action: ConditionAction,
    ) -> bool {
        match self.config.animations.get_mut(&handle) {
            Some(entry) => {
                entry.config.conditions = conditions;
                entry.config.condition_action = action;
                self.apply_conditions();
                true
            }
            None => false,
        }
    }

    /// Re-derive every condition gate from the memberships and the active index.
    ///
    /// Called after anything that can change the answer — a switch, a
    /// declaration, a membership edit, a config load. Cheap enough to run
    /// wholesale (a scene is tens of entries, and this is never on the render
    /// path), which is why no caller has to work out which subset moved.
    ///
    /// What a switch does to an animation's lifecycle state is that animation's
    /// [`ConditionAction`]: `Reset` (the default) idles it on the way out and
    /// re-arms it on the way in, so a protocol step plays the same way every
    /// time it comes up; `Stop` idles it on the way out only; `Hold` leaves the
    /// state alone, freezing a running animation mid-flight.
    ///
    /// Idling goes through [`Self::disarm_animation`], not `cancel`: a
    /// condition switch is bookkeeping, and firing an animation's
    /// `cancel_action` — which can pulse a trigger line — would put a mark on
    /// the recording that no experiment asked for. The `anim_enabled` hold is
    /// released either way, which is the part that has to happen: a running
    /// animation left holding a stimulus hidden would go on hiding it for the
    /// *new* condition.
    pub fn apply_conditions(&mut self) {
        let active = self.config.conditions.active;

        for entry in self.config.stimuli.values_mut() {
            let on = active_in(&entry.conditions, active);
            let flags = entry.stimulus.flags_mut();
            if flags.cond_enabled != on {
                flags.cond_enabled = on;
                flags.mark_dirty();
            }
        }

        // Two passes: the transitions are collected first because idling one
        // animation borrows the whole scene (it reaches the stimuli to release
        // their holds), which cannot happen while the map is being iterated.
        let mut to_idle: Vec<u32> = Vec::new();
        let mut to_arm: Vec<u32> = Vec::new();
        for (&handle, entry) in self.config.animations.iter_mut() {
            let on = active_in(&entry.config.conditions, active);
            if entry.cond_enabled == on {
                continue;
            }
            entry.cond_enabled = on;
            match (entry.config.condition_action, on) {
                (ConditionAction::Hold, _) => {}
                (ConditionAction::Reset, true) => to_arm.push(handle),
                (ConditionAction::Reset | ConditionAction::Stop, false) => to_idle.push(handle),
                (ConditionAction::Stop, true) => {}
            }
        }
        for handle in to_idle {
            self.disarm_animation(handle);
        }
        for handle in to_arm {
            self.arm_animation(handle);
        }
    }

    // ── Deferred mode ─────────────────────────────────────────────────────────

    /// Start deferred mode: snapshot all live state into copy fields.
    pub fn begin_deferred(&mut self) {
        for entry in self.stimuli.values_mut() {
            entry.stimulus.make_copy();
        }
        self.background.make_copy();
        self.photodiode.make_copy();
        self.runtime.deferred_mode = true;
    }

    /// End deferred mode: schedule an atomic flip on the next frame boundary.
    ///
    /// Ending what was never begun does nothing. The flip promotes every copy
    /// slot over its live one, and outside deferred mode the copies are stale by
    /// construction — every ordinary write goes to `live` alone. Scheduling one
    /// anyway would undo, a frame later, whatever had been set in the meantime:
    /// a defensive "make sure deferred mode is off" from a client would quietly
    /// revert the scene.
    pub fn end_deferred(&mut self) {
        if !self.runtime.deferred_mode {
            return;
        }
        self.runtime.pending_flip = true;
        self.runtime.deferred_mode = false;
    }

    /// Promote all copy fields to live. Called by the render thread when
    /// `pending_flip` is set, before animation advance and tessellation.
    pub fn apply_flip(&mut self) {
        for entry in self.stimuli.values_mut() {
            entry.stimulus.flip();
        }
        self.background.flip();
        self.photodiode.flip();
        self.runtime.pending_flip = false;
    }

    // ── Scene commands ────────────────────────────────────────────────────────

    /// Remove every stimulus (except protected ones, unless `protected_too`).
    /// Animations are untouched — see [`Self::clear_animations`].
    pub fn clear_stimuli(&mut self, protected_too: bool) {
        if protected_too {
            self.stimuli.clear();
        } else {
            self.stimuli.retain(|_, e| e.stimulus.flags().protected);
        }
    }

    /// Remove every animation, whatever its state. Goes through
    /// [`Self::delete_animation`] so a running one releases the `anim_enabled`
    /// hold it placed on its stimuli.
    pub fn clear_animations(&mut self) {
        let handles: Vec<u32> = self.animations.keys().copied().collect();
        for h in handles {
            self.delete_animation(h);
        }
    }

    /// Clear the whole scene: animations first, then stimuli, so no animation
    /// is left driving a stimulus that no longer exists. Scene-wide settings
    /// (background, default colours, photodiode, VTL names) are not touched.
    pub fn clear_scene(&mut self, protected_too: bool) {
        self.clear_animations();
        self.clear_stimuli(protected_too);
    }

    pub fn set_all_enabled(&mut self, enabled: bool, protected_too: bool) {
        for entry in self.stimuli.values_mut() {
            if protected_too || !entry.stimulus.flags().protected {
                entry.stimulus.flags_mut().enabled = enabled;
            }
        }
    }

    /// Record a completed command in the ring buffer.
    /// Called from `handle_request` while the write lock is already held —
    /// no extra synchronisation needed.
    ///
    /// Takes the outcome rather than the `proto::Response` it came from: what the
    /// overlay's log shows is "did this command succeed", and reading a proto type
    /// here would make the scene speak the wire for two fields it does not own.
    pub fn push_command_log(
        &mut self,
        handle: u32,
        summary: String,
        ok: bool,
        response_handle: i32,
    ) {
        const MAX_LOG: usize = 200;
        if !ok {
            self.runtime.command_log_errors += 1;
        }
        self.runtime.command_log_total += 1;
        self.runtime.command_log.push_back(CommandEntry {
            elapsed_ms: self.runtime.server_start.elapsed().as_secs_f64() * 1000.0,
            handle,
            summary,
            ok,
            response: response_handle,
        });
        if self.runtime.command_log.len() > MAX_LOG {
            self.runtime.command_log.pop_front();
        }
    }

    // ── Config persistence ────────────────────────────────────────────────────

    pub fn load_snapshot(&mut self, cfg: SceneConfig, mode: super::scene_config::LoadMode) {
        match mode {
            super::scene_config::LoadMode::Replace => {
                self.config = cfg;
                self.fixup_after_load();
            }
            super::scene_config::LoadMode::Additive => {
                let stim_offset = self.config.next_stim_handle;
                let anim_offset = self.config.next_anim_handle;
                let additive_next_stim = cfg.next_stim_handle;
                let additive_next_anim = cfg.next_anim_handle;
                for (handle, entry) in cfg.stimuli {
                    let new_handle = handle + stim_offset;
                    self.config
                        .stimuli
                        .insert(new_handle, make_entry_dirty(entry));
                }
                self.config.conditions.merge_declared(&cfg.conditions);
                for (handle, mut anim) in cfg.animations {
                    for sh in anim.config.target.stimuli_mut() {
                        *sh += stim_offset;
                    }
                    anim.state = state_after_load(&anim.state);
                    anim.captured_user_enabled = None;
                    self.config.animations.insert(handle + anim_offset, anim);
                }
                self.config.next_stim_handle += additive_next_stim;
                self.config.next_anim_handle += additive_next_anim;
                // Condition indices are a scene-wide namespace, not handles, so
                // the merged-in memberships come across unrebased: an additive
                // load is how you *add* to a condition, not how you shadow it.
                self.apply_conditions();
            }
        }
    }

    fn fixup_after_load(&mut self) {
        for entry in self.config.stimuli.values_mut() {
            entry.stimulus.flags_mut().dirty = true;
            entry.stimulus.reset_dynamic_state();
            entry.stimulus.make_copy();
        }
        for anim in self.config.animations.values_mut() {
            anim.state = state_after_load(&anim.state);
            anim.captured_user_enabled = None;
        }
        self.config.background.make_copy();
        self.config.photodiode.make_copy();
        // `cond_enabled` is derived, never saved: a load restores the
        // memberships and the active index, and the gates follow from them.
        self.apply_conditions();
    }

    // ── Scene-config load/save ────────────────────────────────────────────────
    //
    // Name resolution and the scene-side apply. The file format and directory
    // layout live in `crate::scene_config_file`; the matching IPC commands live in
    // `ipc::scene_config_commands`. Nothing here speaks protobuf.

    /// Resolve `[<project>/]<name>` against the project unqualified names land
    /// in. One place, so every caller — the wire, the boot path, the overlay —
    /// spells a scene-config the same way.
    pub fn scene_config_ref(&self, name: &str) -> anyhow::Result<SceneConfigRef> {
        SceneConfigRef::parse(name, DEFAULT_PROJECT)
    }

    /// Path on disk for `[<project>/]<name>`, resolved the same way.
    pub fn scene_config_path_for(&self, name: &str) -> anyhow::Result<std::path::PathBuf> {
        Ok(scene_config_path(&self.runtime.storage_dir, &self.scene_config_ref(name)?))
    }

    /// Load a named scene-config into the scene, replacing (or, with
    /// `additive`, merging) the current scene and — if a VTL segment is present
    /// — its line names. Shared by the `LoadSceneConfig` command and the
    /// `[startup] load_config` boot path.
    pub fn load_named_config(
        &mut self,
        name: &str,
        additive: bool,
        vtl: Option<&mut VtlState>,
    ) -> anyhow::Result<()> {
        let path = self.scene_config_path_for(name)?;
        let (scene_cfg, sections) = load_config(&path)?;
        if let Some(v) = vtl {
            v.config.names = sections.vtl.names;
            v.sync_names_to_shm();
        }
        let mode = if additive {
            LoadMode::Additive
        } else {
            LoadMode::Replace
        };
        self.load_snapshot(scene_cfg, mode);
        Ok(())
    }

    /// Save the current scene and VTL line names to a named scene-config file,
    /// creating the project's `scene-configs/` directory if needed.
    pub fn save_named_config(&self, name: &str, vtl: Option<&VtlState>) -> anyhow::Result<()> {
        let path = self.scene_config_path_for(name)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let default_vtl = VtlConfig::default();
        let vtl_cfg = vtl.map_or(&default_vtl, |v| &v.config);
        save_config(&self.config, vtl_cfg, &path)
    }

    /// Quit-time save (`[startup] save_on_quit`): overwrite the last-session
    /// slot and write a timestamped archive so history is preserved. Both land
    /// in the `_session` project — per-rig state, not part of any study.
    /// Returns the archive's qualified name. Logs a warning once archives pile
    /// up past [`ARCHIVE_WARN_THRESHOLD`] — they are never pruned automatically.
    pub fn save_session_snapshot(&self, vtl: Option<&VtlState>) -> anyhow::Result<String> {
        let slot = format!("{SESSION_PROJECT}/{LAST_SESSION_CONFIG}");
        self.save_named_config(&slot, vtl)?;
        let archive = format!("{SESSION_PROJECT}/{}", archive_timestamp_name());
        self.save_named_config(&archive, vtl)?;

        let n = count_archive_configs(&self.runtime.storage_dir);
        if n > ARCHIVE_WARN_THRESHOLD {
            log::warn!(
                "vstimd: {n} timestamped session archives in {:?} — consider pruning old ones",
                crate::scene_config_file::scene_config_dir(&self.runtime.storage_dir, SESSION_PROJECT)
            );
        }
        Ok(archive)
    }
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}

/// The runtime state a loaded animation starts in.
///
/// A config's animation state is intent, not a resumable snapshot: `Armed`
/// means "this scene is meant to come up waiting for its trigger", which is
/// what makes an armed scene — a shipped demo, a rig's startup config —
/// reproducible across a load. Dropping it (as an unconditional reset to
/// `Idle` does) leaves an animation that never observes its trigger, so the
/// scene looks dead while every value in the file is correct.
///
/// `Running` is *not* resumed: the saved `frame_counter` describes a session
/// that is over, so a mid-run save reloads as `Armed` and starts from the
/// beginning. `Done` reloads as `Idle` — a finished animation is not re-run
/// behind the operator's back.
fn state_after_load(saved: &AnimState) -> AnimState {
    match saved {
        AnimState::Armed | AnimState::Running { .. } => AnimState::Armed,
        AnimState::Idle | AnimState::Done => AnimState::Idle,
    }
}

fn make_entry_dirty(mut entry: StimulusSceneEntry) -> StimulusSceneEntry {
    entry.stimulus.flags_mut().dirty = true;
    entry.stimulus.reset_dynamic_state();
    entry.stimulus.make_copy();
    entry
}
