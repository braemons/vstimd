//! Animation commands. The proto <-> Animation mapping they read and write lives
//! in `convert::animation`, with every other conversion.

use super::convert::{
    animation_body_to_proto, animation_from_proto, output_vtl_bit_from_proto, vtl_bit_from_proto,
    vtl_bit_to_proto, vtl_edge_from_proto, vtl_edge_to_proto,
};
use super::response::{err, ok_ack, ok_body, ok_handle};
use crate::proto;
use crate::scene::animation::{
    AnimState, AnimationEntry, CancelAction, FinalAction, StartAction,
};
use crate::scene::SceneState;
use crate::vtl_state::{VtlNameEntry, VtlState};

impl SceneState {
    // ── Animation commands ────────────────────────────────────────────────────

    pub(super) fn cmd_create_animation(
        &mut self,
        cmd: proto::CreateAnimationRequest,
        vtl: Option<&VtlState>,
    ) -> proto::Response {
        let vtl_names: &[VtlNameEntry] = vtl.map_or(&[], |v| v.names.as_slice());
        let start_action = StartAction::from_bits_truncate(cmd.start_action_mask as u8);

        let start_action_trigger_line =
            if start_action.contains(StartAction::START_ACTION_TRIGGER_LINE) {
                match output_vtl_bit_from_proto(cmd.start_action_trigger_line.as_ref(), vtl_names) {
                    Ok(bit) => Some(bit),
                    Err(e) => return *e,
                }
            } else {
                None
            };

        let final_action = FinalAction::from_bits_truncate(cmd.final_action_mask as u16);

        let final_action_trigger_line =
            if final_action.contains(FinalAction::FINAL_ACTION_TRIGGER_LINE) {
                match output_vtl_bit_from_proto(cmd.final_action_trigger_line.as_ref(), vtl_names) {
                    Ok(bit) => Some(bit),
                    Err(e) => return *e,
                }
            } else {
                None
            };

        let final_action_level_line = if final_action.contains(FinalAction::DONE_LEVEL) {
            match output_vtl_bit_from_proto(cmd.final_action_level_line.as_ref(), vtl_names) {
                Ok(bit) => Some(bit),
                Err(e) => return *e,
            }
        } else {
            None
        };

        let start_trigger = if cmd.start_trigger.is_some() {
            match vtl_bit_from_proto(cmd.start_trigger.as_ref(), vtl_names) {
                Ok(bit) => Some((bit, vtl_edge_from_proto(cmd.start_edge))),
                Err(e) => return *e,
            }
        } else {
            None
        };

        let cancel_trigger = if cmd.cancel_trigger.is_some() {
            match vtl_bit_from_proto(cmd.cancel_trigger.as_ref(), vtl_names) {
                Ok(bit) => Some((bit, vtl_edge_from_proto(cmd.cancel_edge))),
                Err(e) => return *e,
            }
        } else {
            None
        };

        let cancel_action = CancelAction::from_bits_truncate(cmd.cancel_action_mask as u8);

        let cancel_action_trigger_line =
            if cancel_action.contains(CancelAction::CANCEL_ACTION_TRIGGER_LINE) {
                match output_vtl_bit_from_proto(cmd.cancel_action_trigger_line.as_ref(), vtl_names) {
                    Ok(bit) => Some(bit),
                    Err(e) => return *e,
                }
            } else {
                None
            };

        let animation = match animation_from_proto(&cmd, vtl_names) {
            Ok(a) => a,
            Err(e) => return *e,
        };

        let handle = self.alloc_anim_handle();
        self.config.animations.insert(
            handle,
            AnimationEntry {
                config: crate::scene::animation::AnimationConfig {
                    name: cmd.name,
                    state: AnimState::Idle,
                    target: crate::scene::animation::AnimationTarget::Stimuli {
                        handles: cmd
                            .target
                            .and_then(|t| t.target)
                            .map(|proto::animation_target::Target::Stimuli(s)| s.handles)
                            .unwrap_or_default(),
                    },
                    start_action,
                    start_action_trigger_line,
                    final_action,
                    final_action_trigger_line,
                    final_action_level_line,
                    start_trigger,
                    cancel_trigger,
                    cancel_action,
                    cancel_action_trigger_line,
                    animation,
                },
                captured_user_enabled: None,
            },
        );
        ok_handle(handle)
    }

    pub(super) fn cmd_arm_animation(&mut self, cmd: proto::ArmAnimationRequest) -> proto::Response {
        if self.arm_animation(cmd.handle) {
            ok_ack()
        } else {
            err(
                proto::ErrorCode::HandleNotFound,
                format!("animation handle {} not found", cmd.handle),
            )
        }
    }

    pub(super) fn cmd_disarm_animation(&mut self, cmd: proto::DisarmAnimationRequest) -> proto::Response {
        if self.disarm_animation(cmd.handle) {
            ok_ack()
        } else {
            err(
                proto::ErrorCode::HandleNotFound,
                format!("animation handle {} not found", cmd.handle),
            )
        }
    }

    pub(super) fn cmd_cancel_animation(
        &mut self,
        cmd: proto::CancelAnimationRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        // Seed a scratch level buffer from the current staged outputs so any
        // cancel_action level change from the teardown is applied, and commit
        // changed banks straight through to shm — outside the render loop there
        // is no per-frame commit. A pulse goes into `VtlState::pulses`, which
        // the next commit publishes for its one frame.
        let mut levels = vtl.as_ref().map_or([0u64; vtl::MAX_BANKS], |v| v.staged);
        let mut pulses = [0u64; vtl::MAX_BANKS];
        let found = self.cancel_animation(
            cmd.handle,
            &mut crate::vtl_state::VtlOutputs { levels: &mut levels, pulses: &mut pulses },
        );
        if let Some(v) = vtl {
            for (bank, &p) in pulses.iter().enumerate() {
                v.pulses[bank] |= p;
            }
            for (bank, &val) in levels.iter().enumerate() {
                if v.staged[bank] != val {
                    v.set_staged_bank(bank, val);
                }
            }
        }
        if found {
            ok_ack()
        } else {
            err(
                proto::ErrorCode::HandleNotFound,
                format!("animation handle {} not found", cmd.handle),
            )
        }
    }

    pub(super) fn cmd_delete_animation(&mut self, cmd: proto::DeleteAnimationRequest) -> proto::Response {
        if self.delete_animation(cmd.handle) {
            ok_ack()
        } else {
            err(
                proto::ErrorCode::HandleNotFound,
                format!("animation handle {} not found", cmd.handle),
            )
        }
    }

    pub(super) fn cmd_list_animations(&self) -> proto::Response {
        let animations: Vec<proto::AnimationInfo> = self
            .config
            .animations
            .iter()
            .map(|(&handle, entry)| {
                let state = match entry.state {
                    AnimState::Idle => proto::AnimationState::Idle as i32,
                    AnimState::Armed => proto::AnimationState::Armed as i32,
                    AnimState::Running { .. } => proto::AnimationState::Running as i32,
                    AnimState::Done => proto::AnimationState::Done as i32,
                };
                proto::AnimationInfo {
                    handle,
                    name: entry.name.clone(),
                    state,
                    type_name: entry.animation.type_name().to_string(),
                }
            })
            .collect();
        ok_body(proto::response::Body::AnimationList(
            proto::ListAnimationsResponse { animations },
        ))
    }

    pub(super) fn cmd_query_animation(&self, cmd: proto::QueryAnimationRequest) -> proto::Response {
        let entry = match self.config.animations.get(&cmd.handle) {
            Some(e) => e,
            None => {
                return err(
                    proto::ErrorCode::HandleNotFound,
                    format!("animation handle {} not found", cmd.handle),
                );
            }
        };

        let state = match entry.state {
            AnimState::Idle => proto::AnimationState::Idle as i32,
            AnimState::Armed => proto::AnimationState::Armed as i32,
            AnimState::Running { .. } => proto::AnimationState::Running as i32,
            AnimState::Done => proto::AnimationState::Done as i32,
        };

        let (start_trigger, start_edge) = match entry.start_trigger {
            Some((bit, edge)) => (Some(vtl_bit_to_proto(bit)), vtl_edge_to_proto(edge)),
            None => (None, 0),
        };

        let (cancel_trigger, cancel_edge) = match entry.cancel_trigger {
            Some((bit, edge)) => (Some(vtl_bit_to_proto(bit)), vtl_edge_to_proto(edge)),
            None => (None, 0),
        };

        let params = proto::CreateAnimationRequest {
            name: entry.name.clone(),
            start_action_mask: entry.start_action.bits() as u32,
            start_action_trigger_line: entry.start_action_trigger_line.map(vtl_bit_to_proto),
            final_action_mask: entry.final_action.bits() as u32,
            final_action_trigger_line: entry.final_action_trigger_line.map(vtl_bit_to_proto),
            final_action_level_line: entry.final_action_level_line.map(vtl_bit_to_proto),
            start_trigger,
            start_edge,
            cancel_trigger,
            cancel_edge,
            cancel_action_mask: entry.cancel_action.bits() as u32,
            cancel_action_trigger_line: entry.cancel_action_trigger_line.map(vtl_bit_to_proto),
            target: Some(proto::AnimationTarget {
                target: Some(proto::animation_target::Target::Stimuli(
                    proto::AnimationStimuli { handles: entry.target.stimuli().to_vec() },
                )),
            }),
            body: Some(animation_body_to_proto(&entry.animation)),
        };

        ok_body(proto::response::Body::QueryAnimationResponse(
            proto::QueryAnimationResponse {
                handle: cmd.handle,
                state,
                params: Some(params),
                type_name: entry.animation.type_name().to_string(),
            },
        ))
    }
}
