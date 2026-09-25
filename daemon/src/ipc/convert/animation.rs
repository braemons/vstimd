//! Animation <-> proto conversions: the `CreateAnimationRequest` body in both
//! directions, plus the trigger edge and polarity enums it carries.

use super::super::response::err;
use super::vtl::{vtl_bit_from_proto, vtl_bit_to_proto};
use crate::proto;
use crate::scene::animation::{
    Animation, AxisMap, AxisRef, Track3D, TransformChannel, VtlEdge, VtlPolarity,
};
use crate::scene::VtlBit;
use crate::vtl_state::VtlNameEntry;

pub(crate) fn vtl_edge_to_proto(e: VtlEdge) -> i32 {
    match e {
        VtlEdge::Rising => proto::VtlEdge::Rising as i32,
        VtlEdge::Falling => proto::VtlEdge::Falling as i32,
    }
}

pub(crate) fn vtl_polarity_to_proto(p: VtlPolarity) -> i32 {
    match p {
        VtlPolarity::ActiveHigh => proto::VtlPolarity::ActiveHigh as i32,
        VtlPolarity::ActiveLow => proto::VtlPolarity::ActiveLow as i32,
    }
}

pub(crate) fn animation_body_to_proto(anim: &Animation) -> proto::create_animation_request::Body {
    use proto::create_animation_request::Body as PBody;
    match anim {
        Animation::CoupleVisibilityToTriggerLine { trigger, polarity } => {
            PBody::CoupleVisibilityToTriggerLine(proto::CoupleVisibilityToTriggerLine {
                trigger: Some(vtl_bit_to_proto(*trigger)),
                polarity: vtl_polarity_to_proto(*polarity),
            })
        }
        Animation::EnableOnTriggerEdge {
            trigger,
            edge,
            enabled,
        } => PBody::EnableOnTriggerEdge(proto::EnableOnTriggerEdge {
            trigger: Some(vtl_bit_to_proto(*trigger)),
            edge: vtl_edge_to_proto(*edge),
            enabled: *enabled,
        }),
        Animation::FlashForNFrames { duration_frames } => {
            PBody::FlashForNFrames(proto::FlashForNFrames {
                duration_frames: *duration_frames,
            })
        }
        Animation::FlickerForNFrames {
            on_frames,
            off_frames,
            total_frames,
            start_on_phase,
        } => PBody::FlickerForNFrames(proto::FlickerForNFrames {
            on_frames: *on_frames,
            off_frames: *off_frames,
            total_frames: *total_frames,
            start_on_phase: *start_on_phase,
        }),
        Animation::MoveAlongPath2D { coords_px } => PBody::MoveAlongPath2d(proto::MoveAlongPath2D {
            x_px: coords_px.iter().map(|c| c[0]).collect(),
            y_px: coords_px.iter().map(|c| c[1]).collect(),
        }),
        Animation::MoveAlongSegments2D {
            waypoints_px,
            speed_px_per_sec,
        } => PBody::MoveAlongSegments2d(proto::MoveAlongSegments2D {
            x_px: waypoints_px.iter().map(|w| w[0]).collect(),
            y_px: waypoints_px.iter().map(|w| w[1]).collect(),
            speed_px_per_sec: *speed_px_per_sec,
        }),
        Animation::ExternalPosition2D {
            shm_name,
            x_offset_px,
            y_offset_px,
        } => PBody::ExternalPosition2d(proto::ExternalPosition2D {
            shm_name: shm_name.clone(),
            x_offset_px: *x_offset_px,
            y_offset_px: *y_offset_px,
        }),
        Animation::LinearNav3D { speed_cm_per_s, wrap_period_cm, source, track } => {
            PBody::LinearNav3d(proto::LinearNav3D {
                track_length_cm: track.map_or(0.0, |t| t.length_cm),
                fade_frames: track.map_or(0, |t| t.fade_frames),
                speed_cm_per_s: *speed_cm_per_s,
                wrap_period_cm: wrap_period_cm.unwrap_or(0.0),
                source: source
                    .as_ref()
                    .map(|s| proto::AxisRef { device: s.device.clone(), axis: s.axis.clone() }),
            })
        }
        Animation::DeviceDrivenTransform { device, axes } => {
            PBody::DeviceDrivenTransform(proto::DeviceDrivenTransform {
                device: device.clone(),
                axes: axes.iter().map(axis_map_to_proto).collect(),
            })
        }
    }
}

pub(crate) fn vtl_edge_from_proto(e: i32) -> VtlEdge {
    match proto::VtlEdge::try_from(e).unwrap_or(proto::VtlEdge::Rising) {
        proto::VtlEdge::Rising => VtlEdge::Rising,
        proto::VtlEdge::Falling => VtlEdge::Falling,
    }
}

pub(crate) fn vtl_polarity_from_proto(p: i32) -> VtlPolarity {
    match proto::VtlPolarity::try_from(p).unwrap_or(proto::VtlPolarity::ActiveHigh) {
        proto::VtlPolarity::ActiveHigh => VtlPolarity::ActiveHigh,
        proto::VtlPolarity::ActiveLow => VtlPolarity::ActiveLow,
    }
}

pub(crate) fn animation_from_proto(
    cmd: &proto::CreateAnimationRequest,
    vtl_names: &[VtlNameEntry],
) -> Result<Animation, Box<proto::Response>> {
    use proto::create_animation_request::Body as PBody;

    let vtl_bit =
        |h: Option<&proto::VirtualTriggerLineHandle>| -> Result<VtlBit, Box<proto::Response>> {
            vtl_bit_from_proto(h, vtl_names)
        };

    match cmd.body.as_ref() {
        Some(PBody::CoupleVisibilityToTriggerLine(c)) => {
            Ok(Animation::CoupleVisibilityToTriggerLine {
                trigger: vtl_bit(c.trigger.as_ref())?,
                polarity: vtl_polarity_from_proto(c.polarity),
            })
        }
        Some(PBody::EnableOnTriggerEdge(c)) => Ok(Animation::EnableOnTriggerEdge {
            trigger: vtl_bit(c.trigger.as_ref())?,
            edge: vtl_edge_from_proto(c.edge),
            enabled: c.enabled,
        }),
        Some(PBody::FlashForNFrames(c)) => Ok(Animation::FlashForNFrames {
            duration_frames: c.duration_frames,
        }),
        Some(PBody::FlickerForNFrames(c)) => Ok(Animation::FlickerForNFrames {
            on_frames: c.on_frames,
            off_frames: c.off_frames,
            total_frames: c.total_frames,
            start_on_phase: c.start_on_phase,
        }),
        Some(PBody::MoveAlongPath2d(c)) => {
            if c.x_px.len() != c.y_px.len() {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "MoveAlongPath2D: x and y must have equal length",
                )));
            }
            Ok(Animation::MoveAlongPath2D {
                coords_px: c.x_px.iter().zip(c.y_px.iter()).map(|(&x, &y)| [x, y]).collect(),
            })
        }
        Some(PBody::MoveAlongSegments2d(c)) => {
            if c.x_px.len() != c.y_px.len() {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "MoveAlongSegments2D: x and y must have equal length",
                )));
            }
            if c.x_px.len() < 2 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "MoveAlongSegments2D: at least 2 waypoints_px required",
                )));
            }
            Ok(Animation::MoveAlongSegments2D {
                waypoints_px: c.x_px.iter().zip(c.y_px.iter()).map(|(&x, &y)| [x, y]).collect(),
                speed_px_per_sec: c.speed_px_per_sec,
            })
        }
        Some(PBody::ExternalPosition2d(c)) => Ok(Animation::ExternalPosition2D {
            shm_name: c.shm_name.clone(),
            x_offset_px: c.x_offset_px,
            y_offset_px: c.y_offset_px,
        }),
        Some(PBody::DeviceDrivenTransform(c)) => {
            let axes = c
                .axes
                .iter()
                .map(axis_map_from_proto)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|msg| Box::new(err(proto::ErrorCode::InvalidArgument, msg)))?;
            Ok(Animation::DeviceDrivenTransform { device: c.device.clone(), axes })
        }
        Some(PBody::LinearNav3d(c)) => {
            if !c.speed_cm_per_s.is_finite() || !c.wrap_period_cm.is_finite() || c.wrap_period_cm < 0.0 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "LinearNav3D: speed_cm_per_s must be finite and wrap_period_cm finite and non-negative",
                )));
            }
            if !c.track_length_cm.is_finite() || c.track_length_cm < 0.0 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "LinearNav3D: track_length_cm must be finite and non-negative",
                )));
            }
            if c.track_length_cm > 0.0 && c.wrap_period_cm > 0.0 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "LinearNav3D: give wrap_period_cm (an endless corridor) or track_length_cm (a finite track), not both",
                )));
            }
            Ok(Animation::LinearNav3D {
                speed_cm_per_s: c.speed_cm_per_s,
                wrap_period_cm: (c.wrap_period_cm > 0.0).then_some(c.wrap_period_cm),
                track: (c.track_length_cm > 0.0).then_some(Track3D {
                    length_cm: c.track_length_cm,
                    fade_frames: c.fade_frames,
                }),
                source: c
                    .source
                    .as_ref()
                    .map(|s| AxisRef { device: s.device.clone(), axis: s.axis.clone() }),
            })
        }
        None => Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "animation body must be set",
        ))),
    }
}

/// The request's target → the scene's. Absent means an empty stimulus list, as
/// it always has.
pub(crate) fn animation_target_from_proto(
    target: Option<proto::AnimationTarget>,
) -> crate::scene::animation::AnimationTarget {
    use crate::scene::animation::AnimationTarget;
    match target.and_then(|t| t.target) {
        Some(proto::animation_target::Target::Stimuli(s)) => {
            AnimationTarget::Stimuli { handles: s.handles }
        }
        Some(proto::animation_target::Target::Camera(_)) => AnimationTarget::Camera,
        None => AnimationTarget::Stimuli { handles: Vec::new() },
    }
}

pub(crate) fn animation_target_to_proto(
    target: &crate::scene::animation::AnimationTarget,
) -> proto::AnimationTarget {
    use crate::scene::animation::AnimationTarget;
    proto::AnimationTarget {
        target: Some(match target {
            AnimationTarget::Stimuli { handles } => {
                proto::animation_target::Target::Stimuli(proto::AnimationStimuli {
                    handles: handles.clone(),
                })
            }
            AnimationTarget::Camera => {
                proto::animation_target::Target::Camera(proto::AnimationCamera {})
            }
        }),
    }
}

fn channel_from_proto(v: i32) -> Result<TransformChannel, String> {
    use proto::TransformChannel as P;
    Ok(match P::try_from(v).unwrap_or(P::Unspecified) {
        P::Unspecified => return Err("axis mapping needs a channel".into()),
        P::PosX => TransformChannel::PosX,
        P::PosY => TransformChannel::PosY,
        P::PosZ => TransformChannel::PosZ,
        P::Yaw => TransformChannel::Yaw,
        P::Pitch => TransformChannel::Pitch,
        P::Roll => TransformChannel::Roll,
        P::ScaleX => TransformChannel::ScaleX,
        P::ScaleY => TransformChannel::ScaleY,
        P::ScaleZ => TransformChannel::ScaleZ,
        P::ScaleUniform => TransformChannel::ScaleUniform,
        P::Forward => TransformChannel::Forward,
        P::Strafe => TransformChannel::Strafe,
    })
}

fn channel_to_proto(c: TransformChannel) -> proto::TransformChannel {
    use proto::TransformChannel as P;
    match c {
        TransformChannel::PosX => P::PosX,
        TransformChannel::PosY => P::PosY,
        TransformChannel::PosZ => P::PosZ,
        TransformChannel::Yaw => P::Yaw,
        TransformChannel::Pitch => P::Pitch,
        TransformChannel::Roll => P::Roll,
        TransformChannel::ScaleX => P::ScaleX,
        TransformChannel::ScaleY => P::ScaleY,
        TransformChannel::ScaleZ => P::ScaleZ,
        TransformChannel::ScaleUniform => P::ScaleUniform,
        TransformChannel::Forward => P::Forward,
        TransformChannel::Strafe => P::Strafe,
    }
}

fn axis_map_from_proto(m: &proto::AxisMap) -> Result<AxisMap, String> {
    let clamp = match (m.clamp_min, m.clamp_max) {
        (Some(lo), Some(hi)) => Some([lo, hi]),
        (None, None) => None,
        _ => return Err(format!("axis '{}': clamp needs both clamp_min and clamp_max", m.axis)),
    };
    Ok(AxisMap {
        axis: m.axis.clone(),
        channel: channel_from_proto(m.channel)?,
        gain: m.gain,
        deadzone: m.deadzone,
        invert: m.invert,
        clamp,
        wrap: (m.wrap > 0.0).then_some(m.wrap),
    })
}

fn axis_map_to_proto(m: &AxisMap) -> proto::AxisMap {
    proto::AxisMap {
        axis: m.axis.clone(),
        channel: channel_to_proto(m.channel) as i32,
        gain: m.gain,
        deadzone: m.deadzone,
        invert: m.invert,
        clamp_min: m.clamp.map(|c| c[0]),
        clamp_max: m.clamp.map(|c| c[1]),
        wrap: m.wrap.unwrap_or(0.0),
    }
}
