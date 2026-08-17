//! Create/modify commands for the primitive shapes (rect, circle, ellipse)
//! and the transform + appearance setters shared by every stimulus type.

use super::convert::{
    color_or_default, nonempty, parse_or_new_uuid, proto_draw_mode_to_scene,
};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::{Deferred, SceneState};
use crate::scene::stimulus::{
    CircleStimulus, EllipseStimulus, RectStimulus, ShapeAppearance, Stimulus, StimulusCommon,
    StimulusSceneEntry,
};

impl SceneState {
    // ── CreateRect ────────────────────────────────────────────────────────────

    pub(super) fn cmd_create_rect(&mut self, cmd: proto::CreateRectRequest) -> proto::Response {
        let id = match parse_or_new_uuid(&cmd.id) {
            Ok(id) => id,
            Err(resp) => return *resp,
        };
        let center = cmd.center.unwrap_or_default();
        let width = if cmd.width == 0.0 { 100.0 } else { cmd.width };
        let height = if cmd.height == 0.0 { 100.0 } else { cmd.height };
        let fill = color_or_default(cmd.fill_color, self.config.default_fill);
        let entry = StimulusSceneEntry::new(
            id,
            nonempty(cmd.name),
            Stimulus::Rect(RectStimulus {
                common: StimulusCommon::new([center.x, center.y], 0.0),
                appearance: Deferred::new(ShapeAppearance {
                    fill_color: fill,
                    outline_color: self.config.default_outline,
                    ..Default::default()
                }),
                size: Deferred::new([width, height]),
            }),
        );
        let handle = self.add_stimulus(entry);
        ok_handle_with_id(handle, &id)
    }

    // ── CreateCircle ──────────────────────────────────────────────────────────

    pub(super) fn cmd_create_circle(&mut self, cmd: proto::CreateCircleRequest) -> proto::Response {
        let id = match parse_or_new_uuid(&cmd.id) {
            Ok(id) => id,
            Err(resp) => return *resp,
        };
        let center = cmd.center.unwrap_or_default();
        let radius = if cmd.radius == 0.0 { 50.0 } else { cmd.radius };
        let fill = color_or_default(cmd.fill_color, self.config.default_fill);
        let entry = StimulusSceneEntry::new(
            id,
            nonempty(cmd.name),
            Stimulus::Circle(CircleStimulus {
                common: StimulusCommon::new([center.x, center.y], 0.0),
                appearance: Deferred::new(ShapeAppearance {
                    fill_color: fill,
                    outline_color: self.config.default_outline,
                    ..Default::default()
                }),
                radius: Deferred::new(radius),
            }),
        );
        let handle = self.add_stimulus(entry);
        ok_handle_with_id(handle, &id)
    }

    // ── CreateEllipse ─────────────────────────────────────────────────────────

    pub(super) fn cmd_create_ellipse(&mut self, cmd: proto::CreateEllipseRequest) -> proto::Response {
        let id = match parse_or_new_uuid(&cmd.id) {
            Ok(id) => id,
            Err(resp) => return *resp,
        };
        let center = cmd.center.unwrap_or_default();
        let width = if cmd.width == 0.0 { 100.0 } else { cmd.width };
        let height = if cmd.height == 0.0 { 100.0 } else { cmd.height };
        let fill = color_or_default(cmd.fill_color, self.config.default_fill);
        let entry = StimulusSceneEntry::new(
            id,
            nonempty(cmd.name),
            Stimulus::Ellipse(EllipseStimulus {
                common: StimulusCommon::new([center.x, center.y], cmd.angle),
                appearance: Deferred::new(ShapeAppearance {
                    fill_color: fill,
                    outline_color: self.config.default_outline,
                    ..Default::default()
                }),
                size: Deferred::new([width, height]),
            }),
        );
        let handle = self.add_stimulus(entry);
        ok_handle_with_id(handle, &id)
    }

    // ── SetEnabled ────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_enabled(&mut self, handle: u32, cmd: proto::SetEnabledRequest) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                if self.runtime.deferred_mode {
                    entry.stimulus.flags_mut().enabled_copy = cmd.enabled;
                } else {
                    entry.stimulus.flags_mut().enabled = cmd.enabled;
                    entry.stimulus.flags_mut().mark_dirty();
                }
                ok_ack()
            }
            None => err_not_found(handle),
        }
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    pub(super) fn cmd_delete(&mut self, handle: u32) -> proto::Response {
        match self.config.stimuli.shift_remove(&handle) {
            Some(_) => ok_ack(),
            None => err_not_found(handle),
        }
    }

    // ── SetPosition ───────────────────────────────────────────────────────────

    pub(super) fn cmd_set_position(&mut self, handle: u32, cmd: proto::SetPositionRequest) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                entry
                    .stimulus
                    .move_to(self.runtime.deferred_mode, cmd.x, cmd.y);
                ok_ack()
            }
            None => err_not_found(handle),
        }
    }

    // ── SetOrientation ────────────────────────────────────────────────────────

    pub(super) fn cmd_set_orientation(
        &mut self,
        handle: u32,
        cmd: proto::SetOrientationRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                entry
                    .stimulus
                    .set_angle(self.runtime.deferred_mode, cmd.angle_deg);
                ok_ack()
            }
            None => err_not_found(handle),
        }
    }

    // ── SetFillColor ──────────────────────────────────────────────────────────

    pub(super) fn cmd_set_fill_color(
        &mut self,
        handle: u32,
        cmd: proto::SetFillColorRequest,
    ) -> proto::Response {
        let c = match cmd.color {
            Some(c) => c.into(),
            None => return err(proto::ErrorCode::InvalidArgument, "fill color must be set"),
        };
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => {
                let stim = &mut entry.stimulus;
                if !stim.is_shape() {
                    return err(
                        proto::ErrorCode::WrongStimulusType,
                        format!(
                            "SetFillColor is not supported for {} stimuli",
                            stim.type_name()
                        ),
                    );
                }
                let deferred = self.runtime.deferred_mode;
                let app = stim.shape_appearance_mut().expect("is_shape checked");
                let prev = if deferred { app.copy } else { app.live };
                app.set(
                    deferred,
                    ShapeAppearance {
                        fill_color: c,
                        ..prev
                    },
                );
                if !deferred {
                    stim.flags_mut().mark_dirty();
                }
                ok_ack()
            }
        }
    }

    // ── SetAlpha ──────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_alpha(&mut self, handle: u32, cmd: proto::SetAlphaRequest) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => {
                // Valid for every stimulus type: opacity is shared state, and
                // multiplies the alpha of whatever colours the stimulus carries
                // rather than overwriting any one of them.
                entry
                    .stimulus
                    .set_opacity(self.runtime.deferred_mode, cmd.opacity);
                ok_ack()
            }
        }
    }

    // ── SetRectSize ───────────────────────────────────────────────────────────

    pub(super) fn cmd_set_rect_size(
        &mut self,
        handle: u32,
        cmd: proto::SetRectSizeRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Rect(s) => {
                    s.size.set(self.runtime.deferred_mode, [cmd.width, cmd.height]);
                    if !self.runtime.deferred_mode {
                        s.common.flags.mark_dirty();
                    }
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetRectSize", "Rect"),
            },
        }
    }

    // ── SetCircleRadius ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_circle_radius(
        &mut self,
        handle: u32,
        cmd: proto::SetCircleRadiusRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Circle(s) => {
                    s.radius.set(self.runtime.deferred_mode, cmd.radius);
                    if !self.runtime.deferred_mode {
                        s.common.flags.mark_dirty();
                    }
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetCircleRadius", "Circle"),
            },
        }
    }

    // ── SetEllipseSize ────────────────────────────────────────────────────────

    pub(super) fn cmd_set_ellipse_size(
        &mut self,
        handle: u32,
        cmd: proto::SetEllipseSizeRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => match &mut entry.stimulus {
                Stimulus::Ellipse(s) => {
                    s.size.set(self.runtime.deferred_mode, [cmd.width, cmd.height]);
                    if !self.runtime.deferred_mode {
                        s.common.flags.mark_dirty();
                    }
                    ok_ack()
                }
                stim => err_wrong_type(stim, "SetEllipseSize", "Ellipse"),
            },
        }
    }

    // ── SetDrawMode ───────────────────────────────────────────────────────────

    pub(super) fn cmd_set_draw_mode(
        &mut self,
        handle: u32,
        cmd: proto::SetDrawModeRequest,
    ) -> proto::Response {
        let mode = match proto_draw_mode_to_scene(cmd.mode) {
            Ok(m) => m,
            Err(e) => return *e,
        };
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => {
                let stim = &mut entry.stimulus;
                if !stim.is_shape() {
                    return err(
                        proto::ErrorCode::WrongStimulusType,
                        format!(
                            "SetDrawMode is not supported for {} stimuli",
                            stim.type_name()
                        ),
                    );
                }
                let deferred = self.runtime.deferred_mode;
                let app = stim.shape_appearance_mut().expect("is_shape checked");
                let prev = if deferred { app.copy } else { app.live };
                app.set(
                    deferred,
                    ShapeAppearance {
                        draw_mode: mode,
                        ..prev
                    },
                );
                if !deferred {
                    stim.flags_mut().mark_dirty();
                }
                ok_ack()
            }
        }
    }

    // ── SetOutlineColor ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_outline_color(
        &mut self,
        handle: u32,
        cmd: proto::SetOutlineColorRequest,
    ) -> proto::Response {
        let c = match cmd.color {
            Some(c) => c.into(),
            None => {
                return err(
                    proto::ErrorCode::InvalidArgument,
                    "outline color must be set",
                );
            }
        };
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => {
                let stim = &mut entry.stimulus;
                if !stim.is_shape() {
                    return err(
                        proto::ErrorCode::WrongStimulusType,
                        format!(
                            "SetOutlineColor is not supported for {} stimuli",
                            stim.type_name()
                        ),
                    );
                }
                let deferred = self.runtime.deferred_mode;
                let app = stim.shape_appearance_mut().expect("is_shape checked");
                let prev = if deferred { app.copy } else { app.live };
                app.set(
                    deferred,
                    ShapeAppearance {
                        outline_color: c,
                        ..prev
                    },
                );
                if !deferred {
                    stim.flags_mut().mark_dirty();
                }
                ok_ack()
            }
        }
    }

    // ── SetOutlineWidth ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_outline_width(
        &mut self,
        handle: u32,
        cmd: proto::SetOutlineWidthRequest,
    ) -> proto::Response {
        match self.config.stimuli.get_mut(&handle) {
            None => err_not_found(handle),
            Some(entry) => {
                let stim = &mut entry.stimulus;
                if !stim.is_shape() {
                    return err(
                        proto::ErrorCode::WrongStimulusType,
                        format!(
                            "SetOutlineWidth is not supported for {} stimuli",
                            stim.type_name()
                        ),
                    );
                }
                let deferred = self.runtime.deferred_mode;
                let app = stim.shape_appearance_mut().expect("is_shape checked");
                let prev = if deferred { app.copy } else { app.live };
                app.set(
                    deferred,
                    ShapeAppearance {
                        stroke_width: cmd.line_width,
                        ..prev
                    },
                );
                if !deferred {
                    stim.flags_mut().mark_dirty();
                }
                ok_ack()
            }
        }
    }
}
