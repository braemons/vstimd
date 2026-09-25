//! Create/modify commands for the random dot kinematogram.

use super::convert::{
    aperture_from_proto, dots_params_from_proto, identity_from_proto, placement_from_proto,
};
use super::response::{err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::dots::Dots;
use crate::scene::stimulus::{Stimulus, StimulusBody, StimulusSceneEntry, StimulusType};

impl SceneState {
    /// Run `f` on the dot field at `handle`.
    ///
    /// Unlike the other bodies this does **not** mark the stimulus dirty. `dirty`
    /// means "the cached mesh is stale", and a dot field has no cached mesh: it
    /// draws one shared quad, instanced, with every parameter in the push constants
    /// and the positions rewritten every frame regardless. Marking it dirty would
    /// invalidate nothing.
    fn with_dots(
        &mut self,
        handle: u32,
        cmd: &str,
        f: impl FnOnce(&mut Dots, bool),
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let StimulusBody::Dots(d) = &mut entry.stimulus.body else {
            return err_wrong_type(&entry.stimulus, cmd, StimulusType::Dots);
        };
        f(d, deferred);
        ok_ack()
    }

    // ── CreateDots ────────────────────────────────────────────────────────────

    pub(super) fn cmd_create_dots(&mut self, cmd: proto::CreateDotsRequest) -> proto::Response {
        let params = dots_params_from_proto(&cmd.params.unwrap_or_default());
        // `rotation_deg` is ignored: a dot field has no orientation of its own, and
        // the direction of motion is `direction_deg`.
        let (pos_px, _rotation_deg) = placement_from_proto(cmd.placement);
        let identity = identity_from_proto(cmd.identity);
        let id = identity.id;
        let handle = self.alloc_stim_handle();
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(identity, Stimulus::from(Dots::new(pos_px, 0.0, params))),
        );
        ok_handle_with_id(handle, &id)
    }

    // ── Dots setters ──────────────────────────────────────────────────────────

    pub(super) fn cmd_set_dots_direction(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsDirectionRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsDirection", |d, deferred| {
            d.set_direction(deferred, cmd.direction_deg);
        })
    }

    pub(super) fn cmd_set_dots_speed(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsSpeedRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsSpeed", |d, deferred| {
            d.set_speed(deferred, cmd.speed_px_per_s);
        })
    }

    pub(super) fn cmd_set_dots_coherence(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsCoherenceRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsCoherence", |d, deferred| {
            d.set_coherence(deferred, cmd.coherence);
        })
    }

    pub(super) fn cmd_set_dots_count(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsCountRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsCount", |d, deferred| {
            d.set_dot_count(deferred, cmd.dot_count);
        })
    }

    pub(super) fn cmd_set_dots_size(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsSizeRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsSize", |d, deferred| {
            d.set_dot_size(deferred, cmd.dot_size_px);
        })
    }

    pub(super) fn cmd_set_dots_color(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsColorRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsColor", |d, deferred| {
            if let Some(c) = cmd.dot_color {
                d.set_dot_color(deferred, c.into());
            }
            // Absent clears the second colour — the field message is `optional`, so
            // "not sent" and "sent empty" are distinguishable and mean different
            // things: leave it alone is not expressible here, and does not need to
            // be, because the colours are set together.
            d.set_dot_color_alt(deferred, cmd.dot_color_alt.map(Into::into));
        })
    }

    pub(super) fn cmd_set_dots_aperture(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsApertureRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsAperture", |d, deferred| {
            let field = if deferred {
                d.params.copy.field_size_px
            } else {
                d.params.live.field_size_px
            };
            d.set_aperture(deferred, aperture_from_proto(cmd.aperture, field));
        })
    }

    pub(super) fn cmd_set_dots_field_size(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsFieldSizeRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsFieldSize", |d, deferred| {
            d.set_field_size(deferred, [cmd.width_px, cmd.height_px]);
        })
    }

    pub(super) fn cmd_set_dots_lifetime(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsLifetimeRequest,
    ) -> proto::Response {
        self.with_dots(handle, "SetDotsLifetime", |d, deferred| {
            d.set_dot_lifetime(deferred, cmd.dot_lifetime_frames);
        })
    }

    pub(super) fn cmd_set_dots_seed(
        &mut self,
        handle: u32,
        cmd: proto::SetDotsSeedRequest,
    ) -> proto::Response {
        // Deliberately ignores deferred mode — see SetDotsSeedRequest in the proto.
        self.with_dots(handle, "SetDotsSeed", |d, _deferred| {
            d.set_seed(cmd.seed);
        })
    }
}
