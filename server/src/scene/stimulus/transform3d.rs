//! World-space placement and surface appearance for 3-D stimuli.
//!
//! **Placeholder — Phase A of `dev/3D_ROADMAP.md`.** These types exist so the
//! 2-D/3-D seam in [`Stimulus`](super::Stimulus) is real rather than
//! hypothetical: [`placement`](super::Stimulus::placement) has a `D3` arm, and
//! `is_3d()` is derived from it instead of from a hand-maintained variant list
//! that rots the first time someone forgets to extend it. Nothing renders them
//! yet.
//!
//! The roadmap specifies `glam::Vec3` / `glam::Quat` (§A.7, §B.2). Plain arrays
//! stand in until there is a renderer to justify the dependency; swapping them
//! is a mechanical change confined to this file and `mesh3d.rs`, and the wire
//! format is Euler degrees either way (§10.2 — "Euler angles on the wire,
//! quaternion in memory").

use crate::Color;

/// 3-D placement — the world-space counterpart of
/// [`Transform2D`](super::Transform2D).
///
/// World space is right-handed, **Y-up**, in **centimetres** (§3.2), matching
/// glTF and Blender export defaults. Y-up agrees with 2-D stimulus space, so a
/// stimulus moved "up" moves up in either dimension.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Transform3D {
    /// World space, cm.
    pub position_cm: [f32; 3],
    /// Degrees, yaw/pitch/roll — applied in `EulerRot::YXZ` order. Becomes a
    /// `glam::Quat` in memory once glam lands; the order is the part that
    /// silently differs between clients, so it is pinned here and in the
    /// `.proto`.
    pub rotation_euler_deg: [f32; 3],
    /// Non-uniform scale. Composed *on top of* the nominal size_cm a
    /// [`Mesh3dGeometry`](super::Mesh3dGeometry) carries.
    pub scale: [f32; 3],
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position_cm: [0.0; 3],
            rotation_euler_deg: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

impl Transform3D {
    /// Model matrix, folding in the geometry's nominal size_cm.
    ///
    /// Unimplemented: needs `glam::Mat4::from_scale_rotation_translation` and a
    /// decision on the Y-flip against the 2-D path's Y-up clip space (§3.3 —
    /// "the single most likely source of a lost day in Phase A"). Unit-test it
    /// before writing it; do not eyeball it.
    pub fn model_matrix(&self, _geometry_scale: [f32; 3]) -> [[f32; 4]; 4] {
        unimplemented!("Phase A: model matrix — see dev/3D_ROADMAP.md §3.3, §B.3")
    }
}

/// How a 3-D surface is shaded. Simple Phong or unlit — never PBR.
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Material3D {
    pub albedo: Color,
    /// Self-illumination, for stimuli that must hit a specific luminance.
    pub emissive: [f32; 3],
    pub shading: Shading3D,
}
