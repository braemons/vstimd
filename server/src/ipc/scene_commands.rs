//! Scene-wide commands: background, deferred mode, bulk clear/enable, server
//! info, and the stimulus query/list payloads.

use super::convert::{
    grating_query_params, nonempty, parse_version, shape_appearance_to_proto,
    text_query_params,
};
use super::response::{err, err_not_found, ok_ack, ok_body};
use crate::proto;
use crate::scene::stimulus::{
    Mesh3dGeometry, ShapeGeometry, Stimulus, StimulusBody, StimulusSceneEntry,
};
use crate::scene::SceneState;

/// The **user-facing** `StimulusType` for a stimulus.
///
/// The mapping is many-to-one in the other direction: three `StimulusType`s come
/// out of one [`StimulusBody::Shape`], and (from Phase B) three more out of one
/// [`StimulusBody::Mesh3d`]. Sourced from the geometry so an internal kind name
/// can never reach a client.
fn stimulus_type_of(stim: &Stimulus) -> proto::StimulusType {
    match &stim.body {
        StimulusBody::Shape(s) => match s.geometry.live {
            ShapeGeometry::Rect { .. } => proto::StimulusType::Rect,
            ShapeGeometry::Ellipse { .. } => proto::StimulusType::Ellipse,
            ShapeGeometry::Circle { .. } => proto::StimulusType::Circle,
        },
        StimulusBody::Grating(_) => proto::StimulusType::Grating,
        StimulusBody::Text(_) => proto::StimulusType::Text,
        // Phase B: §10.2 reserves `StimulusType` 20–29 for 3-D. Unreachable
        // until a command constructs a `Mesh3d`.
        StimulusBody::Mesh3d(m) => match m.geometry.live {
            Mesh3dGeometry::Cube { .. }
            | Mesh3dGeometry::Sphere { .. }
            | Mesh3dGeometry::Plane { .. } => {
                unimplemented!("Phase B: STIMULUS_TYPE_CUBE_3D / _SPHERE_3D / _PLANE_3D")
            }
        },
    }
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
        let (stimulus_type, params) = match &stim.body {
            StimulusBody::Shape(s) => {
                let appearance = Some(shape_appearance_to_proto(&s.appearance.live));
                match s.geometry.live {
                    ShapeGeometry::Rect { size } => (
                        proto::StimulusType::Rect,
                        proto::stimulus_params::Shape::Rect(proto::RectParams {
                            width: size[0],
                            height: size[1],
                            appearance,
                        }),
                    ),
                    ShapeGeometry::Ellipse { size } => (
                        proto::StimulusType::Ellipse,
                        proto::stimulus_params::Shape::Ellipse(proto::EllipseParams {
                            width: size[0],
                            height: size[1],
                            appearance,
                        }),
                    ),
                    ShapeGeometry::Circle { diameter } => (
                        proto::StimulusType::Circle,
                        proto::stimulus_params::Shape::Circle(proto::CircleParams {
                            diameter,
                            appearance,
                        }),
                    ),
                }
            }
            StimulusBody::Grating(g) => (
                proto::StimulusType::Grating,
                grating_query_params(g)
                    .shape
                    .expect("grating_query_params always sets a shape"),
            ),
            StimulusBody::Text(t) => (
                proto::StimulusType::Text,
                text_query_params(t)
                    .shape
                    .expect("text_query_params always sets a shape"),
            ),
            // Unreachable: no command constructs a `Mesh3d` yet. Phase B owes
            // `StimulusType::Sphere3D`/`Cube3D` (§10.2 reserves 20–29), the
            // `Sphere3DParams`/`Cube3DParams` oneof arms, and a `transform_3d`
            // arm on the `placement` oneof — which does not exist in the proto
            // today, so there is nothing honest to report here yet.
            StimulusBody::Mesh3d(_) => {
                unimplemented!("Phase B: 3-D query params — see dev/3D_ROADMAP.md §10.2")
            }
        };

        let draw_order = self.config.stimuli.get_index_of(&handle).unwrap_or(0) as u32;
        // `placement` is a oneof with a single `transform_2d` arm, so a 3-D
        // stimulus has nothing to put in it until §10.2's `transform_3d` lands.
        let placement = stim.transform2d().map(|t| {
            proto::query_stimulus_response::Placement::Transform2d(proto::Transform2D {
                pos: Some(proto::Vec2 {
                    x: t.live.pos[0],
                    y: t.live.pos[1],
                }),
                rotation_deg: t.live.angle,
            })
        });
        proto::QueryStimulusResponse {
            stimulus_type: stimulus_type as i32,
            enabled: stim.flags().enabled,
            anim_enabled: stim.flags().anim_enabled,
            opacity: stim.opacity().live,
            params: Some(proto::StimulusParams { shape: Some(params) }),
            id: entry.id().to_string(),
            name: entry.name().to_string(),
            draw_order,
            handle,
            placement,
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
                }
            })
            .collect();
        ok_body(proto::response::Body::StimulusList(
            proto::ListStimuliResponse { entries },
        ))
    }
}
