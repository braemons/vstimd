bitflags::bitflags! {
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
    pub struct StartAction: u8 {
        /// Enable stimuli when animation transitions Armed → Running.
        const ENABLE                      = 0x02;
        const TOGGLE_PHOTODIODE           = 0x04;
        const START_ACTION_TRIGGER_LINE   = 0x08;
    }
}

bitflags::bitflags! {
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FinalAction: u8 {
        const DISABLE                 = 0x01;
        const TOGGLE_PHOTODIODE       = 0x04;
        const FINAL_ACTION_TRIGGER_LINE = 0x08;
        const RESTART                 = 0x10;
        const REVERSE                 = 0x20;
        const RESTORE_STATE           = 0x40;
        const END_DEFERRED            = 0x80;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnimState {
    Idle,
    Armed,
    Running { frame_counter: u32 },
    Done,
}

pub use crate::vtl_state::{Edge, VtlBit};

#[derive(Clone, Debug)]
pub enum Animation {
    /// Mirror stimulus enabled state to the level of a trigger line (input or output).
    CoupleVisibilityToTriggerLine { trigger: VtlBit, polarity: bool },
    /// Set stimulus enabled once when a trigger edge fires.
    EnableOnTriggerEdge {
        trigger: VtlBit,
        edge: Edge,
        enabled: bool,
    },
    /// Enable stimuli for `duration_frames`.
    FlashForNFrames { duration_frames: u32 },
    /// Flicker stimuli on/off.
    FlickerForNFrames {
        on_frames: u32,
        off_frames: u32,
        /// None = run forever.
        total_frames: Option<u32>,
        /// If false, start in the off-phase instead of the on-phase.
        start_on_phase: bool,
    },
    /// Move stimulus through a preloaded sequence of positions, one per frame.
    MoveAlongPath2D { coords: Vec<[f32; 2]> },
    /// Move stimulus along piecewise-linear waypoints at a constant speed.
    MoveAlongSegments2D {
        waypoints: Vec<[f32; 2]>,
        speed_px_per_sec: f32,
    },
    /// Read 2-D position from a POSIX shm float array each frame.
    ExternalPosition2D {
        shm_name: String,
        x_offset: f32,
        y_offset: f32,
    },
}

impl Animation {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::CoupleVisibilityToTriggerLine { .. } => "CoupleVisibilityToTriggerLine",
            Self::EnableOnTriggerEdge { .. } => "EnableOnTriggerEdge",
            Self::FlashForNFrames { .. } => "FlashForNFrames",
            Self::FlickerForNFrames { .. } => "FlickerForNFrames",
            Self::MoveAlongPath2D { .. } => "MoveAlongPath2D",
            Self::MoveAlongSegments2D { .. } => "MoveAlongSegments2D",
            Self::ExternalPosition2D { .. } => "ExternalPosition2D",
        }
    }
}

pub struct AnimationEntry {
    pub name: String,
    pub state: AnimState,
    pub stimuli: Vec<u32>,
    /// Bitflags applied when the animation transitions Armed → Running.
    pub start_action: StartAction,
    /// Output line to pulse for one frame when `START_ACTION_TRIGGER_LINE` is set.
    pub start_action_trigger_line: Option<VtlBit>,
    /// Bitflags controlling what happens when the animation completes.
    pub final_action: FinalAction,
    /// Output line to pulse for one frame when `FINAL_ACTION_TRIGGER_LINE` is set.
    pub final_action_trigger_line: Option<VtlBit>,
    /// If `Some`, the animation waits for this edge before starting.
    pub start_trigger: Option<(VtlBit, Edge)>,
    /// Snapshot of each stimulus's `user_enabled` taken when the animation first
    /// transitions to Running. Used by `RESTORE_STATE` to undo visibility changes.
    pub captured_user_enabled: Option<Vec<bool>>,
    pub animation: Animation,
}

impl AnimationEntry {
    pub fn new(animation: Animation, stimuli: Vec<u32>) -> Self {
        Self {
            name: String::new(),
            state: AnimState::Idle,
            stimuli,
            start_action: StartAction::empty(),
            start_action_trigger_line: None,
            final_action: FinalAction::empty(),
            final_action_trigger_line: None,
            start_trigger: None,
            captured_user_enabled: None,
            animation,
        }
    }

    pub fn armed(animation: Animation, stimuli: Vec<u32>) -> Self {
        let mut e = Self::new(animation, stimuli);
        e.state = AnimState::Armed;
        e
    }
}
