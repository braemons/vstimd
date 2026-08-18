use lyon_tessellation::math::{Angle, Transform, Vector};

/// 2-D placement. Used by every positional stimulus.
/// Position is in stimulus-space pixels with origin at screen centre, Y-up.
/// Rotation angle_deg is counter-clockwise degrees.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Transform2D {
    pub pos_px: [f32; 2],
    pub angle_deg: f32, // ccw degrees, 0 = right, 90 = up
}

impl Default for Transform2D {
    fn default() -> Self {
        Self { pos_px: [0.0, 0.0], angle_deg: 0.0 }
    }
}

impl Transform2D {
    /// Rotation-then-translation affine transform for tessellation.
    pub fn to_transform(&self) -> Transform {
        Transform::rotation(Angle::degrees(self.angle_deg))
            .then_translate(Vector::new(self.pos_px[0], self.pos_px[1]))
    }
}
