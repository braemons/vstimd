//! Positions typed by their unit, at the one boundary where two units meet.
//!
//! 2-D stimuli are placed in **pixels** from the screen centre, Y up; 3-D
//! stimuli and the camera in world **centimetres**. Both used to be bare
//! `[f32; N]`, so nothing but a doc comment stopped a centimetre position
//! reaching a pixel command. These newtypes make that a compile error (#124).
//!
//! Deliberately small: two position types, no arithmetic and no conversions
//! between them, because no conversion between pixels and world centimetres
//! is meaningful. Code that computes with a position unwraps it (`.0`) where
//! the computation happens; the wire conversions in `ipc/convert` wrap raw
//! numbers on the way in. Both serialize exactly as the bare array did, so the
//! scene-config format is unchanged.

use glam::Vec3;

/// A 2-D position in stimulus space: pixels from the screen centre, Y up.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Pos2Px(pub [f32; 2]);

impl Pos2Px {
    pub const ORIGIN: Self = Self([0.0, 0.0]);

    pub const fn new(x: f32, y: f32) -> Self {
        Self([x, y])
    }

    pub const fn x(self) -> f32 {
        self.0[0]
    }

    pub const fn y(self) -> f32 {
        self.0[1]
    }
}

/// A 3-D position in world space: right-handed, Y up, centimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Pos3Cm(pub Vec3);

impl Pos3Cm {
    pub const ORIGIN: Self = Self(Vec3::ZERO);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The newtypes must not change the scene-config format.
    #[test]
    fn serialize_as_the_bare_array() {
        assert_eq!(
            serde_json::to_string(&Pos2Px::new(1.5, -2.0)).unwrap(),
            "[1.5,-2.0]"
        );
        assert_eq!(
            serde_json::to_string(&Pos3Cm::new(1.0, 2.0, 3.0)).unwrap(),
            "[1.0,2.0,3.0]"
        );
        let p: Pos2Px = serde_json::from_str("[3.0,4.0]").unwrap();
        assert_eq!((p.x(), p.y()), (3.0, 4.0));
    }
}
