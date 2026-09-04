//! Entry point for every IPC command: `handle_request` and the two
//! sub-dispatchers it fans out to, plus the one-line request summary the
//! command log records.

use super::response::{err, ok_ack};
use crate::proto;
use crate::proto::request;
use crate::scene::SceneState;
use crate::vtl_state::VtlState;

// ── Request summary for the command log ───────────────────────────────────────

/// Centre out of a create request's placement, defaulting to the origin the way
/// the create commands themselves do.
fn placement_pos(placement: Option<&proto::Transform2D>) -> (f32, f32) {
    let pos_px = placement.and_then(|t| t.pos_px.as_ref());
    (pos_px.map_or(0.0, |p| p.x), pos_px.map_or(0.0, |p| p.y))
}

fn command_summary(req: &proto::Request) -> String {
    match &req.body {
        Some(request::Body::CreateRect(c)) => {
            let p = c.params.as_ref();
            format!(
                "CreateRect {:.0}×{:.0}",
                p.map_or(0.0, |p| p.width_px),
                p.map_or(0.0, |p| p.height_px),
            )
        }
        Some(request::Body::CreateCircle(c)) => {
            format!("CreateCircle d={:.0}", c.params.as_ref().map_or(0.0, |p| p.diameter_px))
        }
        Some(request::Body::CreateEllipse(c)) => {
            let p = c.params.as_ref();
            format!(
                "CreateEllipse {:.0}×{:.0}",
                p.map_or(0.0, |p| p.width_px),
                p.map_or(0.0, |p| p.height_px),
            )
        }
        Some(request::Body::SetEnabled(c)) => {
            format!("SetEnabled({})", if c.enabled { "on" } else { "off" })
        }
        Some(request::Body::Delete(_)) => "Delete".into(),
        Some(request::Body::SetPosition(c)) => format!("SetPosition({:.1},{:.1})", c.x_px, c.y_px),
        Some(request::Body::SetRotation(c)) => format!("SetRotation({:.1}°)", c.rotation_deg),
        Some(request::Body::SetFillColor(_)) => "SetFillColor".into(),
        Some(request::Body::SetAlpha(c)) => format!("SetAlpha({:.2})", c.opacity),
        Some(request::Body::SetRectSize(c)) => {
            format!("SetRectSize {:.0}×{:.0}", c.width_px, c.height_px)
        }
        Some(request::Body::SetCircleDiameter(c)) => {
            format!("SetCircleDiameter({:.0})", c.diameter_px)
        }
        Some(request::Body::SetEllipseSize(c)) => {
            format!("SetEllipseSize {:.0}×{:.0}", c.width_px, c.height_px)
        }
        Some(request::Body::SetDrawMode(_)) => "SetDrawMode".into(),
        Some(request::Body::SetOutlineColor(_)) => "SetOutlineColor".into(),
        Some(request::Body::SetOutlineWidth(c)) => format!("SetOutlineWidth({:.1})", c.line_width_px),
        Some(request::Body::SetBackground(_)) => "SetBackground".into(),
        Some(request::Body::SetDeferredMode(c)) => {
            if c.cancel {
                "SetDeferredMode(cancel)".into()
            } else if c.active {
                "SetDeferredMode(begin)".into()
            } else {
                "SetDeferredMode(end/flip)".into()
            }
        }
        Some(request::Body::ClearStimuli(_)) => "ClearStimuli".into(),
        Some(request::Body::ClearAnimations(_)) => "ClearAnimations".into(),
        Some(request::Body::ClearAll(_)) => "ClearAll".into(),
        Some(request::Body::SetAllEnabled(c)) => {
            format!("SetAllEnabled({})", if c.enabled { "on" } else { "off" })
        }
        Some(request::Body::CreateGrating(c)) => {
            let p = c.params.as_ref();
            let (x, y) = placement_pos(c.placement.as_ref());
            format!(
                "CreateGrating {:.0}×{:.0} sf_cycles_per_px={:.4} pos_px=({x:.1},{y:.1})",
                p.map_or(0.0, |p| p.width_px),
                p.map_or(0.0, |p| p.height_px),
                p.map_or(0.0, |p| p.sf_cycles_per_px),
            )
        }
        Some(request::Body::CreateText(c)) => {
            let (x, y) = placement_pos(c.placement.as_ref());
            format!(
                "CreateText {:?} pos_px=({x:.1},{y:.1})",
                c.params.as_ref().map_or("", |p| p.text.as_str()),
            )
        }
        Some(request::Body::CreateDots(c)) => {
            let p = c.params.as_ref();
            let (x, y) = placement_pos(c.placement.as_ref());
            format!(
                "CreateDots n={} coh={:.2} dir={:.1}° pos_px=({x:.1},{y:.1})",
                p.map_or(0, |p| p.dot_count),
                p.and_then(|p| p.coherence).unwrap_or(1.0),
                p.map_or(0.0, |p| p.direction_deg),
            )
        }
        Some(request::Body::SetDotsDirection(c)) => {
            format!("SetDotsDirection({:.1}°)", c.direction_deg)
        }
        Some(request::Body::SetDotsSpeed(c)) => {
            format!("SetDotsSpeed({:.1} px/s)", c.speed_px_per_s)
        }
        Some(request::Body::SetDotsCoherence(c)) => format!("SetDotsCoherence({:.2})", c.coherence),
        Some(request::Body::SetDotsCount(c)) => format!("SetDotsCount({})", c.dot_count),
        Some(request::Body::SetDotsSize(c)) => format!("SetDotsSize({:.1})", c.dot_size_px),
        Some(request::Body::SetDotsColor(_)) => "SetDotsColor".into(),
        Some(request::Body::SetDotsAperture(_)) => "SetDotsAperture".into(),
        Some(request::Body::SetDotsFieldSize(c)) => {
            format!("SetDotsFieldSize({:.0}×{:.0})", c.width_px, c.height_px)
        }
        Some(request::Body::SetDotsLifetime(c)) => {
            format!("SetDotsLifetime({} frames)", c.dot_lifetime_frames)
        }
        Some(request::Body::SetDotsSeed(c)) => format!("SetDotsSeed({})", c.seed),
        Some(request::Body::SetText(c)) => format!("SetText({:?})", c.text),
        Some(request::Body::SetTextColor(_)) => "SetTextColor".into(),
        Some(request::Body::SetGratingPhase(c)) => format!("SetGratingPhase({:.3})", c.phase_cycles),
        Some(request::Body::SetGratingSf(c)) => format!("SetGratingSf({:.4})", c.sf_cycles_per_px),
        Some(request::Body::SetGratingContrast(c)) => {
            format!("SetGratingContrast({:.2})", c.contrast)
        }
        Some(request::Body::SetGratingWaveform(_)) => "SetGratingWaveform".into(),
        Some(request::Body::SetGratingMask(_)) => "SetGratingMask".into(),
        Some(request::Body::SetGratingDriftSpeed(c)) => {
            format!("SetGratingDriftSpeed({:.3})", c.speed_hz)
        }
        Some(request::Body::SetGratingDriftDecoupled(c)) => {
            format!("SetGratingDriftDecoupled({})", c.decoupled)
        }
        Some(request::Body::SetGratingDriftAngle(c)) => {
            format!("SetGratingDriftAngle({:.1}°)", c.drift_angle_deg)
        }
        Some(request::Body::SetGratingForeColor(_)) => "SetGratingForeColor".into(),
        Some(request::Body::SetGratingBackColor(_)) => "SetGratingBackColor".into(),
        Some(request::Body::QueryServerInfo(_)) => "QueryServerInfo".into(),
        Some(request::Body::QueryStimulus(_)) => "QueryStimulus".into(),
        Some(request::Body::ListStimuli(_)) => "ListStimuli".into(),
        Some(request::Body::SetName(c)) => format!("SetName({:?})", c.name),
        Some(request::Body::CreatePolygon(_)) => "CreatePolygon".into(),
        Some(request::Body::SetPolygonVertices(_)) => "SetPolygonVertices".into(),
        Some(request::Body::SetVirtualTriggerLineName(c)) => {
            format!("SetVirtualTriggerLineName({:?})", c.name)
        }
        Some(request::Body::ListVirtualTriggerLines(_)) => "ListVirtualTriggerLines".into(),
        Some(request::Body::SetVirtualTriggerLine(c)) => {
            format!("SetVirtualTriggerLine(val={})", c.value)
        }
        Some(request::Body::ToggleVirtualTriggerLine(_)) => "ToggleVirtualTriggerLine".into(),
        Some(request::Body::ClearVirtualTriggerLineLatches(_)) => {
            "ClearVirtualTriggerLineLatches".into()
        }
        Some(request::Body::SetVirtualTriggerLineBank(c)) => format!(
            "SetVirtualTriggerLineBank(dir={} bank={} val={:#018x})",
            c.kind, c.bank, c.value
        ),
        Some(request::Body::BringToFront(_)) => "BringToFront".into(),
        Some(request::Body::SendToBack(_)) => "SendToBack".into(),
        Some(request::Body::SwapDrawOrder(c)) => {
            format!("SwapDrawOrder({}, {})", c.handle_a, c.handle_b)
        }
        Some(request::Body::CreateAnimation(c)) => format!("CreateAnimation({:?})", c.name),
        Some(request::Body::ArmAnimation(c)) => format!("ArmAnimation({})", c.handle),
        Some(request::Body::DisarmAnimation(c)) => format!("DisarmAnimation({})", c.handle),
        Some(request::Body::CancelAnimation(c)) => format!("CancelAnimation({})", c.handle),
        Some(request::Body::DeleteAnimation(c)) => format!("DeleteAnimation({})", c.handle),
        Some(request::Body::ListAnimations(_)) => "ListAnimations".into(),
        Some(request::Body::QueryAnimation(c)) => format!("QueryAnimation({})", c.handle),
        Some(request::Body::WaitForFrames(c)) => format!("WaitForFrames({})", c.count),
        Some(request::Body::WaitUntil(c)) => format!("WaitUntil({}ns)", c.server_time_ns),
        Some(request::Body::ListSceneConfigs(_)) => "ListSceneConfigs".into(),
        Some(request::Body::LoadSceneConfig(c)) => format!("LoadSceneConfig({:?})", c.name),
        Some(request::Body::UploadSceneConfig(c)) => format!("UploadSceneConfig({:?})", c.name),
        Some(request::Body::RetrieveSceneConfig(_)) => "RetrieveSceneConfig".into(),
        Some(request::Body::SetCondition(c)) => match &c.condition {
            Some(proto::set_condition_request::Condition::Index(i)) => {
                format!("SetCondition({i})")
            }
            Some(proto::set_condition_request::Condition::Name(n)) => {
                format!("SetCondition({n:?})")
            }
            None => "SetCondition(?)".into(),
        },
        Some(request::Body::DeclareConditions(c)) => {
            format!("DeclareConditions({})", c.conditions.len())
        }
        Some(request::Body::ListConditions(_)) => "ListConditions".into(),
        Some(request::Body::SetStimulusConditions(c)) => {
            format!("SetStimulusConditions({:?})", c.condition_indices)
        }
        Some(request::Body::SetAnimationConditions(c)) => {
            format!("SetAnimationConditions({}, {:?})", c.handle, c.condition_indices)
        }
        Some(request::Body::Shutdown(_)) => "Shutdown".into(),
        None => "?".into(),
    }
}

impl SceneState {
    pub fn handle_request(
        &mut self,
        req: proto::Request,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        let log_handle = match &req.target {
            Some(request::Target::Stimulus(h)) => *h,
            _ => 0,
        };
        let log_summary = command_summary(&req);

        let response = match req.body {
            None => err(proto::ErrorCode::InvalidArgument, "empty request body"),
            Some(body) => match req.target {
                Some(request::Target::System(_)) | None => self.handle_system_command(body, vtl),
                Some(request::Target::Stimulus(handle)) => {
                    self.handle_stimulus_command(handle, body)
                }
            },
        };

        self.push_command_log(
            log_handle,
            log_summary.clone(),
            response.code == proto::ErrorCode::Ok as i32,
            response.handle,
        );

        if response.code == proto::ErrorCode::Ok as i32 {
            if log_handle == 0 {
                log::debug!("ipc: {} → handle {}", log_summary, response.handle);
            } else {
                log::debug!("ipc: [{}] {}", log_handle, log_summary);
            }
        } else {
            log::warn!(
                "ipc: [{}] {} → error {}: {}",
                log_handle,
                log_summary,
                response.code,
                response.error
            );
        }

        response
    }

    // ── System command dispatcher ─────────────────────────────────────────────

    fn handle_system_command(
        &mut self,
        body: request::Body,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        match body {
            request::Body::CreateRect(cmd) => self.cmd_create_rect(cmd),
            request::Body::CreateCircle(cmd) => self.cmd_create_circle(cmd),
            request::Body::CreateEllipse(cmd) => self.cmd_create_ellipse(cmd),
            request::Body::CreateGrating(cmd) => self.cmd_create_grating(cmd),
            request::Body::CreateText(cmd) => self.cmd_create_text(cmd),
            request::Body::CreateDots(cmd) => self.cmd_create_dots(cmd),
            request::Body::CreatePolygon(_) => err(
                proto::ErrorCode::NotSupported,
                "CreatePolygon is not yet implemented",
            ),
            request::Body::SetBackground(cmd) => self.cmd_set_background(cmd),
            request::Body::SetDeferredMode(cmd) => self.cmd_set_deferred_mode(cmd),
            request::Body::ClearStimuli(_) => self.cmd_clear_stimuli(),
            request::Body::ClearAnimations(_) => self.cmd_clear_animations(),
            request::Body::ClearAll(_) => self.cmd_clear_all(),
            request::Body::SetAllEnabled(cmd) => self.cmd_set_all_enabled(cmd),
            request::Body::QueryServerInfo(_) => self.cmd_query_server_info(),
            request::Body::ListStimuli(_) => self.cmd_list_stimuli(),
            request::Body::SetVirtualTriggerLineName(cmd) => {
                self.cmd_set_virtual_trigger_line_name(cmd, vtl)
            }
            request::Body::ListVirtualTriggerLines(_) => {
                self.cmd_list_virtual_trigger_lines(vtl.as_deref())
            }
            request::Body::SetVirtualTriggerLine(cmd) => {
                self.cmd_set_virtual_trigger_line(cmd, vtl)
            }
            request::Body::ToggleVirtualTriggerLine(cmd) => {
                self.cmd_toggle_virtual_trigger_line(cmd, vtl)
            }
            request::Body::ClearVirtualTriggerLineLatches(cmd) => {
                self.cmd_clear_virtual_trigger_line_latches(cmd, vtl.as_deref())
            }
            request::Body::SetVirtualTriggerLineBank(cmd) => {
                self.cmd_set_virtual_trigger_line_bank(cmd, vtl)
            }
            request::Body::SwapDrawOrder(_) => err(
                proto::ErrorCode::NotSupported,
                "SwapDrawOrder not yet implemented",
            ),
            request::Body::CreateAnimation(cmd) => self.cmd_create_animation(cmd, vtl.as_deref()),
            request::Body::ArmAnimation(cmd) => self.cmd_arm_animation(cmd),
            request::Body::DisarmAnimation(cmd) => self.cmd_disarm_animation(cmd),
            request::Body::CancelAnimation(cmd) => self.cmd_cancel_animation(cmd, vtl),
            request::Body::DeleteAnimation(cmd) => self.cmd_delete_animation(cmd),
            request::Body::ListAnimations(_) => self.cmd_list_animations(),
            request::Body::QueryAnimation(cmd) => self.cmd_query_animation(cmd),
            request::Body::ListSceneConfigs(cmd) => self.cmd_list_scene_configs(cmd),
            request::Body::LoadSceneConfig(cmd) => self.cmd_load_scene_config(cmd, vtl),
            request::Body::UploadSceneConfig(cmd) => self.cmd_upload_scene_config(cmd, vtl),
            request::Body::RetrieveSceneConfig(_) => self.cmd_retrieve_scene_config(vtl.as_deref()),
            request::Body::SetCondition(cmd) => self.cmd_set_condition(cmd),
            request::Body::DeclareConditions(cmd) => self.cmd_declare_conditions(cmd),
            request::Body::ListConditions(_) => self.cmd_list_conditions(),
            request::Body::SetAnimationConditions(cmd) => self.cmd_set_animation_conditions(cmd),
            request::Body::Shutdown(_) => {
                crate::process::shutdown::request();
                ok_ack()
            }
            _ => err(
                proto::ErrorCode::WrongTarget,
                "command requires a stimulus handle (target.stimulus > 0)",
            ),
        }
    }

    // ── Stimulus command dispatcher ───────────────────────────────────────────

    fn handle_stimulus_command(&mut self, handle: u32, body: request::Body) -> proto::Response {
        match body {
            request::Body::CreateRect(_)
            | request::Body::CreateCircle(_)
            | request::Body::CreateEllipse(_)
            | request::Body::CreateGrating(_)
            | request::Body::CreateText(_)
            | request::Body::CreateDots(_)
            | request::Body::CreatePolygon(_)
            | request::Body::SetBackground(_)
            | request::Body::SetDeferredMode(_)
            | request::Body::ClearStimuli(_)
            | request::Body::ClearAnimations(_)
            | request::Body::ClearAll(_)
            | request::Body::SetAllEnabled(_)
            | request::Body::QueryServerInfo(_)
            | request::Body::ListStimuli(_)
            | request::Body::SetVirtualTriggerLineName(_)
            | request::Body::ListVirtualTriggerLines(_)
            | request::Body::SetVirtualTriggerLine(_)
            | request::Body::ToggleVirtualTriggerLine(_)
            | request::Body::ClearVirtualTriggerLineLatches(_)
            | request::Body::SetVirtualTriggerLineBank(_)
            | request::Body::SwapDrawOrder(_)
            | request::Body::CreateAnimation(_)
            | request::Body::ArmAnimation(_)
            | request::Body::DisarmAnimation(_)
            | request::Body::CancelAnimation(_)
            | request::Body::DeleteAnimation(_)
            | request::Body::ListAnimations(_)
            | request::Body::QueryAnimation(_)
            | request::Body::WaitForFrames(_)
            | request::Body::WaitUntil(_)
            | request::Body::ListSceneConfigs(_)
            | request::Body::LoadSceneConfig(_)
            | request::Body::UploadSceneConfig(_)
            | request::Body::RetrieveSceneConfig(_)
            | request::Body::SetCondition(_)
            | request::Body::DeclareConditions(_)
            | request::Body::ListConditions(_)
            | request::Body::SetAnimationConditions(_)
            | request::Body::Shutdown(_) => err(
                proto::ErrorCode::WrongTarget,
                "system command must use target.system (not a stimulus handle)",
            ),
            request::Body::SetEnabled(cmd) => self.cmd_set_enabled(handle, cmd),
            request::Body::Delete(_) => self.cmd_delete(handle),
            request::Body::SetName(cmd) => self.cmd_set_name(handle, cmd),
            request::Body::SetPosition(cmd) => self.cmd_set_position(handle, cmd),
            request::Body::SetRotation(cmd) => self.cmd_set_orientation(handle, cmd),
            request::Body::SetFillColor(cmd) => self.cmd_set_fill_color(handle, cmd),
            request::Body::SetAlpha(cmd) => self.cmd_set_alpha(handle, cmd),
            request::Body::SetRectSize(cmd) => self.cmd_set_rect_size(handle, cmd),
            request::Body::SetCircleDiameter(cmd) => self.cmd_set_circle_diameter(handle, cmd),
            request::Body::SetEllipseSize(cmd) => self.cmd_set_ellipse_size(handle, cmd),
            request::Body::SetDrawMode(cmd) => self.cmd_set_draw_mode(handle, cmd),
            request::Body::SetOutlineColor(cmd) => self.cmd_set_outline_color(handle, cmd),
            request::Body::SetOutlineWidth(cmd) => self.cmd_set_outline_width(handle, cmd),
            request::Body::SetGratingPhase(cmd) => self.cmd_set_grating_phase(handle, cmd),
            request::Body::SetGratingSf(cmd) => self.cmd_set_grating_sf(handle, cmd),
            request::Body::SetGratingContrast(cmd) => self.cmd_set_grating_contrast(handle, cmd),
            request::Body::SetGratingWaveform(cmd) => self.cmd_set_grating_waveform(handle, cmd),
            request::Body::SetGratingMask(cmd) => self.cmd_set_grating_mask(handle, cmd),
            request::Body::SetGratingDriftSpeed(cmd) => {
                self.cmd_set_grating_drift_speed(handle, cmd)
            }
            request::Body::SetGratingDriftDecoupled(cmd) => {
                self.cmd_set_grating_drift_decoupled(handle, cmd)
            }
            request::Body::SetGratingDriftAngle(cmd) => {
                self.cmd_set_grating_drift_angle(handle, cmd)
            }
            request::Body::SetGratingForeColor(cmd) => self.cmd_set_grating_fore_color(handle, cmd),
            request::Body::SetGratingBackColor(cmd) => self.cmd_set_grating_back_color(handle, cmd),
            request::Body::SetDotsDirection(cmd) => self.cmd_set_dots_direction(handle, cmd),
            request::Body::SetDotsSpeed(cmd) => self.cmd_set_dots_speed(handle, cmd),
            request::Body::SetDotsCoherence(cmd) => self.cmd_set_dots_coherence(handle, cmd),
            request::Body::SetDotsCount(cmd) => self.cmd_set_dots_count(handle, cmd),
            request::Body::SetDotsSize(cmd) => self.cmd_set_dots_size(handle, cmd),
            request::Body::SetDotsColor(cmd) => self.cmd_set_dots_color(handle, cmd),
            request::Body::SetDotsAperture(cmd) => self.cmd_set_dots_aperture(handle, cmd),
            request::Body::SetDotsFieldSize(cmd) => self.cmd_set_dots_field_size(handle, cmd),
            request::Body::SetDotsLifetime(cmd) => self.cmd_set_dots_lifetime(handle, cmd),
            request::Body::SetDotsSeed(cmd) => self.cmd_set_dots_seed(handle, cmd),
            request::Body::SetText(cmd) => self.cmd_set_text(handle, cmd),
            request::Body::SetTextColor(cmd) => self.cmd_set_text_color(handle, cmd),
            request::Body::SetPolygonVertices(_) => err(
                proto::ErrorCode::NotSupported,
                "SetPolygonVertices is not yet implemented",
            ),
            request::Body::BringToFront(_) => err(
                proto::ErrorCode::NotSupported,
                "BringToFront not yet implemented",
            ),
            request::Body::SendToBack(_) => err(
                proto::ErrorCode::NotSupported,
                "SendToBack not yet implemented",
            ),
            request::Body::SetStimulusConditions(cmd) => {
                self.cmd_set_stimulus_conditions(handle, cmd)
            }
            request::Body::QueryStimulus(_) => self.cmd_query_stimulus(handle),
        }
    }
}
