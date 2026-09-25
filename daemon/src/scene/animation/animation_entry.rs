//! Serializable animation configuration (`AnimationConfig`) and the full scene
//! entry (`AnimationEntry`) that pairs it with non-serialized runtime state.

use super::{AnimState, Animation, CancelAction, FinalAction, StartAction};
use crate::scene::conditions::ConditionAction;
use crate::vtl_state::{VtlEdge, VtlBit};

/// What an animation drives.
///
/// The camera is a target rather than a sentinel stimulus handle: it arms,
/// triggers and cancels like any other animation without claiming a handle a
/// stimulus could one day be given (dev/3D_ROADMAP.md §11.1).
///
/// It is not a target for *every* animation. The visibility kinds have nothing to
/// act on, and the 2-D motion kinds move in pixels, which mean nothing for a
/// camera in centimetres — the same reason `Stimulus::move_to_2d` refuses a 3-D
/// stimulus. [`Animation::camera_target`] says which kinds take it; create
/// refuses every other pairing.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum AnimationTarget {
    Stimuli { handles: Vec<u32> },
    Camera,
}

impl AnimationTarget {
    /// Mutable access to the stimulus handles, for the additive-load handle
    /// rebase. Empty slice for targets that are not stimuli.
    pub fn stimuli_mut(&mut self) -> &mut [u32] {
        match self {
            AnimationTarget::Stimuli { handles } => handles,
            AnimationTarget::Camera => &mut [],
        }
    }

    /// The stimuli this animation drives — empty for targets that are not
    /// stimuli, so a caller that only knows how to move stimuli can iterate
    /// unconditionally.
    pub fn stimuli(&self) -> &[u32] {
        match self {
            AnimationTarget::Stimuli { handles } => handles,
            AnimationTarget::Camera => &[],
        }
    }
}

/// Serializable animation configuration.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimationConfig {
    pub name: String,
    pub state: AnimState,
    pub target: AnimationTarget,
    /// Bitflags applied when the animation transitions Armed → Running.
    pub start_action: StartAction,
    /// Output line to pulse for one frame when `START_ACTION_TRIGGER_LINE` is set.
    pub start_action_trigger_line: Option<VtlBit>,
    /// Bitflags controlling what happens when the animation completes.
    pub final_action: FinalAction,
    /// Output line to pulse for one frame when `FINAL_ACTION_TRIGGER_LINE` is set.
    pub final_action_trigger_line: Option<VtlBit>,
    /// Output line driven HIGH on completion (and LOW again when the animation
    /// next starts) when `DONE_LEVEL` is set. Separate from
    /// `final_action_trigger_line` so one animation can mark the moment on one
    /// line and hold the state on another.
    #[serde(default)]
    pub final_action_level_line: Option<VtlBit>,
    /// If `Some`, the animation waits for this edge before starting.
    pub start_trigger: Option<(VtlBit, VtlEdge)>,
    /// If `Some`, this input edge cancels the animation while it is `Armed` or
    /// `Running`. Same wiring as `start_trigger`; evaluated each frame in
    /// `advance_one`.
    #[serde(default)]
    pub cancel_trigger: Option<(VtlBit, VtlEdge)>,
    /// Bitflags applied when the animation is cancelled (edge or software).
    /// Independent of `final_action`; `empty()` means a hard abort that leaves
    /// visibility as-is.
    #[serde(default)]
    pub cancel_action: CancelAction,
    /// Output line to pulse for one frame when `CANCEL_ACTION_TRIGGER_LINE` is set.
    #[serde(default)]
    pub cancel_action_trigger_line: Option<VtlBit>,
    /// The conditions this animation is active in; empty means every condition.
    /// Outside them the animation does not advance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<u32>,
    /// What a condition switch does to this animation's lifecycle state.
    /// Omitted on save when it is the default, so an animation that says
    /// nothing about conditions serializes as it always did.
    #[serde(default, skip_serializing_if = "ConditionAction::is_default")]
    pub condition_action: ConditionAction,
    pub animation: Animation,
}

/// Full animation entry: serializable config + runtime state.
/// Deref/DerefMut give transparent access to the config fields.
#[derive(Clone)]
pub struct AnimationEntry {
    pub config: AnimationConfig,
    /// Snapshot of each stimulus's `user_enabled` taken when the animation first
    /// transitions to Running. Used by `RESTORE_VISIBILITY` to undo visibility changes.
    /// Not serialized — always None in saved configs.
    pub captured_user_enabled: Option<Vec<bool>>,
    /// False while the active condition excludes this animation. Derived from
    /// `config.conditions` by `SceneState::apply_conditions`, the same way
    /// `StimulusFlags::cond_enabled` is; not serialized.
    pub cond_enabled: bool,
    /// Net distance a navigation animation has moved its target since it last
    /// started, cm. Never wrapped: this is the number an experiment logs, while
    /// the camera position it drives wraps into one corridor period. `f64`
    /// because a session covers kilometres, where `f32` resolves only
    /// millimetres. Zero for every other kind; not serialized.
    pub distance_travelled_cm: f64,
    /// The camera `(x, z)` a navigation animation is integrating, in `f64`,
    /// with the position it last wrote. Narrowing to `f32` every frame and
    /// reading it back accumulates rounding — millimetres over a session — so
    /// the animation keeps its own copy and resyncs from the camera only when
    /// something else has moved it. Not serialized.
    pub nav_position_cm: Option<NavPosition>,
    /// Where a finite-track navigation animation is on its track. Reset with
    /// `nav_position_cm`; not serialized.
    pub track_progress: TrackProgress,
}

/// Progress along a [`Track3D`](super::Track3D).
#[derive(Clone, Copy, Debug, Default)]
pub struct TrackProgress {
    /// The camera `(x, z)` and yaw when the animation started — the track's
    /// beginning, and where the camera is sent back to. `None` until the first
    /// frame the animation runs.
    pub start: Option<TrackStart>,
    pub phase: TrackPhase,
    /// Times the camera has been sent back to the start.
    pub laps: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackStart {
    pub x: f64,
    pub z: f64,
    pub yaw_deg: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrackPhase {
    #[default]
    Moving,
    /// Fading out, frames left before the jump. The camera holds still.
    FadingOut(u32),
    /// Fading in after the jump, frames left. The camera moves.
    FadingIn(u32),
}

impl TrackProgress {
    /// How far the 3-D view is faded to the background this frame: 0 clear,
    /// 1 fully faded. Reaches exactly 1 on the frame of the jump, so the jump is
    /// never seen.
    pub fn fade(&self, fade_frames: u32) -> f32 {
        let f = fade_frames.max(1) as f32;
        match self.phase {
            TrackPhase::Moving => 0.0,
            TrackPhase::FadingOut(left) => 1.0 - left as f32 / f,
            TrackPhase::FadingIn(left) => left as f32 / f,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NavPosition {
    pub x: f64,
    pub z: f64,
    pub written: glam::Vec3,
}

impl std::ops::Deref for AnimationEntry {
    type Target = AnimationConfig;
    fn deref(&self) -> &AnimationConfig { &self.config }
}

impl std::ops::DerefMut for AnimationEntry {
    fn deref_mut(&mut self) -> &mut AnimationConfig { &mut self.config }
}

impl serde::Serialize for AnimationEntry {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.config.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for AnimationEntry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self {
            config: AnimationConfig::deserialize(d)?,
            captured_user_enabled: None,
            cond_enabled: true,
            distance_travelled_cm: 0.0,
            nav_position_cm: None,
            track_progress: TrackProgress::default(),
        })
    }
}

impl AnimationEntry {
    pub fn new(animation: Animation, stimuli: Vec<u32>) -> Self {
        Self {
            config: AnimationConfig {
                name: String::new(),
                state: AnimState::Idle,
                target: AnimationTarget::Stimuli { handles: stimuli },
                start_action: StartAction::empty(),
                start_action_trigger_line: None,
                final_action: FinalAction::empty(),
                final_action_trigger_line: None,
                final_action_level_line: None,
                start_trigger: None,
                cancel_trigger: None,
                cancel_action: CancelAction::empty(),
                cancel_action_trigger_line: None,
                conditions: Vec::new(),
                condition_action: ConditionAction::default(),
                animation,
            },
            captured_user_enabled: None,
            cond_enabled: true,
            distance_travelled_cm: 0.0,
            nav_position_cm: None,
            track_progress: TrackProgress::default(),
        }
    }

    pub fn armed(animation: Animation, stimuli: Vec<u32>) -> Self {
        let mut e = Self::new(animation, stimuli);
        e.state = AnimState::Armed;
        e
    }
}
