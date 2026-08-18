use super::shape_appearance::ShapeAppearance;
use super::stimulus_type::StimulusType;
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
/// `CreateRect` / `SetCircleDiameter` / `StimulusType::Rect`, and that mapping
/// lives in `ipc/` — an internal kind name never reaches a client.
///
/// No runtime state, so no config/runtime split (unlike
/// [`Grating`](super::Grating), which owns `phase_accum_cycles`).
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
    Rect { size_px: [f32; 2] },
    Ellipse { size_px: [f32; 2] },
    Circle { diameter_px: f32 },
}

impl Default for ShapeGeometry {
    fn default() -> Self {
        Self::Rect { size_px: [0.0, 0.0] }
    }
}

impl ShapeGeometry {
    /// Which user-facing type this geometry is. The many-to-one hop out of the
    /// renderer's taxonomy: all three arms live in one [`StimulusBody::Shape`].
    ///
    /// [`StimulusBody::Shape`]: super::StimulusBody::Shape
    pub fn stimulus_type(&self) -> StimulusType {
        match self {
            Self::Rect { .. } => StimulusType::Rect,
            Self::Ellipse { .. } => StimulusType::Ellipse,
            Self::Circle { .. } => StimulusType::Circle,
        }
    }

    /// The **user-facing** type name, as it appears in the config `"type"` tag and
    /// in `WRONG_STIMULUS_TYPE` error messages. Never the internal body name — a
    /// client has never heard of "Shape".
    pub fn type_name(&self) -> &'static str {
        self.stimulus_type().type_name()
    }

    /// Full extents in px — `None` for a circle, which needs only the one number
    /// [`diameter_px`](Self::diameter_px) carries.
    pub fn size_px(&self) -> Option<[f32; 2]> {
        match *self {
            Self::Rect { size_px } | Self::Ellipse { size_px } => Some(size_px),
            Self::Circle { .. } => None,
        }
    }

    /// Diameter in px — `None` for rect and ellipse. A full extent like the
    /// others, so every geometry here is sized by the same convention.
    pub fn diameter_px(&self) -> Option<f32> {
        match *self {
            Self::Circle { diameter_px } => Some(diameter_px),
            Self::Rect { .. } | Self::Ellipse { .. } => None,
        }
    }
}

impl Shape {
    pub fn new(
        pos_px: [f32; 2],
        angle_deg: f32,
        appearance: ShapeAppearance,
        geometry: ShapeGeometry,
    ) -> Self {
        Self {
            transform: Deferred::new(Transform2D { pos_px, angle_deg }),
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
