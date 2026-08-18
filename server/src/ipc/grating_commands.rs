//! Create/modify commands for the grating stimulus.

use super::convert::{
    grating_params_from_proto, placement_from_proto, mask_from_proto, waveform_from_proto,
    identity_from_proto,
};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::grating::Grating;
use crate::scene::stimulus::{Stimulus, StimulusBody, StimulusSceneEntry, StimulusType};

impl SceneState {
    /// Run `f` on the grating at `handle`, then mark it dirty unless the write
    /// was deferred.
    ///
    /// `dirty` lives on [`Stimulus`], above the kind, so the grating setters no
    /// longer write it themselves — the caller holding the whole stimulus does.
    /// Centralising that here also collapses eleven identical handle-lookup and
    /// wrong-type bodies into one.
    fn with_grating(
        &mut self,
        handle: u32,
        cmd: &str,
        f: impl FnOnce(&mut Grating, bool),
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let StimulusBody::Grating(g) = &mut entry.stimulus.body else {
            return err_wrong_type(&entry.stimulus, cmd, StimulusType::Grating);
        };
        f(g, deferred);
        if !deferred {
            entry.stimulus.flags_mut().mark_dirty();
        }
        ok_ack()
    }
    // ── CreateGrating ────────────────────────────────────────────────────────

    pub(super) fn cmd_create_grating(&mut self, cmd: proto::CreateGratingRequest) -> proto::Response {
        let params = cmd.params.unwrap_or_default();
        let width_px = if params.width_px == 0.0 { 200.0 } else { params.width_px };
        let height_px = if params.height_px == 0.0 { 200.0 } else { params.height_px };
        // The rotation is the stripe orientation — see CreateGratingRequest.placement.
        let (pos_px, angle_deg) = placement_from_proto(cmd.placement);
        let grating_params = grating_params_from_proto(&params);
        let identity = identity_from_proto(cmd.identity);
        let id = identity.id;
        let handle = self.alloc_stim_handle();
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(
                identity,
                Stimulus::from(Grating::new(pos_px, angle_deg, [width_px, height_px], grating_params)),
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
        self.with_grating(handle, "SetGratingPhase", |s, deferred| {
            s.set_phase(deferred, cmd.phase_cycles);
        })
    }

    pub(super) fn cmd_set_grating_sf(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingSfRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingSf", |s, deferred| {
            s.set_sf(deferred, cmd.sf_cycles_per_px);
        })
    }

    pub(super) fn cmd_set_grating_contrast(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingContrastRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingContrast", |s, deferred| {
            s.set_contrast(deferred, cmd.contrast);
        })
    }

    pub(super) fn cmd_set_grating_waveform(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingWaveformRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingWaveform", |s, deferred| {
            s.set_waveform(deferred, waveform_from_proto(cmd.waveform));
        })
    }

    pub(super) fn cmd_set_grating_mask(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingMaskRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingMask", |s, deferred| {
            s.set_mask(deferred, mask_from_proto(cmd.mask));
        })
    }

    pub(super) fn cmd_set_grating_drift_speed(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingDriftSpeedRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingDriftSpeed", |s, deferred| {
            s.set_drift_speed(deferred, cmd.speed_hz);
        })
    }

    pub(super) fn cmd_set_grating_drift_decoupled(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingDriftDecoupledRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingDriftDecoupled", |s, deferred| {
            s.set_drift_decoupled(deferred, cmd.decoupled);
        })
    }

    pub(super) fn cmd_set_grating_drift_angle(
        &mut self,
        handle: u32,
        cmd: proto::SetGratingDriftAngleRequest,
    ) -> proto::Response {
        self.with_grating(handle, "SetGratingDriftAngle", |s, deferred| {
            s.set_drift_angle(deferred, cmd.drift_angle_deg);
        })
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
        self.with_grating(handle, "SetGratingForeColor", |s, deferred| {
            s.set_fore_color(deferred, c);
        })
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
        self.with_grating(handle, "SetGratingBackColor", |s, deferred| {
            s.set_back_color(deferred, c);
        })
    }
}
