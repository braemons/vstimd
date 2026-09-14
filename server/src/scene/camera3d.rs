//! The 3-D camera and scene lighting — scene objects, not render state
//! (`dev/3D_ROADMAP.md` §1.4, §A.1, §10.1).
//!
//! It lives in [`SceneConfig`](super::SceneConfig) as a `Deferred<Camera3D>`, so a
//! batched camera + stimulus update flips on one frame like everything else, and a
//! scene-config that moves the camera saves where it put it.
//!
//! ## Coordinate conventions
//!
//! World space is right-handed, **Y-up**, in **centimetres** (§3.2). The camera at
//! rest sits at the origin looking down **−Z**.
//!
//! Clip space follows the *renderer's* convention, not Vulkan's textbook one: the
//! 2-D path maps screen-up to clip **+Y** (`render/tess.rs::px_to_ndc`), so the
//! projection here does **not** apply the usual Vulkan Y flip. A flip would render
//! the 3-D layer vertically mirrored against the 2-D overlay drawn on top of it —
//! and would also invert triangle winding, breaking back-face culling. The
//! `point_above_camera_lands_where_2d_puts_up` test pins this.

use glam::{EulerRot, Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Camera3D {
    /// World space, cm.
    pub position_cm: Vec3,
    /// Rotation about world +Y. Positive turns the view to the left
    /// (right-handed, seen from above).
    pub yaw_deg: f32,
    /// Rotation about the camera's +X. Positive tilts the view up.
    pub pitch_deg: f32,
    /// Rotation about the camera's view axis. Usually 0.
    pub roll_deg: f32,
    /// Vertical field of view; the horizontal one follows from the aspect ratio.
    pub fov_y_deg: f32,
    pub near_cm: f32,
    pub far_cm: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            position_cm: Vec3::ZERO,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
            fov_y_deg: 60.0,
            near_cm: 1.0,
            far_cm: 50_000.0,
        }
    }
}

impl Camera3D {
    /// Camera orientation in world space. Yaw, then pitch, then roll — the same
    /// `YXZ` order `Transform3D` pins, so a camera and an object given the same
    /// three angles face the same way.
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.yaw_deg.to_radians(),
            self.pitch_deg.to_radians(),
            self.roll_deg.to_radians(),
        )
    }

    /// World → view. The inverse of the camera's own placement.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation(), self.position_cm).inverse()
    }

    /// View → clip, depth in `[0, 1]`, clip +Y = screen up (see module doc).
    ///
    /// glam's *DirectX* projection is the one with exactly that output; its
    /// Vulkan projection is Y-down, which this renderer is not.
    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            self.fov_y_deg.to_radians(),
            aspect,
            self.near_cm,
            self.far_cm,
        )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj_matrix(aspect) * self.view_matrix()
    }
}

/// Scene-wide lighting for `Phong` surfaces: one ambient term and one
/// directional light (`dev/3D_ROADMAP.md` §10.1). `Unlit` surfaces ignore it,
/// which is what keeps their luminance equal to a 2-D stimulus of the same colour.
///
/// Colours are linear RGB multipliers, not display colours — a sun of `[2, 2, 2]`
/// is a brighter light, not an invalid one.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Lighting3D {
    pub ambient_color: [f32; 3],
    /// The direction the light travels, world space — *from* the sun. Any
    /// non-zero length; [`Self::sun_direction_normalized`] is what the shader gets.
    pub sun_direction: Vec3,
    pub sun_color: [f32; 3],
}

impl Default for Lighting3D {
    fn default() -> Self {
        Self {
            ambient_color: [0.1; 3],
            sun_direction: Vec3::new(-0.5, -1.0, -0.3).normalize(),
            sun_color: [1.0; 3],
        }
    }
}

impl Lighting3D {
    /// Unit sun direction. A zero or non-finite vector — which a command refuses,
    /// but a hand-edited scene-config can still hold — falls back to the default
    /// direction instead of putting NaN into every lit pixel.
    pub fn sun_direction_normalized(&self) -> Vec3 {
        self.sun_direction
            .try_normalize()
            .unwrap_or(Self::default().sun_direction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ndc(cam: &Camera3D, world: Vec3) -> Vec3 {
        let clip = cam.view_proj(16.0 / 9.0) * world.extend(1.0);
        clip.truncate() / clip.w
    }

    #[test]
    fn default_looks_down_negative_z() {
        let p = ndc(&Camera3D::default(), Vec3::new(0.0, 0.0, -100.0));
        assert!(p.x.abs() < 1e-6 && p.y.abs() < 1e-6, "{p}");
        assert!((0.0..=1.0).contains(&p.z), "{p}");
    }

    #[test]
    fn point_above_camera_lands_where_2d_puts_up() {
        // `tess::px_to_ndc` maps a stimulus at +y px to clip +y. A world point
        // above the view axis must land on the same side, or the 3-D layer is
        // mirrored under the 2-D one.
        let p = ndc(&Camera3D::default(), Vec3::new(0.0, 10.0, -100.0));
        assert!(p.y > 0.0, "{p}");
        let p = ndc(&Camera3D::default(), Vec3::new(10.0, 0.0, -100.0));
        assert!(p.x > 0.0, "right of the camera must be clip +x: {p}");
    }

    #[test]
    fn nearer_is_smaller_depth() {
        // The 3-D pipeline tests depth with `LESS`.
        let cam = Camera3D::default();
        let near = ndc(&cam, Vec3::new(0.0, 0.0, -10.0)).z;
        let far = ndc(&cam, Vec3::new(0.0, 0.0, -1000.0)).z;
        assert!(near < far, "near={near} far={far}");
        assert!(ndc(&cam, Vec3::new(0.0, 0.0, -cam.near_cm)).z.abs() < 1e-5);
    }

    #[test]
    fn positive_yaw_turns_left() {
        let cam = Camera3D {
            yaw_deg: 90.0,
            ..Default::default()
        };
        let p = ndc(&cam, Vec3::new(-100.0, 0.0, 0.0));
        assert!(p.x.abs() < 1e-5 && (0.0..=1.0).contains(&p.z), "{p}");
    }

    #[test]
    fn positive_pitch_looks_up() {
        let cam = Camera3D {
            pitch_deg: 90.0,
            ..Default::default()
        };
        let p = ndc(&cam, Vec3::new(0.0, 100.0, 0.0));
        assert!(
            p.x.abs() < 1e-5 && p.y.abs() < 1e-5 && (0.0..=1.0).contains(&p.z),
            "{p}"
        );
    }

    #[test]
    fn position_translates_the_view() {
        let cam = Camera3D {
            position_cm: Vec3::new(5.0, 7.0, 0.0),
            ..Default::default()
        };
        let p = ndc(&cam, Vec3::new(5.0, 7.0, -50.0));
        assert!(p.x.abs() < 1e-5 && p.y.abs() < 1e-5, "{p}");
    }

    #[test]
    fn zero_sun_direction_does_not_produce_nan() {
        let l = Lighting3D {
            sun_direction: Vec3::ZERO,
            ..Default::default()
        };
        let d = l.sun_direction_normalized();
        assert!(d.is_finite() && (d.length() - 1.0).abs() < 1e-5, "{d}");
    }
}
