//! Animation <-> proto conversions: the `CreateAnimationRequest` body in both
//! directions, plus the trigger edge and polarity enums it carries.

use super::super::response::err;
use super::vtl::{vtl_bit_from_proto, vtl_bit_to_proto};
use crate::proto;
use crate::scene::animation::{Animation, VtlEdge, VtlPolarity};
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
        Animation::MoveAlongPath2D { coords } => PBody::MoveAlongPath2d(proto::MoveAlongPath2D {
            x: coords.iter().map(|c| c[0]).collect(),
            y: coords.iter().map(|c| c[1]).collect(),
        }),
        Animation::MoveAlongSegments2D {
            waypoints,
            speed_px_per_sec,
        } => PBody::MoveAlongSegments2d(proto::MoveAlongSegments2D {
            x: waypoints.iter().map(|w| w[0]).collect(),
            y: waypoints.iter().map(|w| w[1]).collect(),
            speed_px_per_sec: *speed_px_per_sec,
        }),
        Animation::ExternalPosition2D {
            shm_name,
            x_offset,
            y_offset,
        } => PBody::ExternalPosition2d(proto::ExternalPosition2D {
            shm_name: shm_name.clone(),
            x_offset: *x_offset,
            y_offset: *y_offset,
        }),
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
            if c.x.len() != c.y.len() {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "MoveAlongPath2D: x and y must have equal length",
                )));
            }
            Ok(Animation::MoveAlongPath2D {
                coords: c.x.iter().zip(c.y.iter()).map(|(&x, &y)| [x, y]).collect(),
            })
        }
        Some(PBody::MoveAlongSegments2d(c)) => {
            if c.x.len() != c.y.len() {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "MoveAlongSegments2D: x and y must have equal length",
                )));
            }
            if c.x.len() < 2 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "MoveAlongSegments2D: at least 2 waypoints required",
                )));
            }
            Ok(Animation::MoveAlongSegments2D {
                waypoints: c.x.iter().zip(c.y.iter()).map(|(&x, &y)| [x, y]).collect(),
                speed_px_per_sec: c.speed_px_per_sec,
            })
        }
        // Refused rather than accepted-and-ignored: `advance_one` never opens the
        // segment, so an accepted ExternalPosition2D arms, runs forever and moves
        // nothing while reporting success — the stimulus silently stays put for a
        // whole session. Returning NOT_SUPPORTED until #84 lands is the honest
        // answer; the fields are kept so the message does not have to change.
        Some(PBody::ExternalPosition2d(_)) => Err(Box::new(err(
            proto::ErrorCode::NotSupported,
            "ExternalPosition2D is not implemented yet (see \
             https://github.com/braemons/vstimd/issues/84): the shared-memory \
             segment is never read, so the stimulus would not move",
        ))),
        None => Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "animation body must be set",
        ))),
    }
}
