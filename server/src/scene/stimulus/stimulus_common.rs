use super::stimulus_flags::StimulusFlags;
use crate::scene::deferred::Deferred;

/// Default for [`StimulusCommon::opacity`] — also the serde default, so a
/// hand-written config may leave `opacity` out and get a fully opaque stimulus.
pub(crate) fn opaque() -> Deferred<f32> {
    Deferred::new(1.0)
}

/// The state every stimulus has, whatever it draws and whichever dimension it
/// lives in: whether it is shown, and how transparent it is.
///
/// Held once on [`Stimulus`](super::Stimulus), above the kind, so the shared
/// accessors are field reads rather than a match arm per variant. Adding a
/// stimulus kind does not touch this struct.
///
/// **`transform` is deliberately absent.** A position, an orientation and a
/// scale in world space cannot be a `Vec2` and one angle, so placement lives on
/// the kind — `Transform2D` for the 2-D kinds, [`Transform3D`] for the 3-D ones
/// — and is reached through [`Stimulus::placement`](super::Stimulus::placement).
///
/// Both fields own their own config/runtime split ([`StimulusFlags`] hides
/// `dirty`/`anim_enabled`/`enabled_copy`, [`Deferred`] hides `copy`), so this
/// struct needs no split of its own.
///
/// [`Transform3D`]: super::Transform3D
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StimulusCommon {
    pub flags: StimulusFlags,
    /// Whole-stimulus opacity in `[0, 1]`, applied *on top of* whatever alpha
    /// the stimulus' own colours carry: the effective alpha of any one colour is
    /// `color.a * opacity`. A shape whose fill is half-transparent and whose
    /// outline is opaque keeps that relationship at every opacity — the two
    /// alphas stay independent, and this scales both.
    #[serde(default = "opaque")]
    pub opacity: Deferred<f32>,
}

impl Default for StimulusCommon {
    fn default() -> Self {
        Self {
            flags: StimulusFlags::enabled(true),
            opacity: opaque(),
        }
    }
}

impl StimulusCommon {
    /// Enabled and fully opaque — what every `Create*` command starts from.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn make_copy(&mut self) {
        self.flags.make_copy();
        self.opacity.make_copy();
    }

    pub fn flip(&mut self) {
        self.flags.get_copy();
        self.flags.mark_dirty();
        self.opacity.flip();
    }
}
