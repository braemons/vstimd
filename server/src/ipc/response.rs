//! Constructors for the protobuf `Response` variants, shared by every
//! command impl in this module.

use crate::proto;
use crate::scene::stimulus::{Stimulus, StimulusType};
use uuid::Uuid;

pub(crate) fn ok_ack() -> proto::Response {
    proto::Response { handle: -1, code: proto::ErrorCode::Ok as i32, ..Default::default() }
}

pub(crate) fn ok_handle_with_id(h: u32, id: &Uuid) -> proto::Response {
    proto::Response {
        handle: h as i32,
        code: proto::ErrorCode::Ok as i32,
        id: id.to_string(),
        ..Default::default()
    }
}

pub(crate) fn ok_body(body: proto::response::Body) -> proto::Response {
    proto::Response {
        handle: -1,
        code: proto::ErrorCode::Ok as i32,
        body: Some(body),
        ..Default::default()
    }
}

pub(crate) fn err(code: proto::ErrorCode, msg: impl Into<String>) -> proto::Response {
    proto::Response { code: code as i32, error: msg.into(), ..Default::default() }
}

pub(crate) fn err_not_found(handle: u32) -> proto::Response {
    proto::Response {
        code: proto::ErrorCode::HandleNotFound as i32,
        error: format!("stimulus handle {} not found", handle),
        ..Default::default()
    }
}

pub(crate) fn ok_handle(h: u32) -> proto::Response {
    proto::Response { handle: h as i32, code: proto::ErrorCode::Ok as i32, ..Default::default() }
}

/// A stimulus that is not placed in 2-D space, where the command only makes sense
/// there. Separate from [`err_wrong_type`] because the requirement is a *dimension*,
/// not a type: `SetPosition` takes pixels, which mean nothing in world space, and it
/// is refused by every 3-D type rather than by all but one 2-D one.
pub(crate) fn err_not_2d(stim: &Stimulus, cmd: &str) -> proto::Response {
    proto::Response {
        code: proto::ErrorCode::WrongStimulusType as i32,
        error: format!(
            "{} requires a 2-D stimulus, got {}",
            cmd,
            stim.type_name()
        ),
        ..Default::default()
    }
}

/// `expected` is a [`StimulusType`], not a name: the only spelling of a type a client
/// may see comes from [`StimulusType::type_name`], so it cannot drift from what a
/// query reports for the same stimulus.
pub(crate) fn err_wrong_type(
    stim: &Stimulus,
    cmd: &str,
    expected: StimulusType,
) -> proto::Response {
    proto::Response {
        code: proto::ErrorCode::WrongStimulusType as i32,
        error: format!(
            "{} requires a {} stimulus, got {}",
            cmd,
            expected.type_name(),
            stim.type_name()
        ),
        ..Default::default()
    }
}
