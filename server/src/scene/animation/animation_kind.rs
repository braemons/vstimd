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
    /// Set 2-D position every frame from the first two axes of an input device
    /// — both absolute, in pixels after the rig-config's scale — plus an offset.
    ///
    /// `shm_name` names the device: a rig-config device whose `name` or `shm`
    /// matches. The field keeps its old name for wire compatibility.
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
        /// Negative moves backwards. Ignored while `source` is set.
        speed_cm_per_s: f32,
        wrap_period_cm: Option<f32>,
        /// Drive the speed from an input device instead: a rate axis is a speed
        /// in cm/s integrated over real frame time, a cumulative axis moves the
        /// camera by exactly its change each frame.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<AxisRef>,
    },
    /// Map input-device axes onto transform channels of the target — stimuli
    /// or the camera — every frame. How an axis becomes a channel update is its
    /// semantic's business: absolute sets the channel, cumulative adds its
    /// change, rate adds its value times the frame time. Never finishes.
    DeviceDrivenTransform { device: String, axes: Vec<AxisMap> },
}

/// One axis of one rig-config input device, by name.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AxisRef {
    pub device: String,
    pub axis: String,
}

/// What an [`AxisMap`] drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransformChannel {
    /// Position: pixels on a 2-D stimulus, centimetres in 3-D and on the camera.
    PosX,
    PosY,
    PosZ,
    /// Degrees. `Yaw` is the rotation of a 2-D stimulus.
    Yaw,
    Pitch,
    Roll,
    /// 3-D stimuli only.
    ScaleX,
    ScaleY,
    ScaleZ,
    ScaleUniform,
    /// The camera's horizontal forward and rightward directions, cm. Camera only.
    Forward,
    Strafe,
}

impl TransformChannel {
    pub fn is_camera_local(self) -> bool {
        matches!(self, Self::Forward | Self::Strafe)
    }

    pub fn is_scale(self) -> bool {
        matches!(self, Self::ScaleX | Self::ScaleY | Self::ScaleZ | Self::ScaleUniform)
    }
}

/// One device axis driving one transform channel.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AxisMap {
    /// The axis' name in the rig-config.
    pub axis: String,
    pub channel: TransformChannel,
    /// Device units → channel units (px, cm or degrees).
    pub gain: f32,
    /// Absolute and rate values within ±deadzone (device units) read as zero.
    #[serde(default)]
    pub deadzone: f32,
    #[serde(default)]
    pub invert: bool,
    /// Keep the channel within `[min, max]` after each update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clamp: Option<[f32; 2]>,
    /// Wrap the channel into `[0, wrap)` after each update — a corridor period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<f32>,
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
            Self::DeviceDrivenTransform { .. } => "DeviceDrivenTransform",
        }
    }

    /// Which targets this kind takes.
    pub fn camera_target(&self) -> CameraTarget {
        match self {
            Self::LinearNav3D { .. } => CameraTarget::Required,
            Self::DeviceDrivenTransform { .. } => CameraTarget::Allowed,
            _ => CameraTarget::Forbidden,
        }
    }
}

/// Whether an animation kind drives the camera, stimuli, or either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraTarget {
    Required,
    Allowed,
    Forbidden,
}
