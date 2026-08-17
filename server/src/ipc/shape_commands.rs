//! Create/modify commands for the primitive shapes (rect, circle, ellipse)
//! and the transform + appearance setters shared by every stimulus type.
//!
//! The wire keeps one command per shape — `CreateRect`, `SetCircleRadius`,
//! `SetEllipseSize` — because that is what clients already speak. Internally
//! they all land on one [`Shape`] with a [`ShapeGeometry`] arm; the
//! many-to-one mapping lives here and nowhere else.

use super::convert::{
    color_or_default, nonempty, parse_or_new_uuid, proto_draw_mode_to_scene,
    shape_appearance_from_proto,
};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::stimulus::{
    Shape, ShapeAppearance, ShapeGeometry, Stimulus, StimulusKind, StimulusSceneEntry,
};
use crate::scene::SceneState;

impl SceneState {
    /// Insert a new shape stimulus.
    fn add_shape(
        &mut self,
        id: uuid::Uuid,
        name: Option<String>,
        pos: [f32; 2],
        angle: f32,
        appearance: ShapeAppearance,
        geometry: ShapeGeometry,
    ) -> u32 {
        let stimulus = Stimulus::from(Shape::new(pos, angle, appearance, geometry));
        self.add_stimulus(StimulusSceneEntry::new(id, name, stimulus))
    }

    /// Run `f` on the geometry of the shape at `handle`, rejecting the wrong
    /// geometry arm — `SetCircleRadius` must refuse a rect the way it always did
    /// — and marking the stimulus dirty unless the write was deferred.
    ///
    /// The handle lookup, the wrong-type rejection and the dirty bookkeeping are
    /// identical across all three size setters, and `dirty` now lives above the
    /// kind, so the caller holding the whole [`Stimulus`] is the one that can
    /// write it. Doing that once here keeps the three setters one-liners.
    ///
    /// `expected` is the **user-facing** type name quoted back in the error.
    fn with_shape_geometry(
        &mut self,
        handle: u32,
        cmd: &str,
        expected: &str,
        f: impl FnOnce(&mut ShapeGeometry, ShapeGeometry) -> bool,
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let StimulusKind::Shape(shape) = &mut entry.stimulus.kind else {
            return err_wrong_type(&entry.stimulus, cmd, expected);
        };
        let prev = if deferred {
            shape.geometry.copy
        } else {
            shape.geometry.live
        };
        let mut next = prev;
        if !f(&mut next, prev) {
            return err_wrong_type(&entry.stimulus, cmd, expected);
        }
        shape.geometry.set(deferred, next);
        if !deferred {
            entry.stimulus.flags_mut().mark_dirty();
        }
        ok_ack()
    }

    /// Apply an appearance edit. Shared by the four colour/outline/draw-mode
    /// commands, all of which are shape-only.
    fn set_appearance(
        &mut self,
        handle: u32,
        cmd: &str,
        f: impl FnOnce(ShapeAppearance) -> ShapeAppearance,
    ) -> proto::Response {
        // Not `with_shape`: these four report "is not supported for {type}"
        // rather than "requires a {expected} stimulus", and that wording is
        // client-visible.
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let StimulusKind::Shape(shape) = &mut entry.stimulus.kind else {
            return err(
                proto::ErrorCode::WrongStimulusType,
                format!(
                    "{} is not supported for {} stimuli",
                    cmd,
                    entry.stimulus.type_name()
                ),
            );
        };
        let prev = if deferred {
            shape.appearance.copy
        } else {
            shape.appearance.live
        };
        shape.appearance.set(deferred, f(prev));
        if !deferred {
            entry.stimulus.flags_mut().mark_dirty();
        }
        ok_ack()
    }

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
        let appearance =
            match shape_appearance_from_proto(cmd.appearance, fill, self.config.default_outline) {
                Ok(a) => a,
                Err(e) => return *e,
            };
        let handle = self.add_shape(
            id,
            nonempty(cmd.name),
            [center.x, center.y],
            cmd.angle,
            appearance,
            ShapeGeometry::Rect {
                size: [width, height],
            },
        );
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
        let appearance =
            match shape_appearance_from_proto(cmd.appearance, fill, self.config.default_outline) {
                Ok(a) => a,
                Err(e) => return *e,
            };
        let handle = self.add_shape(
            id,
            nonempty(cmd.name),
            [center.x, center.y],
            // A circle is rotationally symmetric, so it has no `angle` field.
            0.0,
            appearance,
            ShapeGeometry::Circle { radius },
        );
        ok_handle_with_id(handle, &id)
    }

    // ── CreateEllipse ─────────────────────────────────────────────────────────

    pub(super) fn cmd_create_ellipse(
        &mut self,
        cmd: proto::CreateEllipseRequest,
    ) -> proto::Response {
        let id = match parse_or_new_uuid(&cmd.id) {
            Ok(id) => id,
            Err(resp) => return *resp,
        };
        let center = cmd.center.unwrap_or_default();
        let width = if cmd.width == 0.0 { 100.0 } else { cmd.width };
        let height = if cmd.height == 0.0 { 100.0 } else { cmd.height };
        let fill = color_or_default(cmd.fill_color, self.config.default_fill);
        let appearance =
            match shape_appearance_from_proto(cmd.appearance, fill, self.config.default_outline) {
                Ok(a) => a,
                Err(e) => return *e,
            };
        let handle = self.add_shape(
            id,
            nonempty(cmd.name),
            [center.x, center.y],
            cmd.angle,
            appearance,
            ShapeGeometry::Ellipse {
                size: [width, height],
            },
        );
        ok_handle_with_id(handle, &id)
    }

    // ── SetEnabled ────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_enabled(
        &mut self,
        handle: u32,
        cmd: proto::SetEnabledRequest,
    ) -> proto::Response {
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

    pub(super) fn cmd_set_position(
        &mut self,
        handle: u32,
        cmd: proto::SetPositionRequest,
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                // `move_to_2d` writes a 2-D transform and fails if the
                // stimulus has none. Pixels have no meaning for world-space
                // placement, so a 3-D stimulus is rejected rather than silently
                // reinterpreted — see `Stimulus::move_to_2d`.
                if entry.stimulus.move_to_2d(deferred, cmd.x, cmd.y).is_ok() {
                    ok_ack()
                } else {
                    err_wrong_type(&entry.stimulus, "SetPosition", "2-D")
                }
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
        let deferred = self.runtime.deferred_mode;
        match self.config.stimuli.get_mut(&handle) {
            Some(entry) => {
                if entry.stimulus.set_angle_2d(deferred, cmd.angle_deg).is_ok() {
                    ok_ack()
                } else {
                    err_wrong_type(&entry.stimulus, "SetOrientation", "2-D")
                }
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
        self.set_appearance(handle, "SetFillColor", |prev| ShapeAppearance {
            fill_color: c,
            ..prev
        })
    }

    // ── SetAlpha ──────────────────────────────────────────────────────────────

    pub(super) fn cmd_set_alpha(
        &mut self,
        handle: u32,
        cmd: proto::SetAlphaRequest,
    ) -> proto::Response {
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
        self.with_shape_geometry(handle, "SetRectSize", "Rect", |next, prev| {
            if !matches!(prev, ShapeGeometry::Rect { .. }) {
                return false;
            }
            *next = ShapeGeometry::Rect {
                size: [cmd.width, cmd.height],
            };
            true
        })
    }

    // ── SetCircleRadius ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_circle_radius(
        &mut self,
        handle: u32,
        cmd: proto::SetCircleRadiusRequest,
    ) -> proto::Response {
        self.with_shape_geometry(handle, "SetCircleRadius", "Circle", |next, prev| {
            if !matches!(prev, ShapeGeometry::Circle { .. }) {
                return false;
            }
            *next = ShapeGeometry::Circle { radius: cmd.radius };
            true
        })
    }

    // ── SetEllipseSize ────────────────────────────────────────────────────────

    pub(super) fn cmd_set_ellipse_size(
        &mut self,
        handle: u32,
        cmd: proto::SetEllipseSizeRequest,
    ) -> proto::Response {
        self.with_shape_geometry(handle, "SetEllipseSize", "Ellipse", |next, prev| {
            if !matches!(prev, ShapeGeometry::Ellipse { .. }) {
                return false;
            }
            *next = ShapeGeometry::Ellipse {
                size: [cmd.width, cmd.height],
            };
            true
        })
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
        self.set_appearance(handle, "SetDrawMode", |prev| ShapeAppearance {
            draw_mode: mode,
            ..prev
        })
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
        self.set_appearance(handle, "SetOutlineColor", |prev| ShapeAppearance {
            outline_color: c,
            ..prev
        })
    }

    // ── SetOutlineWidth ───────────────────────────────────────────────────────

    pub(super) fn cmd_set_outline_width(
        &mut self,
        handle: u32,
        cmd: proto::SetOutlineWidthRequest,
    ) -> proto::Response {
        self.set_appearance(handle, "SetOutlineWidth", |prev| ShapeAppearance {
            stroke_width: cmd.line_width,
            ..prev
        })
    }
}
