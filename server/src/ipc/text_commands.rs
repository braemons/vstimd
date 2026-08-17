//! Create/modify commands for the text stimulus.

use super::convert::{nonempty, parse_or_new_uuid};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::text::{
    TextStimulus, anchor_from_str, proto_to_language_style, text_render_params_from_proto,
};
use crate::scene::stimulus::{Stimulus, StimulusSceneEntry};

impl SceneState {
    // ── CreateText ────────────────────────────────────────────────────────────

    pub(super) fn cmd_create_text(&mut self, cmd: proto::CreateTextRequest) -> proto::Response {
        let id = match parse_or_new_uuid(&cmd.id) {
            Ok(id) => id,
            Err(resp) => return *resp,
        };
        let pos = cmd.pos.unwrap_or_default();
        let requested = cmd.box_size.unwrap_or_default();
        let box_size = [
            if requested.x == 0.0 { 200.0 } else { requested.x },
            if requested.y == 0.0 { 100.0 } else { requested.y },
        ];
        let letter_height_px = if cmd.letter_height == 0.0 {
            32.0
        } else {
            cmd.letter_height
        };
        let anchor = anchor_from_str(&cmd.anchor);
        let language_style = proto_to_language_style(cmd.language_style);
        let params = text_render_params_from_proto(&cmd);
        let name = nonempty(cmd.name);
        let handle = self.alloc_stim_handle();
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(
                id,
                name,
                Stimulus::Text(TextStimulus::new(
                    [pos.x, pos.y],
                    box_size,
                    cmd.text,
                    cmd.font,
                    letter_height_px,
                    anchor,
                    language_style,
                    params,
                )),
            ),
        );
        ok_handle_with_id(handle, &id)
    }

    // ── SetText ───────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_text(&mut self, handle: u32, cmd: proto::SetTextRequest) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Text(s) => {
                    s.set_text(self.runtime.deferred_mode, cmd.text);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetText", "Text"),
            },
        }
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
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Text(s) => {
                    s.set_color(self.runtime.deferred_mode, c);
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetTextColor", "Text"),
            },
        }
    }
}
