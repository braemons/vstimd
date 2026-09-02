//! Scene-wide commands: background, deferred mode, bulk clear/enable, server
//! info, and the stimulus query/list payloads.

use super::convert::{
    grating_params_to_proto, nonempty, parse_version, shape_appearance_to_proto,
    stimulus_type_to_proto, text_params_to_proto,
};
use super::response::{err, err_not_found, ok_ack, ok_body};
use crate::proto;
use crate::scene::stimulus::{
    ShapeGeometry, Stimulus, StimulusBody, StimulusSceneEntry,
};
use crate::scene::SceneState;

/// The **user-facing** `StimulusType` for a stimulus, on the wire.
///
/// Both halves of the hop are elsewhere now: the scene decides *which* type a
/// stimulus is (`Stimulus::stimulus_type`, sourced from the geometry so an internal
/// body name can never leak), and `convert` maps that to the wire value.
fn stimulus_type_of(stim: &Stimulus) -> proto::StimulusType {
    stimulus_type_to_proto(stim.stimulus_type())
}

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
        let was_deferred = self.runtime.deferred_mode;
        if cmd.active {
            self.begin_deferred();
        } else if cmd.cancel {
            self.runtime.deferred_mode = false;
        } else {
            self.end_deferred();
        }
        // Say what the call did: ending or cancelling a mode that was never
        // begun is a no-op, and a client that cannot tell that apart from
        // "flip queued" has no way to know whether its staged frame is coming.
        ok_body(proto::response::Body::DeferredMode(
            proto::SetDeferredModeResponse {
                deferred: self.runtime.deferred_mode,
                flip_scheduled: self.runtime.pending_flip,
                was_deferred,
                // The render thread flips at the top of a frame and then counts
                // it, so the staged state is what the *next* frame is drawn
                // from. A client can wait for that number instead of sleeping.
                flip_frame: if self.runtime.pending_flip {
                    self.runtime.frame_count + 1
                } else {
                    0
                },
            },
        ))
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
                width_px: w,
                height_px: h,
                // Nominal, not measured: this is what clients convert durations
                // against, and a measurement would make the same script produce
                // a different frame count on every run (#120).
                frame_rate_hz: self.runtime.nominal_frame_rate_hz,
                measured_frame_rate_hz: self.runtime.frame_rate_hz,
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
                entry.identity.name = nonempty(cmd.name);
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
        // The wire taxonomy is the user's: `Rect`, `Ellipse` and `Circle` are
        // three `StimulusType`s and three `params` arms even though internally
        // they share one `Shape`. This match is where that mapping is declared.
        // Only the params arm is decided here. The `StimulusType` comes from
        // `stimulus_type_of`, so the geometry → user-facing-type mapping exists once
        // in the scene rather than a second time inside this match.
        let params = match &stim.body {
            StimulusBody::Shape(s) => {
                let appearance = Some(shape_appearance_to_proto(&s.appearance.live));
                match s.geometry.live {
                    ShapeGeometry::Rect { size_px } => {
                        proto::stimulus_params::Shape::Rect(proto::RectParams {
                            width_px: size_px[0],
                            height_px: size_px[1],
                            appearance,
                        })
                    }
                    ShapeGeometry::Ellipse { size_px } => {
                        proto::stimulus_params::Shape::Ellipse(proto::EllipseParams {
                            width_px: size_px[0],
                            height_px: size_px[1],
                            appearance,
                        })
                    }
                    ShapeGeometry::Circle { diameter_px } => {
                        proto::stimulus_params::Shape::Circle(proto::CircleParams {
                            diameter_px,
                            appearance,
                        })
                    }
                }
            }
            StimulusBody::Grating(g) => grating_params_to_proto(g)
                .shape
                .expect("grating_params_to_proto always sets a shape"),
            StimulusBody::Text(t) => text_params_to_proto(t)
                .shape
                .expect("text_params_to_proto always sets a shape"),
            // Unreachable: no command constructs a `Mesh3d` yet. Phase B owes the
            // `Sphere3DParams`/`Cube3DParams` oneof arms and a `transform_3d` arm on
            // the `placement` oneof — neither exists in the proto today, so there is
            // nothing honest to report here. `stimulus_type_to_proto` refuses the
            // matching wire value for the same reason.
            StimulusBody::Mesh3d(_) => {
                unimplemented!("Phase B: 3-D query params — see dev/3D_ROADMAP.md §10.2")
            }
        };

        let draw_order = self.config.stimuli.get_index_of(&handle).unwrap_or(0) as u32;
        // `placement` is a oneof with a single `transform_2d` arm, so a 3-D
        // stimulus has nothing to put in it until §10.2's `transform_3d` lands.
        let placement = stim.transform2d().map(|t| {
            proto::query_stimulus_response::Placement::Transform2d(proto::Transform2D {
                pos_px: Some(proto::Vec2 {
                    x: t.live.pos_px[0],
                    y: t.live.pos_px[1],
                }),
                rotation_deg: t.live.angle_deg,
            })
        });
        proto::QueryStimulusResponse {
            stimulus_type: stimulus_type_of(stim) as i32,
            enabled: stim.flags().enabled,
            anim_enabled: stim.flags().anim_enabled,
            opacity: stim.opacity().live,
            params: Some(proto::StimulusParams { shape: Some(params) }),
            id: entry.id().to_string(),
            name: entry.name().to_string(),
            draw_order,
            handle,
            placement,
            condition_indices: entry.conditions.clone(),
            condition_enabled: stim.flags().cond_enabled,
        }
    }

    // ── ListStimuli ───────────────────────────────────────────────────────────

    pub(super) fn cmd_list_stimuli(&self) -> proto::Response {
        let entries: Vec<proto::StimulusEntry> = self
            .stimuli
            .iter()
            .map(|(&handle, entry)| {
                let stim = &entry.stimulus;
                let stimulus_type = stimulus_type_of(stim) as i32;
                proto::StimulusEntry {
                    handle,
                    stimulus_type,
                    enabled: stim.flags().enabled,
                    id: entry.id().to_string(),
                    name: entry.name().to_string(),
                    condition_indices: entry.conditions.clone(),
                }
            })
            .collect();
        ok_body(proto::response::Body::StimulusList(
            proto::ListStimuliResponse { entries },
        ))
    }
}
