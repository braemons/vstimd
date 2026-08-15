use super::super::deferred::Deferred;
use super::shape_appearance::ShapeAppearance;
use super::stimulus_common::StimulusCommon;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RectStimulus {
    #[serde(flatten)]
    pub common: StimulusCommon,
    pub appearance: Deferred<ShapeAppearance>,
    /// Full extents in pixels: `[width, height]` — the same numbers the
    /// command API takes and reports.
    pub size: Deferred<[f32; 2]>,
}

impl RectStimulus {
    pub const TYPE_NAME: &'static str = "Rect";

    pub fn make_copy(&mut self) {
        self.common.make_copy();
        self.appearance.make_copy();
        self.size.make_copy();
    }

    pub fn flip(&mut self) {
        self.common.flip();
        self.appearance.flip();
        self.size.flip();
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EllipseStimulus {
    #[serde(flatten)]
    pub common: StimulusCommon,
    pub appearance: Deferred<ShapeAppearance>,
    /// Full extents in pixels: `[width, height]` (i.e. twice the semi-axes),
    /// matching the command API.
    pub size: Deferred<[f32; 2]>,
}

impl EllipseStimulus {
    pub const TYPE_NAME: &'static str = "Ellipse";

    pub fn make_copy(&mut self) {
        self.common.make_copy();
        self.appearance.make_copy();
        self.size.make_copy();
    }

    pub fn flip(&mut self) {
        self.common.flip();
        self.appearance.flip();
        self.size.flip();
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CircleStimulus {
    #[serde(flatten)]
    pub common: StimulusCommon,
    pub appearance: Deferred<ShapeAppearance>,
    pub radius: Deferred<f32>,
}

impl CircleStimulus {
    pub const TYPE_NAME: &'static str = "Circle";

    pub fn make_copy(&mut self) {
        self.common.make_copy();
        self.appearance.make_copy();
        self.radius.make_copy();
    }

    pub fn flip(&mut self) {
        self.common.flip();
        self.appearance.flip();
        self.radius.flip();
    }
}
