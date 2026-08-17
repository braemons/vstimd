//! Constructors for the protobuf `Response` variants, shared by every
//! command impl in this module.

use crate::proto;
use crate::scene::stimulus::Stimulus;
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

pub(crate) fn err_wrong_type(stim: &Stimulus, cmd: &str, expected: &str) -> proto::Response {
    proto::Response {
        code: proto::ErrorCode::WrongStimulusType as i32,
        error: format!("{} requires a {} stimulus, got {}", cmd, expected, stim.type_name()),
        ..Default::default()
    }
}
