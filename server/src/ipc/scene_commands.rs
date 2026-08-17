//! Scene-wide commands: background, deferred mode, bulk clear/enable, server
//! info, and the stimulus query/list payloads.

use super::convert::{nonempty, parse_version, shape_appearance_to_proto};
use super::response::{err, err_not_found, ok_ack, ok_body};
use crate::proto;
use crate::scene::stimulus::grating::grating_query_params;
use crate::scene::stimulus::text::text_query_params;
use crate::scene::stimulus::{Stimulus, StimulusSceneEntry};
use crate::scene::SceneState;

impl SceneState {
    // ── SetBackground ─────────────────────────────────────────────────────────

    pub(super) fn cmd_set_background(&mut self, cmd: proto::SetBackgroundRequest) -> proto::Response {
        let c = match cmd.color {
            Some(c) => c.into(),
            None => {
                return err(
                    proto::ErrorCode::InvalidArgument,
                    "background color must be set",
                );
            }
        };
        self.config.background.set(self.runtime.deferred_mode, c);
        ok_ack()
    }

    // ── SetDeferredMode ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_deferred_mode(&mut self, cmd: proto::SetDeferredModeRequest) -> proto::Response {
        if cmd.active {
            self.begin_deferred();
        } else if cmd.cancel {
            self.runtime.deferred_mode = false;
        } else {
            self.end_deferred();
        }
        ok_ack()
    }

    // ── ClearStimuli / ClearAnimations / ClearAll ─────────────────────────────

    pub(super) fn cmd_clear_stimuli(&mut self) -> proto::Response {
        self.clear_stimuli(false);
        ok_ack()
    }

    pub(super) fn cmd_clear_animations(&mut self) -> proto::Response {
        self.clear_animations();
        ok_ack()
    }

    pub(super) fn cmd_clear_all(&mut self) -> proto::Response {
        self.clear_scene(false);
        ok_ack()
    }

    // ── SetAllEnabled ─────────────────────────────────────────────────────────

    pub(super) fn cmd_set_all_enabled(&mut self, cmd: proto::SetAllEnabledRequest) -> proto::Response {
        self.set_all_enabled(cmd.enabled, false);
        ok_ack()
    }

    // ── QueryServerInfo ───────────────────────────────────────────────────────

    pub(super) fn cmd_query_server_info(&self) -> proto::Response {
        let (w, h) = self.runtime.screen_size.unwrap_or((0, 0));
        let bg = self.config.background.live;
        let version = parse_version();
        ok_body(proto::response::Body::ServerInfo(
            proto::QueryServerInfoResponse {
                width: w,
                height: h,
                // Nominal, not measured: this is what clients convert durations
                // against, and a measurement would make the same script produce
                // a different frame count on every run (#120).
                frame_rate: self.runtime.nominal_frame_rate,
                measured_frame_rate: self.runtime.frame_rate,
                background_color: Some(bg.into()),
                backend: proto::RenderBackend::Unspecified as i32,
                version: Some(version),
            },
        ))
    }

    // ── SetName ───────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_name(&mut self, handle: u32, cmd: proto::SetNameRequest) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                entry.name = nonempty(cmd.name);
                ok_ack()
            }
            None => err_not_found(handle),
        }
    }

    // ── QueryStimulus ─────────────────────────────────────────────────────────

    pub(super) fn cmd_query_stimulus(&self, handle: u32) -> proto::Response {
        let entry = match self.config.stimuli.get(&handle) {
            Some(e) => e,
            None => return err_not_found(handle),
        };
        ok_body(proto::response::Body::StimulusInfo(
            self.query_stimulus_response(handle, entry),
        ))
    }

    /// Build the full per-stimulus query payload. Shared by `QueryStimulus` and
    /// the web `SceneSnapshot` builder so both agree on geometry/appearance.
    pub(crate) fn query_stimulus_response(
        &self,
        handle: u32,
        entry: &StimulusSceneEntry,
    ) -> proto::QueryStimulusResponse {
        let stim = &entry.stimulus;

        // Everything that is true of any stimulus stays at the top level;
        // per-type state goes in `params`, per-dimension placement in
        // `placement`. Nothing is synthesised to fill a field that does not
        // apply — a grating has no outline, and says so by omitting it.
        let (stimulus_type, params) = match stim {
            Stimulus::Rect(r) => (
                proto::StimulusType::Rect,
                proto::stimulus_params::Shape::Rect(proto::RectParams {
                    width: r.size.live[0],
                    height: r.size.live[1],
                    appearance: Some(shape_appearance_to_proto(&r.appearance.live)),
                }),
            ),
            Stimulus::Circle(c) => (
                proto::StimulusType::Circle,
                proto::stimulus_params::Shape::Circle(proto::CircleParams {
                    radius: c.radius.live,
                    appearance: Some(shape_appearance_to_proto(&c.appearance.live)),
                }),
            ),
            Stimulus::Ellipse(e) => (
                proto::StimulusType::Ellipse,
                proto::stimulus_params::Shape::Ellipse(proto::EllipseParams {
                    width: e.size.live[0],
                    height: e.size.live[1],
                    appearance: Some(shape_appearance_to_proto(&e.appearance.live)),
                }),
            ),
            Stimulus::Grating(g) => (
                proto::StimulusType::Grating,
                grating_query_params(g)
                    .shape
                    .expect("grating_query_params always sets a shape"),
            ),
            Stimulus::Text(t) => (
                proto::StimulusType::Text,
                text_query_params(t)
                    .shape
                    .expect("text_query_params always sets a shape"),
            ),
        };

        let pos = stim.get_pos();
        let draw_order = self.config.stimuli.get_index_of(&handle).unwrap_or(0) as u32;
        proto::QueryStimulusResponse {
            stimulus_type: stimulus_type as i32,
            enabled: stim.flags().enabled,
            anim_enabled: stim.flags().anim_enabled,
            opacity: stim.opacity().live,
            params: Some(proto::StimulusParams { shape: Some(params) }),
            id: entry.id.to_string(),
            name: entry.name.clone().unwrap_or_default(),
            draw_order,
            handle,
            placement: Some(proto::query_stimulus_response::Placement::Transform2d(
                proto::Transform2D {
                    pos: Some(proto::Vec2 { x: pos[0], y: pos[1] }),
                    rotation_deg: stim.transform().live.angle,
                },
            )),
        }
    }

    // ── ListStimuli ───────────────────────────────────────────────────────────

    pub(super) fn cmd_list_stimuli(&self) -> proto::Response {
        let entries: Vec<proto::StimulusEntry> = self
            .stimuli
            .iter()
            .map(|(&handle, entry)| {
                let stim = &entry.stimulus;
                let stimulus_type = match stim {
                    Stimulus::Rect(_) => proto::StimulusType::Rect,
                    Stimulus::Ellipse(_) => proto::StimulusType::Ellipse,
                    Stimulus::Circle(_) => proto::StimulusType::Circle,
                    Stimulus::Grating(_) => proto::StimulusType::Grating,
                    Stimulus::Text(_) => proto::StimulusType::Text,
                } as i32;
                proto::StimulusEntry {
                    handle,
                    stimulus_type,
                    enabled: stim.flags().enabled,
                    id: entry.id.to_string(),
                    name: entry.name.clone().unwrap_or_default(),
                }
            })
            .collect();
        ok_body(proto::response::Body::StimulusList(
            proto::ListStimuliResponse { entries },
        ))
    }
}
