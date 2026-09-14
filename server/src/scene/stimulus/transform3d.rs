//! World-space placement and surface appearance for 3-D stimuli.
//!
//! Rotation is stored as Euler degrees rather than a quaternion: the config
//! format is the runtime shape (no DTO), and a scene-config is hand-edited — a
//! quaternion there is unreadable. The quaternion is built where the matrix is.

use glam::{EulerRot, Mat4, Quat, Vec3};

use crate::Color;

/// 3-D placement — the world-space counterpart of
/// [`Transform2D`](super::Transform2D).
///
/// World space is right-handed, **Y-up**, in **centimetres** (§3.2), matching
/// glTF and Blender export defaults. Y-up agrees with 2-D stimulus space, so a
/// stimulus moved "up" moves up in either dimension.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Transform3D {
    /// World space, cm.
    pub position_cm: Vec3,
    /// Degrees, `[yaw, pitch, roll]` — about world +Y, then the object's +X,
    /// then its +Z (`EulerRot::YXZ`), the same order [`Camera3D`] uses. Positive
    /// yaw turns counter-clockwise seen from above, so +X swings towards −Z.
    ///
    /// [`Camera3D`]: crate::scene::Camera3D
    pub rotation_euler_deg: Vec3,
    /// Non-uniform scale, composed *on top of* the size the
    /// [`Mesh3dGeometry`](super::Mesh3dGeometry) carries.
    pub scale: Vec3,
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position_cm: Vec3::ZERO,
            rotation_euler_deg: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }
}

impl Transform3D {
    pub fn rotation(&self) -> Quat {
        let [yaw, pitch, roll] = self.rotation_euler_deg.to_array().map(f32::to_radians);
        Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll)
    }

    /// Model matrix for a unit mesh: scale by `geometry_scale` (from
    /// [`Mesh3dGeometry::model_scale`](super::Mesh3dGeometry::model_scale)) and
    /// then by `scale`, rotate, translate.
    pub fn model_matrix(&self, geometry_scale: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale * geometry_scale,
            self.rotation(),
            self.position_cm,
        )
    }
}

/// How a 3-D surface is shaded. Simple Phong or unlit — never PBR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Shading3D {
    /// Albedo only, no lighting. What most psychophysics stimuli want.
    #[default]
    Unlit,
    /// Lambert diffuse + Blinn-Phong specular, one directional light.
    Phong,
}

/// Surface appearance for a 3-D stimulus — the 3-D peer of
/// [`ShapeAppearance`](super::ShapeAppearance).
///
/// Deliberately *not* unified with `ShapeAppearance`: fill/outline/draw-mode and
/// albedo/emissive/shading have nothing in common beyond both being "the
/// appearance blob". The two accessors are peers; the types are not related.
///
/// Carries no alpha of its own beyond `albedo.a`, which the shared
/// [`StimulusCommon::opacity`](super::StimulusCommon::opacity) multiplies. No
/// `roughness` field — we do not do PBR, and a dead field in the wire format and
/// the config JSON is a liability (§B.2).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Material3D {
    pub albedo: Color,
    /// Self-illumination, for stimuli that must hit a specific luminance.
    pub emissive: [f32; 3],
    pub shading: Shading3D,
}

/// Opaque white, unlit: a surface that shows its texture (or its vertex colour)
/// unchanged. `Color::default()` would be transparent black, i.e. invisible.
impl Default for Material3D {
    fn default() -> Self {
        Self {
            albedo: Color::WHITE,
            emissive: [0.0; 3],
            shading: Shading3D::Unlit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn translation_moves_the_origin() {
        let t = Transform3D {
            position_cm: Vec3::new(1.0, 2.0, -3.0),
            ..Default::default()
        };
        let p = t.model_matrix(Vec3::ONE).transform_point3(Vec3::ZERO);
        assert!(close(p, t.position_cm), "{p}");
    }

    #[test]
    fn positive_yaw_swings_x_towards_negative_z() {
        let t = Transform3D {
            rotation_euler_deg: Vec3::new(90.0, 0.0, 0.0),
            ..Default::default()
        };
        let p = t.model_matrix(Vec3::ONE).transform_point3(Vec3::X);
        assert!(close(p, Vec3::NEG_Z), "{p}");
    }

    #[test]
    fn yaw_is_applied_after_pitch() {
        // YXZ composes as Ry · Rx · Rz, so a vector meets roll, then pitch, then yaw.
        let t = Transform3D {
            rotation_euler_deg: Vec3::new(90.0, 90.0, 0.0),
            ..Default::default()
        };
        let p = t.model_matrix(Vec3::ONE).transform_vector3(Vec3::Z);
        // Pitch +90 about X takes +Z to −Y; yaw about Y leaves −Y alone.
        assert!(close(p, Vec3::NEG_Y), "{p}");
    }

    #[test]
    fn geometry_scale_and_scale_compose_before_rotation() {
        let t = Transform3D {
            rotation_euler_deg: Vec3::new(90.0, 0.0, 0.0),
            scale: Vec3::new(2.0, 1.0, 1.0),
            ..Default::default()
        };
        // X is stretched by 2 × 5 in object space, then yawed onto −Z.
        let p = t
            .model_matrix(Vec3::new(5.0, 1.0, 1.0))
            .transform_point3(Vec3::X);
        assert!(close(p, Vec3::new(0.0, 0.0, -10.0)), "{p}");
    }

    #[test]
    fn default_material_is_visible() {
        assert_eq!(Material3D::default().albedo, Color::WHITE);
    }
}
