//! Condition commands: switching the active condition, declaring the named
//! set, listing it, and editing per-stimulus / per-animation membership.
//!
//! The scene owns the semantics ([`crate::scene::conditions`] and the
//! `SceneState` methods it documents); this module only decodes the wire and
//! turns a scene-side refusal into an error code.

use super::convert::{condition_action_from_proto, condition_from_proto, condition_to_proto};
use super::response::{err, err_not_found, ok_ack, ok_body};
use crate::proto;
use crate::scene::SceneState;

impl SceneState {
    // ── SetCondition ──────────────────────────────────────────────────────────

    pub(super) fn cmd_set_condition(&mut self, cmd: proto::SetConditionRequest) -> proto::Response {
        match cmd.condition {
            Some(proto::set_condition_request::Condition::Index(index)) => {
                self.set_condition(index);
                ok_ack()
            }
            Some(proto::set_condition_request::Condition::Name(name)) => {
                match self.set_condition_by_name(&name) {
                    Ok(_) => ok_ack(),
                    Err(msg) => err(proto::ErrorCode::InvalidArgument, msg),
                }
            }
            None => err(
                proto::ErrorCode::InvalidArgument,
                "SetCondition needs an index or a name",
            ),
        }
    }

    // ── DeclareConditions ─────────────────────────────────────────────────────

    pub(super) fn cmd_declare_conditions(
        &mut self,
        cmd: proto::DeclareConditionsRequest,
    ) -> proto::Response {
        let declared = cmd.conditions.into_iter().map(condition_from_proto).collect();
        match self.declare_conditions(declared) {
            Ok(()) => ok_ack(),
            Err(msg) => err(proto::ErrorCode::InvalidArgument, msg),
        }
    }

    // ── ListConditions ────────────────────────────────────────────────────────

    pub(super) fn cmd_list_conditions(&self) -> proto::Response {
        let c = &self.config.conditions;
        ok_body(proto::response::Body::ConditionList(
            proto::ListConditionsResponse {
                conditions: c.declared.iter().map(condition_to_proto).collect(),
                active_index: c.active,
                active_name: c.active_name().to_string(),
            },
        ))
    }

    // ── SetStimulusConditions / SetAnimationConditions ────────────────────────

    pub(super) fn cmd_set_stimulus_conditions(
        &mut self,
        handle: u32,
        cmd: proto::SetStimulusConditionsRequest,
    ) -> proto::Response {
        if self.set_stimulus_conditions(handle, cmd.condition_indices) {
            ok_ack()
        } else {
            err_not_found(handle)
        }
    }

    pub(super) fn cmd_set_animation_conditions(
        &mut self,
        cmd: proto::SetAnimationConditionsRequest,
    ) -> proto::Response {
        let action = condition_action_from_proto(cmd.condition_action);
        if self.set_animation_conditions(cmd.handle, cmd.condition_indices, action) {
            ok_ack()
        } else {
            err(
                proto::ErrorCode::HandleNotFound,
                format!("animation handle {} not found", cmd.handle),
            )
        }
    }
}
