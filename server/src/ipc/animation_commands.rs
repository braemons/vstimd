//! Animation commands and the proto <-> Animation mapping in both directions.

use super::response::{err, ok_ack, ok_body, ok_handle};
use super::vtl_commands::{resolve_output_handle, resolve_vtl_handle, vtl_bit_to_proto};
use crate::proto;
use crate::scene::animation::{
    AnimState, Animation, AnimationEntry, CancelAction, FinalAction, StartAction, VtlEdge,
    VtlPolarity,
};
use crate::scene::{SceneState, VtlBit};
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
                match resolve_output_handle(cmd.start_action_trigger_line.as_ref(), vtl_names) {
                    Ok(bit) => Some(bit),
                    Err(e) => return *e,
                }
            } else {
                None
            };

        let final_action = FinalAction::from_bits_truncate(cmd.final_action_mask as u16);

        let final_action_trigger_line =
            if final_action.contains(FinalAction::FINAL_ACTION_TRIGGER_LINE) {
                match resolve_output_handle(cmd.final_action_trigger_line.as_ref(), vtl_names) {
                    Ok(bit) => Some(bit),
                    Err(e) => return *e,
                }
            } else {
                None
            };

        let final_action_level_line = if final_action.contains(FinalAction::DONE_LEVEL) {
            match resolve_output_handle(cmd.final_action_level_line.as_ref(), vtl_names) {
                Ok(bit) => Some(bit),
                Err(e) => return *e,
            }
        } else {
            None
        };

        let start_trigger = if cmd.start_trigger.is_some() {
            match resolve_vtl_handle(cmd.start_trigger.as_ref(), vtl_names) {
                Ok(bit) => Some((bit, proto_vtl_edge(cmd.start_edge))),
                Err(e) => return *e,
            }
        } else {
            None
        };

        let cancel_trigger = if cmd.cancel_trigger.is_some() {
            match resolve_vtl_handle(cmd.cancel_trigger.as_ref(), vtl_names) {
                Ok(bit) => Some((bit, proto_vtl_edge(cmd.cancel_edge))),
                Err(e) => return *e,
            }
        } else {
            None
        };

        let cancel_action = CancelAction::from_bits_truncate(cmd.cancel_action_mask as u8);

        let cancel_action_trigger_line =
            if cancel_action.contains(CancelAction::CANCEL_ACTION_TRIGGER_LINE) {
                match resolve_output_handle(cmd.cancel_action_trigger_line.as_ref(), vtl_names) {
                    Ok(bit) => Some(bit),
                    Err(e) => return *e,
                }
            } else {
                None
            };

        let animation = match proto_to_animation(&cmd, vtl_names) {
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
            Some((bit, edge)) => (Some(vtl_bit_to_proto(bit)), edge_to_proto(edge)),
            None => (None, 0),
        };

        let (cancel_trigger, cancel_edge) = match entry.cancel_trigger {
            Some((bit, edge)) => (Some(vtl_bit_to_proto(bit)), edge_to_proto(edge)),
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
            body: Some(animation_to_proto_body(&entry.animation)),
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

fn edge_to_proto(e: VtlEdge) -> i32 {
    match e {
        VtlEdge::Rising => proto::VtlEdge::Rising as i32,
        VtlEdge::Falling => proto::VtlEdge::Falling as i32,
    }
}

fn polarity_to_proto(p: VtlPolarity) -> i32 {
    match p {
        VtlPolarity::ActiveHigh => proto::VtlPolarity::ActiveHigh as i32,
        VtlPolarity::ActiveLow => proto::VtlPolarity::ActiveLow as i32,
    }
}

fn animation_to_proto_body(anim: &Animation) -> proto::create_animation_request::Body {
    use proto::create_animation_request::Body as PBody;
    match anim {
        Animation::CoupleVisibilityToTriggerLine { trigger, polarity } => {
            PBody::CoupleVisibilityToTriggerLine(proto::CoupleVisibilityToTriggerLine {
                trigger: Some(vtl_bit_to_proto(*trigger)),
                polarity: polarity_to_proto(*polarity),
            })
        }
        Animation::EnableOnTriggerEdge {
            trigger,
            edge,
            enabled,
        } => PBody::EnableOnTriggerEdge(proto::EnableOnTriggerEdge {
            trigger: Some(vtl_bit_to_proto(*trigger)),
            edge: edge_to_proto(*edge),
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

fn proto_vtl_edge(e: i32) -> VtlEdge {
    match proto::VtlEdge::try_from(e).unwrap_or(proto::VtlEdge::Rising) {
        proto::VtlEdge::Rising => VtlEdge::Rising,
        proto::VtlEdge::Falling => VtlEdge::Falling,
    }
}

fn proto_vtl_polarity(p: i32) -> VtlPolarity {
    match proto::VtlPolarity::try_from(p).unwrap_or(proto::VtlPolarity::ActiveHigh) {
        proto::VtlPolarity::ActiveHigh => VtlPolarity::ActiveHigh,
        proto::VtlPolarity::ActiveLow => VtlPolarity::ActiveLow,
    }
}

// ── Animation proto → Rust mapping ───────────────────────────────────────────

fn proto_to_animation(
    cmd: &proto::CreateAnimationRequest,
    vtl_names: &[VtlNameEntry],
) -> Result<Animation, Box<proto::Response>> {
    use proto::create_animation_request::Body as PBody;

    let vtl_bit =
        |h: Option<&proto::VirtualTriggerLineHandle>| -> Result<VtlBit, Box<proto::Response>> {
            resolve_vtl_handle(h, vtl_names)
        };

    let proto_edge = |e: i32| -> VtlEdge { proto_vtl_edge(e) };

    match cmd.body.as_ref() {
        Some(PBody::CoupleVisibilityToTriggerLine(c)) => {
            Ok(Animation::CoupleVisibilityToTriggerLine {
                trigger: vtl_bit(c.trigger.as_ref())?,
                polarity: proto_vtl_polarity(c.polarity),
            })
        }
        Some(PBody::EnableOnTriggerEdge(c)) => Ok(Animation::EnableOnTriggerEdge {
            trigger: vtl_bit(c.trigger.as_ref())?,
            edge: proto_edge(c.edge),
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
