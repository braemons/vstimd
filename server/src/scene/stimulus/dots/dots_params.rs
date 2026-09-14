//! Parameters of a random dot kinematogram.
//!
//! See `dev/design/RDK_PLAN.md` for the design and for the figure-ground stimulus
//! this reproduces.

use crate::Color;

use super::dots_rng::DotsRng;

// ── Region ────────────────────────────────────────────────────────────────────

/// The shape of a [`Region`]. A circle is an ellipse with equal width and height.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum RegionShape {
    #[default]
    Rect = 0,
    Ellipse = 1,
}

/// A shape placed relative to the stimulus position.
///
/// The one shape type both the field and the aperture use, so that a new shape is
/// written once and serves both. Each use asks different things of it: the field
/// [`sample`](Self::sample)s births and [`reenter`](Self::reenter)s dots that left,
/// the aperture only asks whether a point is [`contains`](Self::contains)ed. A
/// shape that cannot do the first two in constant draws is still a valid aperture.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub shape: RegionShape,
    /// Full extents, never half-extents (see `CLAUDE.md`): a circle of radius `r` is
    /// an `Ellipse` of `[2r, 2r]`.
    pub size_px: [f32; 2],
    /// Centre relative to the stimulus position.
    pub offset_px: [f32; 2],
}

impl Region {
    pub const fn rect(size_px: [f32; 2]) -> Self {
        Self { shape: RegionShape::Rect, size_px, offset_px: [0.0, 0.0] }
    }

    pub const fn ellipse(size_px: [f32; 2]) -> Self {
        Self { shape: RegionShape::Ellipse, size_px, offset_px: [0.0, 0.0] }
    }

    fn half(&self) -> [f32; 2] {
        [self.size_px[0] * 0.5, self.size_px[1] * 0.5]
    }

    /// Is `p` (pixels relative to the stimulus position) inside? The boundary counts
    /// as inside, which is also PsychoPy's out-of-bounds rule (`abs(x) > half`).
    pub fn contains(&self, p: [f32; 2]) -> bool {
        let [hw, hh] = self.half();
        let dx = p[0] - self.offset_px[0];
        let dy = p[1] - self.offset_px[1];
        match self.shape {
            RegionShape::Rect => dx.abs() <= hw && dy.abs() <= hh,
            // Normalised to the unit circle. An infinite half-extent (the "hides
            // nothing" aperture) gives 0 there, and a zero one NaN, which is outside.
            RegionShape::Ellipse => {
                let (nx, ny) = (dx / hw, dy / hh);
                nx * nx + ny * ny <= 1.0
            }
        }
    }

    /// A uniformly distributed point inside, drawing exactly two outputs from `rng`
    /// whatever the shape — so a birth consumes a fixed number of draws, and a dot's
    /// stream position stays a function of how long it has lived.
    ///
    /// An ellipse samples the unit disc by `sqrt` of a uniform radius rather than by
    /// rejection, which would consume a variable number of draws.
    pub fn sample(&self, rng: &mut DotsRng) -> [f32; 2] {
        let [hw, hh] = self.half();
        let [ox, oy] = self.offset_px;
        match self.shape {
            RegionShape::Rect => [ox + rng.f32_range(-hw, hw), oy + rng.f32_range(-hh, hh)],
            RegionShape::Ellipse => {
                let r = rng.f32_01().sqrt();
                let theta = rng.f32_01() * std::f32::consts::TAU;
                [ox + hw * r * theta.cos(), oy + hh * r * theta.sin()]
            }
        }
    }

    /// Where a dot that has just left re-enters, holding density constant.
    ///
    /// `p` is the position outside and `step` the displacement that took it there.
    /// `None` when there is no sensible answer — a degenerate region, a dot that did
    /// not leave by moving (the field was changed under it), or a line of motion
    /// that misses the region — and the caller respawns instead.
    ///
    /// - `Rect` folds each axis back by one period: the field is a torus. A dot that
    ///   left by more than a whole field in one frame is a misconfiguration, not
    ///   something to accommodate, and stays where the one fold puts it.
    /// - `Ellipse` re-enters where the dot's line of motion enters the ellipse, carried
    ///   in by the distance it overshot. For straight-line motion through a convex
    ///   region this keeps a uniform field uniform: the flux out along a chord is the
    ///   flux back in at its other end.
    pub fn reenter(&self, p: [f32; 2], step: [f32; 2]) -> Option<[f32; 2]> {
        let [hw, hh] = self.half();
        if !(hw > 0.0 && hh > 0.0) {
            return None;
        }
        let [ox, oy] = self.offset_px;
        match self.shape {
            RegionShape::Rect => Some([ox + fold(p[0] - ox, hw), oy + fold(p[1] - oy, hh)]),
            RegionShape::Ellipse => {
                // In unit-circle space, solve |q + t·d|² = 1 for the chord's two ends.
                let q = [(p[0] - ox) / hw, (p[1] - oy) / hh];
                let d = [step[0] / hw, step[1] / hh];
                let a = d[0] * d[0] + d[1] * d[1];
                if a <= 0.0 {
                    return None;
                }
                let b = 2.0 * (q[0] * d[0] + q[1] * d[1]);
                let c = q[0] * q[0] + q[1] * q[1] - 1.0;
                let disc = b * b - 4.0 * a * c;
                if disc < 0.0 {
                    return None;
                }
                let sq = disc.sqrt();
                let t_in = (-b - sq) / (2.0 * a);
                let t_out = (-b + sq) / (2.0 * a);
                // The exit is behind the dot, or it did not leave by this step.
                if t_out > 0.0 {
                    return None;
                }
                let overshoot = -t_out;
                // Never past the far end of the chord, and pulled a hair inside so
                // rounding cannot leave the result on the wrong side of the boundary.
                let t = (t_in + overshoot).min(t_out);
                let t = t_in + (t - t_in).max((t_out - t_in) * 1e-4);
                let r = [q[0] + t * d[0], q[1] + t * d[1]];
                let out = [ox + r[0] * hw, oy + r[1] * hh];
                self.contains(out).then_some(out)
            }
        }
    }
}

/// Fold `v` back into `[-half, half]` by one period.
fn fold(v: f32, half: f32) -> f32 {
    let span = half * 2.0;
    if v > half {
        v - span
    } else if v < -half {
        v + span
    } else {
        v
    }
}

// ── Aperture ──────────────────────────────────────────────────────────────────

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
/// inside the same circle. Hence a mask with its own region and an `invert` flag,
/// over a field that is a region of its own.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Aperture {
    pub region: Region,
    /// Draw *outside* the region instead of inside. This is the whole of "background
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
            region: Region::rect([f32::INFINITY, f32::INFINITY]),
            invert: false,
            clip: ApertureClip::DotCenter,
        }
    }
}

impl Aperture {
    /// The aperture that shows exactly the field.
    pub fn of_field(field: Region) -> Self {
        Self { region: field, ..Self::default() }
    }

    /// Is a dot centred at `p` (pixels relative to the stimulus position) visible?
    pub fn contains(&self, p: [f32; 2]) -> bool {
        self.region.contains(p) != self.invert
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
    /// Roles are fixed. The Psychtoolbox original's rule.
    #[default]
    Same = 0,
    /// Roles are redrawn every frame, so no dot carries the signal for longer than
    /// one frame.
    Different = 1,
}

/// How many dots carry the signal.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum CoherenceCount {
    /// Exactly `round(coherence · dot_count)` dots, rounding half to even — the rule
    /// PsychoPy's `DotStim` uses. At 100 dots and coherence 0.1 that is 10 signal
    /// dots on every frame; a methods section that says "10% coherence" means this.
    #[default]
    Exact = 0,
    /// Each dot independently with probability `coherence`, so the signal count
    /// varies binomially from frame to frame — and each dot's role is a function of
    /// its own stream alone, which `Exact` cannot offer: counting couples the dots.
    Binomial = 1,
}

/// How a noise dot moves. PsychoPy's `noiseDots`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum NoiseRule {
    /// A fresh uniform random position in the field every frame — the dot does not
    /// move so much as reappear.
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
    /// Re-enter from the opposite side, keeping density constant — see
    /// [`Region::reenter`] for what "opposite" means per shape.
    ///
    /// The default. With the field separate from the aperture the wrap boundary is
    /// not a visible boundary, so the usual objection to wrapping — an edge cue
    /// where dots reappear — does not apply.
    #[default]
    Wrap = 0,
    /// Reappear at a uniform random position anywhere in the field. PsychoPy's rule.
    Respawn = 1,
}

/// A dot's shape.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum DotShape {
    /// A hard-edged disc: a pixel is drawn when its centre is within the radius.
    #[default]
    Round = 0,
    /// What Psychtoolbox's `dot_type = 0` and PsychoPy's `DotStim` give.
    Square = 1,
    /// A disc with a one-pixel anti-aliased edge: Psychtoolbox's `dot_type` 1–3.
    RoundSmooth = 2,
}

// ── DotsParams ────────────────────────────────────────────────────────────────

/// Everything about a dot field except the dots themselves.
///
/// `Copy`, because it lives in a [`Deferred`](crate::scene::deferred::Deferred) —
/// which also means nothing here may own a heap allocation.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct DotsParams {
    // ── field ──
    /// Where the dots live: where they are born, where they re-enter, and what
    /// `dot_count` counts. Invisible: what is *seen* is [`aperture`](Self::aperture).
    /// An `Ellipse` field is PsychoPy's `fieldShape='circle'`.
    pub field: Region,
    /// How many dots the field holds. Stored rather than derived from a density,
    /// because this is the number a methods section quotes and the number the config
    /// has to record. The Python client converts from a density.
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
    /// Centre each dot on the nearest pixel centre and draw only the pixels strictly
    /// within its radius — a Psychtoolbox script that rounds a position and blits a
    /// `dist < radius` mask. Off, dots sit at sub-pixel positions.
    pub pixel_snap: bool,

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
    pub coherence_count: CoherenceCount,
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
        let field = Region::rect([800.0, 600.0]);
        Self {
            field,
            dot_count: 200,
            // The field itself: the default field is unmasked, and every dot in it
            // is drawn.
            aperture: Aperture::of_field(field),
            dot_size_px: 6.0,
            dot_color: Color::WHITE,
            dot_color_alt: None,
            dot_shape: DotShape::Round,
            pixel_snap: false,
            direction_deg: 0.0,
            speed_px_per_s: 100.0,
            coherence: 1.0,
            coherence_count: CoherenceCount::Exact,
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

    /// How many of `live` dots carry the signal under [`CoherenceCount::Exact`]:
    /// `coherence · live`, rounded half to even as Python's `round` does.
    ///
    /// Computed in `f64` so that a coherence the client already rounded to `k / n`
    /// comes back as exactly `k`.
    pub fn exact_signal_count(&self, live: usize) -> usize {
        let k = (f64::from(self.coherence.clamp(0.0, 1.0)) * live as f64).round_ties_even();
        (k as usize).min(live)
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
        let [hw, hh] = [p.field.size_px[0] * 0.5, p.field.size_px[1] * 0.5];
        for corner in [[hw, hh], [-hw, hh], [hw, -hh], [-hw, -hh]] {
            assert!(p.aperture.contains(corner), "default aperture crops the field at {corner:?}");
        }
    }

    #[test]
    fn a_circle_is_an_ellipse_sized_by_diameter() {
        let a = Aperture { region: Region::ellipse([100.0, 100.0]), ..Default::default() };
        // A radius would put 60 inside; a diameter puts it outside.
        assert!(a.contains([49.0, 0.0]));
        assert!(!a.contains([60.0, 0.0]));
    }

    /// An ellipse is not a circle of its width: PsychoPy's `fieldShape='circle'` with
    /// `fieldSize=(w, h)` is one, and the old diameter-only circle could not say so.
    #[test]
    fn an_ellipse_uses_both_extents() {
        let e = Region::ellipse([200.0, 100.0]);
        assert!(e.contains([99.0, 0.0]));
        assert!(!e.contains([0.0, 60.0]));
        assert!(e.contains([0.0, 49.0]));
    }

    #[test]
    fn invert_is_the_complement() {
        let inside = Aperture { region: Region::ellipse([100.0, 100.0]), ..Default::default() };
        let outside = Aperture { invert: true, ..inside };
        for p in [[0.0, 0.0], [49.0, 0.0], [60.0, 0.0], [400.0, -300.0]] {
            assert_ne!(inside.contains(p), outside.contains(p), "at {p:?}");
        }
    }

    #[test]
    fn region_offset_moves_the_region() {
        let r = Region { offset_px: [200.0, 0.0], ..Region::ellipse([100.0, 100.0]) };
        assert!(!r.contains([0.0, 0.0]));
        assert!(r.contains([200.0, 0.0]));
    }

    /// Samples land inside, and fill an ellipse uniformly — the `sqrt` radius, not a
    /// plain uniform one, which would pile dots up at the centre.
    #[test]
    fn ellipse_samples_are_inside_and_uniform() {
        let e = Region { offset_px: [30.0, -20.0], ..Region::ellipse([400.0, 200.0]) };
        let mut rng = DotsRng::new(1);
        let n = 20_000;
        let mut inner = 0;
        for _ in 0..n {
            let p = e.sample(&mut rng);
            assert!(e.contains(p), "sample {p:?} outside");
            let (nx, ny) = ((p[0] - 30.0) / 200.0, (p[1] + 20.0) / 100.0);
            if nx * nx + ny * ny <= 0.25 {
                inner += 1; // inside half the normalised radius: a quarter of the area
            }
        }
        let frac = inner as f32 / n as f32;
        assert!((frac - 0.25).abs() < 0.02, "inner quarter-area held {frac}, wanted 0.25");
    }

    /// Both shapes consume exactly two draws, so a birth's draw count does not
    /// depend on the field shape.
    #[test]
    fn sampling_consumes_two_draws_for_every_shape() {
        for region in [Region::rect([10.0, 10.0]), Region::ellipse([10.0, 10.0])] {
            let mut a = DotsRng::new(9);
            let mut b = DotsRng::new(9);
            region.sample(&mut a);
            b.next_u32();
            b.next_u32();
            assert_eq!(a.next_u32(), b.next_u32(), "{:?}", region.shape);
        }
    }

    #[test]
    fn a_rect_reenters_on_the_opposite_edge() {
        let r = Region::rect([100.0, 100.0]);
        assert_eq!(r.reenter([53.0, 10.0], [5.0, 0.0]), Some([-47.0, 10.0]));
        assert_eq!(r.reenter([-51.0, -52.0], [-2.0, -3.0]), Some([49.0, 48.0]));
    }

    /// An ellipse re-enters at the far end of the chord along its motion, carried in
    /// by the distance it overshot.
    #[test]
    fn an_ellipse_reenters_at_the_far_end_of_its_chord() {
        let c = Region::ellipse([100.0, 100.0]);
        // Moving right along y = 0, overshot the edge at x = 50 by 3.
        let p = c.reenter([53.0, 0.0], [5.0, 0.0]).unwrap();
        assert!((p[0] + 47.0).abs() < 0.05 && p[1].abs() < 1e-4, "{p:?}");
        // Along a chord off the diameter: y = 30, so the edge is at |x| = 40.
        let p = c.reenter([41.0, 30.0], [2.0, 0.0]).unwrap();
        assert!((p[0] + 39.0).abs() < 0.05 && (p[1] - 30.0).abs() < 1e-4, "{p:?}");
        assert!(c.contains(p));
    }

    /// Re-entry is the density-preserving choice for straight motion: a uniform
    /// ellipse field of coherent dots stays uniform after many laps.
    #[test]
    fn ellipse_reentry_keeps_a_coherent_field_uniform() {
        let e = Region::ellipse([300.0, 200.0]);
        let mut rng = DotsRng::new(4);
        let step = [7.0, 3.0];
        let mut pts: Vec<[f32; 2]> = (0..8000).map(|_| e.sample(&mut rng)).collect();
        for _ in 0..400 {
            for p in &mut pts {
                let q = [p[0] + step[0], p[1] + step[1]];
                *p = if e.contains(q) { q } else { e.reenter(q, step).expect("re-entry") };
            }
        }
        // Uniformity by quadrant and by inner quarter-area.
        let n = pts.len() as f32;
        let quad = pts.iter().filter(|p| p[0] > 0.0 && p[1] > 0.0).count() as f32 / n;
        let inner = pts
            .iter()
            .filter(|p| (p[0] / 150.0).powi(2) + (p[1] / 100.0).powi(2) <= 0.25)
            .count() as f32
            / n;
        assert!((quad - 0.25).abs() < 0.03, "quadrant held {quad}");
        assert!((inner - 0.25).abs() < 0.03, "inner quarter-area held {inner}");
    }

    #[test]
    fn reentry_declines_when_it_has_no_answer() {
        let c = Region::ellipse([100.0, 100.0]);
        assert_eq!(c.reenter([80.0, 0.0], [0.0, 0.0]), None, "did not move");
        assert_eq!(c.reenter([0.0, 80.0], [5.0, 0.0]), None, "the line misses the ellipse");
        assert_eq!(c.reenter([-80.0, 0.0], [5.0, 0.0]), None, "moving toward it, not away");
        assert_eq!(Region::rect([0.0, 10.0]).reenter([1.0, 0.0], [1.0, 0.0]), None);
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

    /// Python's `round` — PsychoPy's — rounds half to even, and a client-rounded
    /// `k / n` comes back as `k` despite `f32`.
    #[test]
    fn exact_signal_count_rounds_like_python() {
        let count = |coherence, n| DotsParams { coherence, ..Default::default() }.exact_signal_count(n);
        assert_eq!(count(0.25, 10), 2, "2.5 rounds to even");
        assert_eq!(count(0.75, 2), 2, "1.5 rounds to even");
        assert_eq!(count(0.1, 100), 10);
        assert_eq!(count(0.0, 100), 0);
        assert_eq!(count(1.0, 100), 100);
        for n in 1..300usize {
            for k in 0..=n {
                assert_eq!(count(k as f32 / n as f32, n), k, "{k}/{n}");
            }
        }
    }
}
