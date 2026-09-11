//! Parameters of a random dot kinematogram.
//!
//! See `dev/design/RDK_PLAN.md` for the design and for the figure-ground stimulus
//! this reproduces.

use crate::Color;

// ── Aperture ──────────────────────────────────────────────────────────────────

/// The shape a dot must be inside — or, with [`Aperture::invert`], outside — to be
/// drawn.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum ApertureShape {
    #[default]
    Rect = 0,
    Circle = 1,
}

/// How the aperture edge cuts a dot.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum ApertureClip {
    /// A dot is drawn whole when its **centre** is inside, and not at all when it is
    /// outside. Dots therefore overhang the edge uncut.
    ///
    /// The default, and the one a motion-defined figure wants: cutting dots at the
    /// boundary draws a crisp outline of the aperture, which is a *static form cue* —
    /// precisely what the stimulus exists to avoid. It is also what the Psychtoolbox
    /// original does, testing `figureMask` at one pixel and then blitting the whole
    /// dot.
    #[default]
    DotCenter = 0,
    /// A dot is cut at the aperture edge, per pixel. What a classic RDK in a hard
    /// circular aperture usually wants, where the aperture is meant to be seen.
    Pixel = 1,
}

/// Where dots are *visible* — a separate thing from the field they live in.
///
/// Conflating the two is invisible for a Newsome-style RDK, where the aperture is
/// the field, and fatal for a figure-ground one: its background dots must fill the
/// screen while being visible only *outside* a circle, and its figure dots only
/// inside the same circle. Hence a mask with its own size, its own offset, and an
/// `invert` flag, sitting over a field that is always a plain rectangle.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Aperture {
    pub shape: ApertureShape,
    /// Full extents, never half-extents (see `CLAUDE.md`): `[width, height]` for
    /// `Rect`, and `[0]` is the **diameter** for `Circle`.
    pub size_px: [f32; 2],
    /// Centre of the aperture relative to the stimulus position, which is also the
    /// centre of the field. Lets one field be masked off-centre — the figure circle
    /// sits on the receptive field, not on the screen centre.
    pub offset_px: [f32; 2],
    /// Draw *outside* the shape instead of inside. This is the whole of "background
    /// dots, everywhere but the figure".
    pub invert: bool,
    pub clip: ApertureClip,
}

/// A rectangle big enough not to mask anything, so that `..Default::default()` on
/// an aperture only overrides what the caller names. A default that cropped the
/// field would silently hide dots — the size that means "no mask" is the field's,
/// and only the caller knows it, so the default here is chosen to be out of the way.
impl Default for Aperture {
    fn default() -> Self {
        Self {
            shape: ApertureShape::Rect,
            size_px: [f32::INFINITY, f32::INFINITY],
            offset_px: [0.0, 0.0],
            invert: false,
            clip: ApertureClip::DotCenter,
        }
    }
}

impl Aperture {
    /// Is a dot centred at `p` (field-local pixels, origin at the field centre)
    /// inside the aperture?
    pub fn contains(&self, p: [f32; 2]) -> bool {
        let dx = p[0] - self.offset_px[0];
        let dy = p[1] - self.offset_px[1];
        let inside = match self.shape {
            ApertureShape::Rect => {
                dx.abs() <= self.size_px[0] * 0.5 && dy.abs() <= self.size_px[1] * 0.5
            }
            // `size_px[0]` is a diameter, so the comparison is against its half.
            ApertureShape::Circle => {
                let r = self.size_px[0] * 0.5;
                dx * dx + dy * dy <= r * r
            }
        };
        inside != self.invert
    }
}

// ── Motion rules ──────────────────────────────────────────────────────────────

/// Is a dot's signal/noise role fixed, or redrawn every frame?
///
/// PsychoPy's `signalDots`. One half of the Scase, Braddick & Raymond (1996)
/// taxonomy; kept orthogonal to [`NoiseRule`] because the two are independent
/// choices and papers report them independently.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum SignalRule {
    /// Roles are fixed for the dot's life. The Psychtoolbox original's rule.
    #[default]
    Same = 0,
    /// Roles are redrawn every frame, so no dot carries the signal for longer than
    /// one frame.
    Different = 1,
}

/// How a noise dot moves. PsychoPy's `noiseDots`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum NoiseRule {
    /// A fresh uniform random position every frame — the dot does not move so much
    /// as reappear.
    Position = 0,
    /// A random but *constant* direction, drawn once at birth. The Psychtoolbox
    /// original's rule.
    #[default]
    Direction = 1,
    /// A fresh random direction every frame, at the same speed as a signal dot.
    Walk = 2,
}

/// What happens to a dot that leaves the field.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Reinsertion {
    /// Re-enter from the opposite edge, keeping density exactly constant.
    ///
    /// The default. With the field separate from the aperture the wrap boundary is
    /// not a visible boundary, so the usual objection to wrapping — an edge cue
    /// where dots reappear — does not apply.
    #[default]
    Wrap = 0,
    /// Reappear at a uniform random position anywhere in the field.
    Respawn = 1,
}

/// A dot's shape.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum DotShape {
    #[default]
    Round = 0,
    /// What Psychtoolbox's `dot_type = 0` gives.
    Square = 1,
}

// ── DotsParams ────────────────────────────────────────────────────────────────

/// Everything about a dot field except the dots themselves.
///
/// `Copy`, because it lives in a [`Deferred`](crate::scene::deferred::Deferred) —
/// which also means nothing here may own a heap allocation.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct DotsParams {
    // ── field ──
    /// Full extents of the rectangle the dots live in and wrap within, centred on
    /// the stimulus position. Invisible: what is *seen* is [`aperture`](Self::aperture).
    pub field_size_px: [f32; 2],
    /// How many dots the field holds. Stored rather than derived from a density,
    /// because this is the number a methods section quotes and the number the config
    /// has to record. Density is `dot_count / area(field_size_px)`, and the Python
    /// client converts.
    pub dot_count: u32,

    // ── aperture ──
    pub aperture: Aperture,

    // ── appearance ──
    /// Dot **diameter** (`CLAUDE.md`: sizes are full extents). Psychtoolbox scripts
    /// specify a radius; the client doubles at the boundary.
    pub dot_size_px: f32,
    pub dot_color: Color,
    /// A second dot colour, assigned to each dot at birth with probability ½.
    ///
    /// This is Psychtoolbox's `bwSameTrial`: black-or-white dots on grey, which
    /// removes the mean-luminance difference that a single-polarity field carries
    /// against its background. `None` gives a single-colour field.
    pub dot_color_alt: Option<Color>,
    pub dot_shape: DotShape,

    // ── motion ──
    /// Direction of coherent motion: CCW degrees, 0° = right, matching
    /// `rotation_deg` and `drift_angle_deg`. Psychtoolbox angles are measured
    /// against a downward Y and negate across (see `dev/design/RDK_PLAN.md` §1.3).
    pub direction_deg: f32,
    /// Per *second*, not per frame: the per-frame step is resolved against the
    /// nominal refresh rate, so the same config moves at the same speed on rigs of
    /// different refresh rates (#120).
    pub speed_px_per_s: f32,
    /// Fraction of dots carrying the coherent direction, `[0, 1]`. Dimensionless.
    pub coherence: f32,
    pub signal_rule: SignalRule,
    pub noise_rule: NoiseRule,
    pub reinsertion: Reinsertion,

    // ── lifetime ──
    /// Frames a dot lives before it is reborn at a fresh position. `0` is infinite
    /// (MWorks' convention, and the Psychtoolbox original's behaviour); PsychoPy
    /// spells infinite as `-1`, which the Python client translates.
    pub dot_lifetime_frames: u32,

    // ── reproducibility ──
    /// The sample is a function of this and the frame index alone. Part of the
    /// config, not drawn at create time and forgotten — replaying a config has to
    /// reproduce the stimulus, not merely one like it.
    pub seed: u64,
}

impl Default for DotsParams {
    fn default() -> Self {
        Self {
            field_size_px: [800.0, 600.0],
            dot_count: 200,
            // Sized to the field: the default field is unmasked, and every dot in
            // it is drawn.
            aperture: Aperture { size_px: [800.0, 600.0], ..Aperture::default() },
            dot_size_px: 6.0,
            dot_color: Color::WHITE,
            dot_color_alt: None,
            dot_shape: DotShape::Round,
            direction_deg: 0.0,
            speed_px_per_s: 100.0,
            coherence: 1.0,
            signal_rule: SignalRule::Same,
            noise_rule: NoiseRule::Direction,
            reinsertion: Reinsertion::Wrap,
            dot_lifetime_frames: 0,
            seed: 0,
        }
    }
}

impl DotsParams {
    /// The unit vector of coherent motion.
    pub fn direction_unit(&self) -> [f32; 2] {
        let r = self.direction_deg.to_radians();
        [r.cos(), r.sin()]
    }

    /// Pixels a dot moves per frame at `nominal_hz`.
    ///
    /// Nominal, never measured: an RDK stepped by a jittering divisor is not a
    /// reproducible stimulus, which is the whole of #120.
    pub fn step_px(&self, nominal_hz: f32) -> f32 {
        if nominal_hz > 0.0 {
            self.speed_px_per_s / nominal_hz
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default aperture masks nothing — `DotsParams::default()` must not
    /// silently hide two thirds of its own dots.
    #[test]
    fn the_default_aperture_covers_the_default_field() {
        let p = DotsParams::default();
        let [hw, hh] = [p.field_size_px[0] * 0.5, p.field_size_px[1] * 0.5];
        for corner in [[hw, hh], [-hw, hh], [hw, -hh], [-hw, -hh]] {
            assert!(p.aperture.contains(corner), "default aperture crops the field at {corner:?}");
        }
    }

    #[test]
    fn circle_aperture_is_sized_by_diameter() {
        let a = Aperture {
            shape: ApertureShape::Circle,
            size_px: [100.0, 100.0],
            ..Default::default()
        };
        // A radius would put 60 inside; a diameter puts it outside.
        assert!(a.contains([49.0, 0.0]));
        assert!(!a.contains([60.0, 0.0]));
    }

    #[test]
    fn invert_is_the_complement() {
        let inside = Aperture {
            shape: ApertureShape::Circle,
            size_px: [100.0, 100.0],
            ..Default::default()
        };
        let outside = Aperture { invert: true, ..inside };
        for p in [[0.0, 0.0], [49.0, 0.0], [60.0, 0.0], [400.0, -300.0]] {
            assert_ne!(inside.contains(p), outside.contains(p), "at {p:?}");
        }
    }

    #[test]
    fn aperture_offset_moves_the_mask_not_the_field() {
        let a = Aperture {
            shape: ApertureShape::Circle,
            size_px: [100.0, 100.0],
            offset_px: [200.0, 0.0],
            ..Default::default()
        };
        assert!(!a.contains([0.0, 0.0]));
        assert!(a.contains([200.0, 0.0]));
    }

    /// 0° is rightward and 90° is up, the same convention as `rotation_deg` — the
    /// Psychtoolbox `3*pi/2` that means "up" ports to 90°, not to 270°.
    #[test]
    fn direction_convention_matches_rotation_deg() {
        let right = DotsParams { direction_deg: 0.0, ..Default::default() }.direction_unit();
        assert!((right[0] - 1.0).abs() < 1e-6 && right[1].abs() < 1e-6);
        let up = DotsParams { direction_deg: 90.0, ..Default::default() }.direction_unit();
        assert!(up[0].abs() < 1e-6 && (up[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn step_is_speed_over_nominal_rate() {
        let p = DotsParams { speed_px_per_s: 120.0, ..Default::default() };
        assert!((p.step_px(60.0) - 2.0).abs() < 1e-6);
        assert_eq!(p.step_px(0.0), 0.0, "a zero refresh rate must not divide");
    }
}
