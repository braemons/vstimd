use super::stimulus_flags::StimulusFlags;
use super::transform2d::Transform2D;
use crate::scene::deferred::Deferred;

/// Default for [`StimulusCommon::opacity`] — also the serde default, so a
/// hand-written config may leave `opacity` out and get a fully opaque stimulus.
fn opaque() -> Deferred<f32> {
    Deferred::new(1.0)
}

/// The state every stimulus has, whatever it draws: whether it is shown, where
/// it sits, and how transparent it is.
///
/// Flattened into each stimulus' serialization, so the config JSON stays flat
/// (`flags`, `transform`, `opacity`, then the type's own fields) while the
/// sharing is explicit in Rust and reusable in the JSON Schema.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StimulusCommon {
    pub flags: StimulusFlags,
    pub transform: Deferred<Transform2D>,
    /// Whole-stimulus opacity in `[0, 1]`, applied *on top of* whatever alpha
    /// the stimulus' own colours carry: the effective alpha of any one colour is
    /// `color.a * opacity`. A shape whose fill is half-transparent and whose
    /// outline is opaque keeps that relationship at every opacity — the two
    /// alphas stay independent, and this scales both.
    #[serde(default = "opaque")]
    pub opacity: Deferred<f32>,
}

impl StimulusCommon {
    pub fn new(pos: [f32; 2], angle: f32) -> Self {
        Self {
            flags: StimulusFlags::enabled(true),
            transform: Deferred::new(Transform2D { pos, angle }),
            opacity: opaque(),
        }
    }

    pub fn make_copy(&mut self) {
        self.flags.make_copy();
        self.transform.make_copy();
        self.opacity.make_copy();
    }

    pub fn flip(&mut self) {
        self.flags.get_copy();
        self.flags.mark_dirty();
        self.transform.flip();
        self.opacity.flip();
    }
}
