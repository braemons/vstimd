//! Create/modify commands for the text stimulus.

use super::convert::{
    anchor_from_str, placement_from_proto, language_style_from_proto, identity_from_proto,
    text_render_params_from_proto,
};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::text::Text;
use crate::scene::stimulus::{Stimulus, StimulusBody, StimulusSceneEntry, StimulusType};

impl SceneState {
    /// Run `f` on the text stimulus at `handle`, then mark it dirty unless the
    /// write was deferred. See `with_grating` in `grating_commands.rs` — `dirty`
    /// lives above the kind now, so the caller writes it.
    fn with_text(
        &mut self,
        handle: u32,
        cmd: &str,
        f: impl FnOnce(&mut Text, bool),
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let StimulusBody::Text(t) = &mut entry.stimulus.body else {
            return err_wrong_type(&entry.stimulus, cmd, StimulusType::Text);
        };
        f(t, deferred);
        if !deferred {
            entry.stimulus.flags_mut().mark_dirty();
        }
        ok_ack()
    }
    // ── CreateText ────────────────────────────────────────────────────────────

    pub(super) fn cmd_create_text(&mut self, cmd: proto::CreateTextRequest) -> proto::Response {
        let params = cmd.params.unwrap_or_default();
        let (pos_px, angle_deg) = placement_from_proto(cmd.placement);
        let requested = params.box_size_px.unwrap_or_default();
        let box_size_px = [
            if requested.x == 0.0 { 200.0 } else { requested.x },
            if requested.y == 0.0 { 100.0 } else { requested.y },
        ];
        let letter_height_px = if params.letter_height_px == 0.0 {
            32.0
        } else {
            params.letter_height_px
        };
        let anchor = anchor_from_str(&params.anchor);
        let language_style = language_style_from_proto(params.language_style);
        let render_params = text_render_params_from_proto(&params);
        let identity = identity_from_proto(cmd.identity);
        let id = identity.id;
        let handle = self.alloc_stim_handle();
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(
                identity,
                Stimulus::from(Text::new(
                    pos_px,
                    angle_deg,
                    box_size_px,
                    params.text,
                    params.font,
                    letter_height_px,
                    anchor,
                    language_style,
                    render_params,
                )),
            ),
        );
        ok_handle_with_id(handle, &id)
    }

    // ── SetText ───────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_text(&mut self, handle: u32, cmd: proto::SetTextRequest) -> proto::Response {
        self.with_text(handle, "SetText", |s, deferred| {
            s.set_text(deferred, cmd.text);
        })
    }

    // ── SetTextColor ──────────────────────────────────────────────────────────

    pub(super) fn cmd_set_text_color(
        &mut self,
        handle: u32,
        cmd: proto::SetTextColorRequest,
    ) -> proto::Response {
        let c = match cmd.color {
            Some(c) => c.into(),
            None => return err(proto::ErrorCode::InvalidArgument, "color must be set"),
        };
        self.with_text(handle, "SetTextColor", |s, deferred| {
            s.set_color(deferred, c);
        })
    }
}
