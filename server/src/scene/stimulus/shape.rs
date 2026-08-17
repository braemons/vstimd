use super::shape_appearance::ShapeAppearance;
use super::transform2d::Transform2D;
use crate::scene::deferred::Deferred;

/// A flat coloured shape: rectangle, ellipse or circle.
///
/// One struct, not three. Rect, ellipse and circle share a pipeline
/// (`solid_pipeline`), a mesh cache (`SceneCache::solid`), a dirty/upload
/// lifecycle and an appearance; the only thing that differs between them is
/// which lyon path gets built, which is exactly what [`ShapeGeometry`] says.
/// Splitting them into three types meant three identical `make_copy`/`flip`
/// impls and a `shape_arm!` macro to paper over the sameness.
///
/// The user-facing taxonomy stays finer than this: the wire still has
/// `CreateRect` / `SetCircleRadius` / `StimulusType::Rect`, and that mapping
/// lives in `ipc/` — an internal kind name never reaches a client.
///
/// No runtime state, so no config/runtime split (unlike
/// [`Grating`](super::Grating), which owns `phase_accum`).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Shape {
    pub transform: Deferred<Transform2D>,
    pub appearance: Deferred<ShapeAppearance>,
    pub geometry: Deferred<ShapeGeometry>,
}

/// Which shape, and how big — the only thing that distinguishes a rect from an
/// ellipse from a circle.
///
/// Sizes are **full extents** in pixels, the same numbers the command API takes
/// and reports; the tessellator halves them (`render::tess`).
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ShapeGeometry {
    Rect { size: [f32; 2] },
    Ellipse { size: [f32; 2] },
    Circle { radius: f32 },
}

impl Default for ShapeGeometry {
    fn default() -> Self {
        Self::Rect { size: [0.0, 0.0] }
    }
}

impl ShapeGeometry {
    /// The **user-facing** type name, as it appears in the config `"type"` tag,
    /// in `StimulusType`, and in `WRONG_STIMULUS_TYPE` error messages. Never the
    /// internal kind name — a client has never heard of "Shape".
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Rect { .. } => "Rect",
            Self::Ellipse { .. } => "Ellipse",
            Self::Circle { .. } => "Circle",
        }
    }

    /// Full extents in px — `None` for a circle, which carries a
    /// [`radius`](Self::radius) instead. The asymmetry is the wire's:
    /// `CreateCircle`/`SetCircleRadius` take a radius where the other two take
    /// width and height.
    pub fn size(&self) -> Option<[f32; 2]> {
        match *self {
            Self::Rect { size } | Self::Ellipse { size } => Some(size),
            Self::Circle { .. } => None,
        }
    }

    /// Radius in px — `None` for rect and ellipse.
    pub fn radius(&self) -> Option<f32> {
        match *self {
            Self::Circle { radius } => Some(radius),
            Self::Rect { .. } | Self::Ellipse { .. } => None,
        }
    }
}

impl Shape {
    pub fn new(
        pos: [f32; 2],
        angle: f32,
        appearance: ShapeAppearance,
        geometry: ShapeGeometry,
    ) -> Self {
        Self {
            transform: Deferred::new(Transform2D { pos, angle }),
            appearance: Deferred::new(appearance),
            geometry: Deferred::new(geometry),
        }
    }

    pub fn make_copy(&mut self) {
        self.transform.make_copy();
        self.appearance.make_copy();
        self.geometry.make_copy();
    }

    pub fn flip(&mut self) {
        self.transform.flip();
        self.appearance.flip();
        self.geometry.flip();
    }
}
