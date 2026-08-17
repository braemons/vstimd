//! Create/modify commands for the grating stimulus.

use super::convert::{nonempty, parse_or_new_uuid};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::grating::{
    GratingStimulus, grating_params_from_proto, proto_to_mask, proto_to_waveform,
};
use crate::scene::stimulus::{Stimulus, StimulusSceneEntry};

impl SceneState {
    // ── CreateGrating ────────────────────────────────────────────────────────

    pub(super) fn cmd_create_grating(&mut self, cmd: proto::CreateGratingRequest) -> proto::Response {
        // Borrow cmd fully before any partial moves.
        let id = match parse_or_new_uuid(&cmd.id) {
            Ok(id) => id,
            Err(resp) => return *resp,
        };
        let params = grating_params_from_proto(&cmd);
        let center = cmd.center.unwrap_or_default();
        let width = if cmd.width == 0.0 { 200.0 } else { cmd.width };
        let height = if cmd.height == 0.0 { 200.0 } else { cmd.height };
        let angle = cmd.angle;
        let name = nonempty(cmd.name);
        let handle = self.alloc_stim_handle();
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(
                id,
                name,
                Stimulus::Grating(GratingStimulus::new(
                    [center.x, center.y],
                    angle,
                    [width, height],
                    params,
                )),
            ),
        );
        ok_handle_with_id(handle, &id)
    }

    // ── Grating setters ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_grating_phase(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingPhaseRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_phase(self.runtime.deferred_mode, cmd.phase);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingPhase", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_sf(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingSfRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_sf(self.runtime.deferred_mode, cmd.sf);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingSf", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_contrast(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingContrastRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_contrast(self.runtime.deferred_mode, cmd.contrast);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingContrast", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_waveform(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingWaveformRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_waveform(self.runtime.deferred_mode, proto_to_waveform(cmd.waveform));
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingWaveform", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_mask(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingMaskRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_mask(self.runtime.deferred_mode, proto_to_mask(cmd.mask));
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingMask", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_drift_speed(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingDriftSpeedRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_drift_speed(self.runtime.deferred_mode, cmd.speed);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingDriftSpeed", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_drift_decoupled(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingDriftDecoupledRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_drift_decoupled(self.runtime.deferred_mode, cmd.decoupled);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingDriftDecoupled", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_drift_angle(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingDriftAngleRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_drift_angle(self.runtime.deferred_mode, cmd.angle_deg);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingDriftAngle", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_fore_color(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingForeColorRequest,
    ) -> proto::Response {
        let c = match cmd.fore_color {
            Some(c) => c.into(),
            None => return err(proto::ErrorCode::InvalidArgument, "fore_color must be set"),
        };
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_fore_color(self.runtime.deferred_mode, c);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingForeColor", "Grating"),
            },
        }
    }

    pub(super) fn cmd_set_grating_back_color(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingBackColorRequest,
    ) -> proto::Response {
        let c = match cmd.back_color {
            Some(c) => c.into(),
            None => return err(proto::ErrorCode::InvalidArgument, "back_color must be set"),
        };
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Grating(s) => {
                    s.set_back_color(self.runtime.deferred_mode, c);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetGratingBackColor", "Grating"),
            },
        }
    }
}
