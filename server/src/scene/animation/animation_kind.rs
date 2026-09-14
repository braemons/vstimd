//! The `Animation` enum — the kind of behaviour an animation drives, plus its
//! per-kind parameters. Advancing these each frame lives in [`super::advance`].

use crate::vtl_state::{VtlEdge, VtlBit, VtlPolarity};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Animation {
    /// Mirror stimulus enabled state to the level of a trigger line (input or output).
    CoupleVisibilityToTriggerLine {
        trigger: VtlBit,
        polarity: VtlPolarity,
    },
    /// Set stimulus enabled once when a trigger edge fires.
    EnableOnTriggerEdge {
        trigger: VtlBit,
        edge: VtlEdge,
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
        /// If false, start in the off-phase_cycles instead of the on-phase_cycles.
        start_on_phase: bool,
    },
    /// Move stimulus through a preloaded sequence of positions, one per frame.
    MoveAlongPath2D { coords_px: Vec<[f32; 2]> },
    /// Move stimulus along piecewise-linear waypoints_px at a constant speed.
    MoveAlongSegments2D {
        waypoints_px: Vec<[f32; 2]>,
        speed_px_per_sec: f32,
    },
    /// Read 2-D position from a POSIX shm float array each frame.
    ///
    /// TODO(#84): unimplemented — `animation_advance` never reads the segment.
    ExternalPosition2D {
        shm_name: String,
        x_offset_px: f32,
        y_offset_px: f32,
    },
    /// Move the camera straight ahead at a constant speed — its horizontal
    /// forward direction, so pitch tilts the view without flying the camera
    /// into the floor. Never finishes on its own.
    ///
    /// With `wrap_period_cm`, the camera's `z` wraps into `[0, L)`: the endless
    /// corridor is periodic geometry the camera circulates through, not geometry
    /// that grows. The true distance is kept separately, unwrapped, in
    /// `AnimationEntry::distance_travelled_cm`.
    ///
    /// Speed is set by command. Driving it from a treadmill is the input-device
    /// work (#79), not a per-frame command stream.
    LinearNav3D {
        /// Negative moves backwards.
        speed_cm_per_s: f32,
        wrap_period_cm: Option<f32>,
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
            Self::LinearNav3D { .. } => "LinearNav3D",
        }
    }

    /// Whether this kind drives the camera — the only kinds an
    /// `AnimationTarget::Camera` accepts, and the only target they accept.
    pub fn drives_camera(&self) -> bool {
        matches!(self, Self::LinearNav3D { .. })
    }
}
