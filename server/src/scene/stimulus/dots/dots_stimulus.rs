//! The dot field: its state, and the per-frame update that advances it.

use crate::scene::deferred::Deferred;
use crate::scene::stimulus::Transform2D;

use super::dots_params::{Aperture, ApertureClip, DotsParams, NoiseRule, Reinsertion, SignalRule};
use super::dots_pipeline::{DotInstance, DotsPushConstants};
use super::dots_rng::DotsRng;

// ── DotsConfig ────────────────────────────────────────────────────────────────

/// The dot-field state a config file records.
///
/// Split out from [`Dots`] for the same reason `GratingConfig` is split out of
/// `Grating`: everything below — positions, per-dot roles, the RNG, the frame
/// counter — is render-thread runtime state, and a saved copy of it would describe
/// a session that is over. What the config records is the seed, and the seed is
/// enough: the sample is a function of it and the frame index alone.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DotsConfig {
    pub transform: Deferred<Transform2D>,
    pub params: Deferred<DotsParams>,
}

// ── Dots ──────────────────────────────────────────────────────────────────────

/// A random dot kinematogram.
///
/// Deref/DerefMut give transparent access to the config fields, as `Grating` does.
#[derive(Clone)]
pub struct Dots {
    pub config: DotsConfig,

    // ── runtime ──
    //
    // Parallel arrays rather than a `Vec<Dot>`: the per-frame update walks
    // positions for every dot and the other arrays only on some branches, and the
    // instance write is a straight copy out of `pos_px`.
    //
    // Their length is a *capacity*, which is only ever grown, and only on the ZMQ
    // thread. The number of dots actually live is `params.dot_count`, which may be
    // smaller. That split is what lets `SetDotCount` take effect at a deferred flip
    // without the render thread allocating: growing the arrays happens when the
    // command arrives, and the flip only changes how many of them are used.
    pos_px: Vec<[f32; 2]>,
    dir_unit: Vec<[f32; 2]>,
    is_signal: Vec<bool>,
    use_alt_color: Vec<bool>,

    /// Frames advanced since the field was seeded. The sample at frame N is a
    /// function of `(seed, N)`; this is the N.
    frame: u64,
    rng: DotsRng,
    /// The seed the arrays were built from, so that setting a new one reseeds.
    seeded_with: u64,
}

impl std::ops::Deref for Dots {
    type Target = DotsConfig;
    fn deref(&self) -> &DotsConfig {
        &self.config
    }
}

impl std::ops::DerefMut for Dots {
    fn deref_mut(&mut self) -> &mut DotsConfig {
        &mut self.config
    }
}

/// Serializes as [`DotsConfig`]; the dots themselves are reconstructed from the
/// seed on load.
impl serde::Serialize for Dots {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.config.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for Dots {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let config = DotsConfig::deserialize(d)?;
        Ok(Self::from_config(config))
    }
}

impl Dots {
    pub fn new(pos_px: [f32; 2], angle_deg: f32, params: DotsParams) -> Self {
        Self::from_config(DotsConfig {
            transform: Deferred::new(Transform2D { pos_px, angle_deg }),
            params: Deferred::new(params),
        })
    }

    fn from_config(config: DotsConfig) -> Self {
        let mut s = Self {
            config,
            pos_px: Vec::new(),
            dir_unit: Vec::new(),
            is_signal: Vec::new(),
            use_alt_color: Vec::new(),
            frame: 0,
            rng: DotsRng::new(0),
            seeded_with: 0,
        };
        s.reseed();
        s
    }

    // ── The sample ────────────────────────────────────────────────────────────

    /// Rebuild the whole field from `params.seed`, at frame 0.
    ///
    /// Every path that has to put the stimulus back to its start goes through
    /// here: creation, a config load, a seed change, and `reset_dynamic_state`.
    pub fn reseed(&mut self) {
        let p = self.params.live;
        self.seeded_with = p.seed;
        self.rng = DotsRng::new(p.seed);
        self.frame = 0;
        let n = p.dot_count as usize;
        self.pos_px.resize(n.max(self.pos_px.len()), [0.0, 0.0]);
        self.dir_unit.resize(n.max(self.dir_unit.len()), [1.0, 0.0]);
        self.is_signal.resize(n.max(self.is_signal.len()), true);
        self.use_alt_color.resize(n.max(self.use_alt_color.len()), false);
        for i in 0..n {
            Self::birth(&mut self.rng, &mut self.pos_px, &mut self.dir_unit,
                        &mut self.is_signal, &mut self.use_alt_color, i, &p);
        }
    }

    /// Grow the arrays so `count` dots fit, birthing any that are new.
    ///
    /// Called from the ZMQ thread when `dot_count` rises. Capacity never shrinks:
    /// a field that went 500 → 100 → 500 would otherwise pay an allocation on the
    /// way back up, possibly at a deferred flip on the render thread.
    fn ensure_capacity(&mut self, count: usize) {
        if count <= self.pos_px.len() {
            return;
        }
        let p = self.params.live;
        let old = self.pos_px.len();
        self.pos_px.resize(count, [0.0, 0.0]);
        self.dir_unit.resize(count, [1.0, 0.0]);
        self.is_signal.resize(count, true);
        self.use_alt_color.resize(count, false);
        for i in old..count {
            Self::birth(&mut self.rng, &mut self.pos_px, &mut self.dir_unit,
                        &mut self.is_signal, &mut self.use_alt_color, i, &p);
        }
    }

    /// Place dot `i` at a fresh uniform position with a fresh role, direction and
    /// colour.
    ///
    /// All four draws happen unconditionally, even when the parameters make some of
    /// them unused (a noise direction at `coherence = 1`, an alternate colour with
    /// no `dot_color_alt`). A birth therefore consumes a fixed number of RNG outputs
    /// whatever the parameters are, which is what keeps the stream position a
    /// function of the frame index rather than of the values drawn.
    ///
    /// An associated function taking the arrays rather than `&mut self`, because the
    /// callers already hold `&mut` on the field they are walking.
    fn birth(
        rng: &mut DotsRng,
        pos_px: &mut [[f32; 2]],
        dir_unit: &mut [[f32; 2]],
        is_signal: &mut [bool],
        use_alt_color: &mut [bool],
        i: usize,
        p: &DotsParams,
    ) {
        let hw = p.field_size_px[0] * 0.5;
        let hh = p.field_size_px[1] * 0.5;
        pos_px[i] = [rng.f32_range(-hw, hw), rng.f32_range(-hh, hh)];
        dir_unit[i] = rng.unit_vector();
        is_signal[i] = rng.chance(p.coherence);
        use_alt_color[i] = rng.chance(0.5);
    }

    // ── The per-frame update ──────────────────────────────────────────────────

    /// Advance the field by one frame.
    ///
    /// Positions are integrated rather than recomputed from the frame index. The
    /// closed form is the more obvious way to satisfy #120, but it cannot express a
    /// direction change part-way through a trial — the Psychtoolbox original's
    /// `noFigureFrames`, and any animation targeting `direction_deg` — without
    /// teleporting every dot back onto a line through its birth position.
    /// Integration handles that as what it physically is, a change of velocity.
    ///
    /// Determinism is unaffected: #120 asks that frame N be a function of the config
    /// and N, not that it be computable in closed form. The RNG is seeded once and
    /// advanced only here, in dot-index order, and the step comes from the *nominal*
    /// refresh rate. The one thing this costs is seeking: a replay steps from frame
    /// 0, exactly as the grating's `phase_accum_cycles` already does.
    ///
    /// Allocation-free — the arrays are sized before the render thread ever sees
    /// them.
    pub fn advance(&mut self, nominal_hz: f32) {
        let p = self.params.live;
        let n = (p.dot_count as usize).min(self.pos_px.len());
        if n == 0 {
            return;
        }
        self.frame += 1;

        let step = p.step_px(nominal_hz);
        let signal_dir = p.direction_unit();
        let hw = p.field_size_px[0] * 0.5;
        let hh = p.field_size_px[1] * 0.5;

        // Which lifetime group is reborn this frame. A dot's group is its index
        // modulo the lifetime, so membership needs no storage — and, more to the
        // point, births are staggered uniformly by construction. Staggering is the
        // single easiest thing to get wrong in an RDK; under this scheme it is not
        // a thing that can be got wrong. It is also the structure the formless
        // dot-field method needs, where a group *is* one lifetime step
        // (`dev/design/RDK_PLAN.md` §8).
        let life = p.dot_lifetime_frames as usize;
        let reborn_group = (life > 0).then(|| (self.frame as usize) % life);

        for i in 0..n {
            if reborn_group == Some(i % life.max(1)) {
                Self::birth(&mut self.rng, &mut self.pos_px, &mut self.dir_unit,
                            &mut self.is_signal, &mut self.use_alt_color, i, &p);
                continue;
            }

            if p.signal_rule == SignalRule::Different {
                self.is_signal[i] = self.rng.chance(p.coherence);
            }

            let dir = if self.is_signal[i] {
                signal_dir
            } else {
                match p.noise_rule {
                    NoiseRule::Position => {
                        // Not motion at all: the dot reappears somewhere else.
                        self.pos_px[i] =
                            [self.rng.f32_range(-hw, hw), self.rng.f32_range(-hh, hh)];
                        continue;
                    }
                    NoiseRule::Direction => self.dir_unit[i],
                    NoiseRule::Walk => self.rng.unit_vector(),
                }
            };

            let mut x = self.pos_px[i][0] + dir[0] * step;
            let mut y = self.pos_px[i][1] + dir[1] * step;

            let outside = x < -hw || x > hw || y < -hh || y > hh;
            if outside {
                match p.reinsertion {
                    Reinsertion::Wrap => {
                        x = wrap(x, hw);
                        y = wrap(y, hh);
                    }
                    Reinsertion::Respawn => {
                        x = self.rng.f32_range(-hw, hw);
                        y = self.rng.f32_range(-hh, hh);
                    }
                }
            }
            self.pos_px[i] = [x, y];
        }
    }

    // ── Reading the field ─────────────────────────────────────────────────────

    /// Frames advanced since the last reseed.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// How many dots the field currently holds.
    pub fn live_count(&self) -> usize {
        (self.params.live.dot_count as usize).min(self.pos_px.len())
    }

    /// Positions of every live dot, in field-local pixels with the origin at the
    /// field centre. For tests and the overlay; the render path uses
    /// [`write_instances`](Self::write_instances).
    pub fn positions(&self) -> &[[f32; 2]] {
        &self.pos_px[..self.live_count()]
    }

    /// Write the dots that should be drawn into `out`, returning how many.
    ///
    /// Under `ApertureClip::DotCenter` the aperture test happens here, and a dot
    /// that fails it is simply not emitted — which is also why the count is
    /// returned rather than assumed. Under `ApertureClip::Pixel` every dot is
    /// emitted and the fragment shader does the cutting.
    ///
    /// Writes into a caller-owned buffer rather than returning a `Vec`: this runs on
    /// the render thread, once per frame, and must not allocate.
    pub fn write_instances(&self, out: &mut [DotInstance]) -> u32 {
        let p = self.params.live;
        let by_centre = p.aperture.clip == ApertureClip::DotCenter;
        let mut w = 0usize;
        for i in 0..self.live_count() {
            if w >= out.len() {
                break;
            }
            let pos = self.pos_px[i];
            if by_centre && !p.aperture.contains(pos) {
                continue;
            }
            out[w] = DotInstance {
                pos_px: pos,
                alt_color: if self.use_alt_color[i] && p.dot_color_alt.is_some() {
                    1.0
                } else {
                    0.0
                },
            };
            w += 1;
        }
        w as u32
    }

    // ── Setters ───────────────────────────────────────────────────────────────

    fn with_params(&mut self, deferred: bool, f: impl FnOnce(&mut DotsParams)) {
        let mut next = if deferred { self.params.copy } else { self.params.live };
        f(&mut next);
        self.params.set(deferred, next);
    }

    /// Replace the whole parameter block, reseeding if the seed changed.
    pub fn set_params(&mut self, deferred: bool, params: DotsParams) {
        self.ensure_capacity(params.dot_count as usize);
        self.params.set(deferred, params);
        if !deferred && params.seed != self.seeded_with {
            self.reseed();
        }
    }

    /// Grow the field. Capacity is taken immediately even in deferred mode — see
    /// [`ensure_capacity`](Self::ensure_capacity).
    pub fn set_dot_count(&mut self, deferred: bool, dot_count: u32) {
        self.ensure_capacity(dot_count as usize);
        self.with_params(deferred, |p| p.dot_count = dot_count);
    }

    pub fn set_direction(&mut self, deferred: bool, direction_deg: f32) {
        self.with_params(deferred, |p| p.direction_deg = direction_deg);
    }

    pub fn set_speed(&mut self, deferred: bool, speed_px_per_s: f32) {
        self.with_params(deferred, |p| p.speed_px_per_s = speed_px_per_s);
    }

    pub fn set_coherence(&mut self, deferred: bool, coherence: f32) {
        self.with_params(deferred, |p| p.coherence = coherence.clamp(0.0, 1.0));
    }

    pub fn set_dot_size(&mut self, deferred: bool, dot_size_px: f32) {
        self.with_params(deferred, |p| p.dot_size_px = dot_size_px);
    }

    pub fn set_dot_color(&mut self, deferred: bool, color: crate::Color) {
        self.with_params(deferred, |p| p.dot_color = color);
    }

    pub fn set_dot_color_alt(&mut self, deferred: bool, color: Option<crate::Color>) {
        self.with_params(deferred, |p| p.dot_color_alt = color);
    }

    pub fn set_aperture(&mut self, deferred: bool, aperture: Aperture) {
        self.with_params(deferred, |p| p.aperture = aperture);
    }

    pub fn set_field_size(&mut self, deferred: bool, field_size_px: [f32; 2]) {
        self.with_params(deferred, |p| p.field_size_px = field_size_px);
    }

    pub fn set_dot_lifetime(&mut self, deferred: bool, dot_lifetime_frames: u32) {
        self.with_params(deferred, |p| p.dot_lifetime_frames = dot_lifetime_frames);
    }

    /// Set the seed and redraw the sample from it. Never deferred: a seed is not a
    /// value that can be half-applied — the field it describes either exists or does
    /// not — and a flip that redrew the whole sample would allocate on the render
    /// thread.
    pub fn set_seed(&mut self, seed: u64) {
        self.params.live.seed = seed;
        self.params.copy.seed = seed;
        self.reseed();
    }

    // ── Deferred mode ─────────────────────────────────────────────────────────

    pub fn make_copy(&mut self) {
        self.config.transform.make_copy();
        self.config.params.make_copy();
    }

    pub fn flip(&mut self) {
        self.config.transform.flip();
        self.config.params.flip();
    }
}

/// Fold `v` back into `[-half, half]` by one period. One fold, not a modulo loop:
/// a dot that has left the field has left it by one frame's step, and a step longer
/// than the field is a misconfiguration rather than something to accommodate.
fn wrap(v: f32, half: f32) -> f32 {
    let span = half * 2.0;
    if span <= 0.0 {
        return 0.0;
    }
    if v > half {
        v - span
    } else if v < -half {
        v + span
    } else {
        v
    }
}

// ── Push constants ────────────────────────────────────────────────────────────

/// Build the per-field push constants for one dot stimulus.
///
/// Takes `opacity` as an argument rather than reading it off the field: opacity is
/// shared state living *above* the body, so the dot field has never heard of it.
///
/// This is where full extents become half-extents. The scene, the config and the
/// wire all say `dot_size_px` and `size_px` — full widths, per `CLAUDE.md` — and the
/// halving happens here, at the shader boundary, and nowhere else.
pub fn build_dots_push_constants(
    d: &Dots,
    opacity: f32,
    screen_w: f32,
    screen_h: f32,
) -> DotsPushConstants {
    use super::dots_params::{ApertureShape, DotShape};

    let p = d.params.live;
    let a = p.aperture;
    let aperture_half = match a.shape {
        // A circle is sized by its diameter, so both components are its radius —
        // the shader compares against `.x` and never reads `.y`.
        ApertureShape::Circle => [a.size_px[0] * 0.5, a.size_px[0] * 0.5],
        ApertureShape::Rect => [a.size_px[0] * 0.5, a.size_px[1] * 0.5],
    };
    DotsPushConstants {
        screen_half: [screen_w * 0.5, screen_h * 0.5],
        field_center_px: d.transform.live.pos_px,
        aperture_offset_px: a.offset_px,
        aperture_half,
        dot_radius_px: p.dot_size_px * 0.5,
        dot_shape: match p.dot_shape {
            DotShape::Round => 0,
            DotShape::Square => 1,
        },
        aperture_shape: match a.shape {
            ApertureShape::Rect => 0,
            ApertureShape::Circle => 1,
        },
        aperture_invert: u32::from(a.invert),
        // Under `DotCenter` the CPU has already dropped the dots that fail the
        // test, and the ones that pass are meant to overhang the edge uncut.
        clip_per_pixel: u32::from(a.clip == ApertureClip::Pixel),
        global_opacity: opacity,
        _pad: [0, 0],
        dot_color: p.dot_color.into(),
        alt_color: p.dot_color_alt.unwrap_or(p.dot_color).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::dots_params::{Aperture, ApertureShape, DotShape};
    use super::build_dots_push_constants;

    const HZ: f32 = 60.0;

    fn field(params: DotsParams) -> Dots {
        Dots::new([0.0, 0.0], 0.0, params)
    }

    fn params(f: impl FnOnce(&mut DotsParams)) -> DotsParams {
        let mut p = DotsParams { dot_count: 64, field_size_px: [800.0, 600.0], ..Default::default() };
        f(&mut p);
        p
    }

    /// The property the whole design rests on: the same seed and the same number of
    /// frames give the same dots, down to the bit.
    #[test]
    fn same_seed_reproduces_the_sample() {
        let run = || {
            let mut d = field(params(|p| { p.seed = 42; p.coherence = 0.5; }));
            for _ in 0..120 {
                d.advance(HZ);
            }
            d.positions().to_vec()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn different_seeds_give_different_samples() {
        let sample = |seed| {
            let mut d = field(params(|p| p.seed = seed));
            d.advance(HZ);
            d.positions().to_vec()
        };
        assert_ne!(sample(1), sample(2));
    }

    /// Nothing but the config and the frame index may reach the sample — in
    /// particular not the measured frame rate. Two runs at different *nominal*
    /// rates should differ (they are different rigs), but the same nominal rate
    /// must give the same answer every time.
    #[test]
    fn the_step_comes_from_the_nominal_rate() {
        let at = |hz| {
            let mut d = field(params(|p| { p.speed_px_per_s = 120.0; p.coherence = 1.0; }));
            d.advance(hz);
            d.positions()[0]
        };
        let a = at(60.0);
        assert_eq!(a, at(60.0));
        assert_ne!(a, at(120.0));
    }

    /// Coherent dots move by exactly `speed / nominal_hz` in `direction_deg`.
    #[test]
    fn coherent_dots_step_by_speed_over_rate() {
        let mut d = field(params(|p| {
            p.coherence = 1.0;
            p.direction_deg = 90.0; // up
            p.speed_px_per_s = 120.0;
        }));
        let before = d.positions().to_vec();
        d.advance(HZ);
        let after = d.positions();
        for (b, a) in before.iter().zip(after) {
            assert!((a[0] - b[0]).abs() < 1e-4, "no sideways motion at 90°");
            assert!((a[1] - b[1] - 2.0).abs() < 1e-4, "expected +2 px/frame, got {}", a[1] - b[1]);
        }
    }

    /// A direction change is a change of *velocity*, applied where the dots are —
    /// it does not teleport them back onto a line through their birth positions.
    /// This is what makes Psychtoolbox's `noFigureFrames` expressible.
    #[test]
    fn a_direction_change_does_not_teleport() {
        let mut d = field(params(|p| {
            p.coherence = 1.0;
            p.direction_deg = 0.0;
            p.speed_px_per_s = 60.0;
        }));
        for _ in 0..30 {
            d.advance(HZ);
        }
        let before = d.positions().to_vec();
        d.set_direction(false, 90.0);
        d.advance(HZ);
        for (b, a) in before.iter().zip(d.positions()) {
            let moved = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            assert!(moved < 1.5, "dot jumped {moved} px on a direction change");
        }
    }

    /// Wrapping keeps every dot inside the field, so density is exactly constant.
    #[test]
    fn wrapping_holds_every_dot_in_the_field() {
        let mut d = field(params(|p| {
            p.coherence = 1.0;
            p.speed_px_per_s = 3000.0;
            p.reinsertion = Reinsertion::Wrap;
        }));
        for _ in 0..200 {
            d.advance(HZ);
            for p in d.positions() {
                assert!(p[0].abs() <= 400.0 && p[1].abs() <= 300.0, "escaped at {p:?}");
            }
        }
    }

    /// Births are staggered by construction — a dot's lifetime group is its index
    /// modulo the lifetime — so a field never flickers in lockstep. Over one full
    /// lifetime every dot is reborn exactly once.
    #[test]
    fn lifetime_births_are_staggered() {
        let life = 8u32;
        let mut d = field(params(|p| {
            p.dot_count = 64;
            p.dot_lifetime_frames = life;
            p.speed_px_per_s = 0.0; // isolate rebirth from motion
        }));
        let mut reborn_per_frame = Vec::new();
        for _ in 0..life {
            let before = d.positions().to_vec();
            d.advance(HZ);
            let n = before
                .iter()
                .zip(d.positions())
                .filter(|(b, a)| b != a)
                .count();
            reborn_per_frame.push(n);
        }
        assert!(
            reborn_per_frame.iter().all(|&n| n == 64 / life as usize),
            "births not evenly spread: {reborn_per_frame:?}"
        );
        assert_eq!(reborn_per_frame.iter().sum::<usize>(), 64, "not every dot reborn once");
    }

    /// Infinite lifetime is `0`, and means no dot is ever reborn.
    #[test]
    fn zero_lifetime_means_infinite() {
        let mut d = field(params(|p| {
            p.dot_lifetime_frames = 0;
            p.speed_px_per_s = 0.0;
        }));
        let before = d.positions().to_vec();
        for _ in 0..500 {
            d.advance(HZ);
        }
        assert_eq!(before, d.positions(), "a dot was reborn under an infinite lifetime");
    }

    /// Coherence is the fraction of dots carrying the signal.
    #[test]
    fn coherence_sets_the_signal_fraction() {
        let mut d = field(params(|p| {
            p.dot_count = 4000;
            p.coherence = 0.25;
            p.direction_deg = 0.0;
            p.speed_px_per_s = 60.0;
            p.noise_rule = NoiseRule::Direction;
        }));
        let before = d.positions().to_vec();
        d.advance(HZ);
        // A signal dot moves exactly +1 px in x and 0 in y.
        let signal = before
            .iter()
            .zip(d.positions())
            .filter(|(b, a)| (a[0] - b[0] - 1.0).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3)
            .count();
        let fraction = signal as f32 / 4000.0;
        assert!((fraction - 0.25).abs() < 0.03, "signal fraction {fraction}, wanted ~0.25");
    }

    // ── The aperture ──────────────────────────────────────────────────────────

    fn instances(d: &Dots) -> Vec<DotInstance> {
        let mut buf = vec![DotInstance::default(); d.live_count()];
        let n = d.write_instances(&mut buf) as usize;
        buf.truncate(n);
        buf
    }

    /// Under `DotCenter` the aperture is enforced on the CPU: a dot outside it is
    /// not emitted at all.
    #[test]
    fn dot_center_clipping_culls_whole_dots() {
        let d = field(params(|p| {
            p.dot_count = 2000;
            p.aperture = Aperture {
                shape: ApertureShape::Circle,
                size_px: [200.0, 200.0],
                ..Default::default()
            };
        }));
        let drawn = instances(&d);
        assert!(!drawn.is_empty() && drawn.len() < 2000, "{} of 2000 drawn", drawn.len());
        for i in &drawn {
            let r = (i.pos_px[0].powi(2) + i.pos_px[1].powi(2)).sqrt();
            assert!(r <= 100.0, "drew a dot at r={r} outside a 200 px-diameter aperture");
        }
    }

    /// An inverted aperture draws exactly the dots the upright one does not. This is
    /// the whole of "background dots, everywhere but the figure".
    #[test]
    fn inverted_aperture_is_the_exact_complement() {
        let inside = Aperture {
            shape: ApertureShape::Circle,
            size_px: [200.0, 200.0],
            ..Default::default()
        };
        let make = |a| field(params(|p| { p.dot_count = 1000; p.aperture = a; }));
        let fig = instances(&make(inside));
        let gnd = instances(&make(Aperture { invert: true, ..inside }));
        assert_eq!(fig.len() + gnd.len(), 1000, "a dot was drawn twice or not at all");
    }

    /// Under `Pixel` the CPU emits every dot and the shader does the cutting, so the
    /// instance count is the dot count.
    #[test]
    fn pixel_clipping_emits_every_dot() {
        let d = field(params(|p| {
            p.dot_count = 500;
            p.aperture = Aperture {
                shape: ApertureShape::Circle,
                size_px: [50.0, 50.0],
                clip: ApertureClip::Pixel,
                ..Default::default()
            };
        }));
        assert_eq!(instances(&d).len(), 500);
    }

    /// The second colour is assigned at birth and does not change while the dot
    /// lives — and the flag is only set when there is a second colour to select.
    #[test]
    fn alt_color_is_stable_and_only_used_when_set() {
        let mut d = field(params(|p| { p.dot_count = 400; p.dot_lifetime_frames = 0; }));
        assert!(instances(&d).iter().all(|i| i.alt_color == 0.0), "no alt colour was set");

        d.set_dot_color_alt(false, Some(crate::Color::BLACK));
        let first = instances(&d);
        assert_eq!(first.len(), 400, "the default aperture must not crop the field");
        let alt = first.iter().filter(|i| i.alt_color == 1.0).count();
        assert!((150..250).contains(&alt), "{alt} of 400 took the alt colour, wanted ~200");
        d.advance(HZ);
        let second = instances(&d);
        let flags_a: Vec<f32> = first.iter().map(|i| i.alt_color).collect();
        let flags_b: Vec<f32> = second.iter().map(|i| i.alt_color).collect();
        assert_eq!(flags_a, flags_b, "a dot changed colour without being reborn");
    }

    // ── State management ──────────────────────────────────────────────────────

    #[test]
    fn setting_the_seed_restarts_the_sample() {
        let mut d = field(params(|p| p.seed = 7));
        for _ in 0..50 {
            d.advance(HZ);
        }
        assert_eq!(d.frame(), 50);
        d.set_seed(7);
        assert_eq!(d.frame(), 0, "reseeding must restart at frame 0");
        let fresh = field(params(|p| p.seed = 7));
        assert_eq!(d.positions(), fresh.positions());
    }

    /// Growing the field takes its allocation immediately, even in deferred mode, so
    /// the flip that raises the live count never allocates on the render thread.
    #[test]
    fn deferred_count_growth_allocates_before_the_flip() {
        let mut d = field(params(|p| p.dot_count = 100));
        d.make_copy();
        d.set_dot_count(true, 5000);
        assert_eq!(d.live_count(), 100, "a deferred write must not take effect yet");
        assert_eq!(d.pos_px.len(), 5000, "the allocation must already have happened");
        d.flip();
        assert_eq!(d.live_count(), 5000);
    }

    /// Shrinking and regrowing keeps the capacity, so the second growth is free.
    #[test]
    fn capacity_never_shrinks() {
        let mut d = field(params(|p| p.dot_count = 1000));
        d.set_dot_count(false, 10);
        assert_eq!(d.live_count(), 10);
        assert_eq!(d.pos_px.len(), 1000);
    }

    #[test]
    fn round_trips_through_json() {
        let d = field(params(|p| {
            p.seed = 99;
            p.dot_count = 321;
            p.direction_deg = 45.0;
            p.dot_shape = DotShape::Square;
            p.noise_rule = NoiseRule::Walk;
            p.signal_rule = SignalRule::Different;
            p.reinsertion = Reinsertion::Respawn;
            p.dot_color_alt = Some(crate::Color::BLACK);
            p.aperture = Aperture {
                shape: ApertureShape::Circle,
                size_px: [450.0, 450.0],
                offset_px: [120.0, -80.0],
                invert: true,
                clip: ApertureClip::Pixel,
            };
        }));
        let json = serde_json::to_string(&d).unwrap();
        let back: Dots = serde_json::from_str(&json).unwrap();
        assert_eq!(back.params.live, d.params.live);
        // The dots are not in the file — they are rebuilt from the seed, and must
        // come back identical.
        assert_eq!(back.positions(), d.positions());
        assert_eq!(back.frame(), 0);
    }

    // ── The shader contract ───────────────────────────────────────────────────
    //
    // The push constants are the whole interface between the CPU field and the
    // fragment shader, and they are where full extents become half-extents. A
    // mistake here renders something plausible at the wrong size, so the halving
    // is tested rather than trusted.

    #[test]
    fn push_constants_halve_sizes_exactly_once() {
        let d = Dots::new([100.0, -50.0], 0.0, params(|p| {
            p.dot_size_px = 30.0;
            p.aperture = Aperture {
                shape: ApertureShape::Rect,
                size_px: [400.0, 200.0],
                ..Default::default()
            };
        }));
        let pc = build_dots_push_constants(&d, 1.0, 1920.0, 1080.0);
        assert_eq!(pc.screen_half, [960.0, 540.0]);
        assert_eq!(pc.dot_radius_px, 15.0, "dot_size_px is a diameter");
        assert_eq!(pc.aperture_half, [200.0, 100.0]);
        assert_eq!(pc.field_center_px, [100.0, -50.0], "the field centre is the transform");
    }

    /// A circle is sized by its diameter, so *both* components of `aperture_half`
    /// are its radius — the shader compares against `.x` and never reads `.y`.
    #[test]
    fn a_circle_aperture_pushes_its_radius() {
        let d = Dots::new([0.0, 0.0], 0.0, params(|p| {
            p.aperture = Aperture {
                shape: ApertureShape::Circle,
                size_px: [900.0, 0.0],
                ..Default::default()
            };
        }));
        let pc = build_dots_push_constants(&d, 1.0, 1920.0, 1080.0);
        assert_eq!(pc.aperture_half, [450.0, 450.0]);
        assert_eq!(pc.aperture_shape, 1);
    }

    /// Under `DotCenter` the CPU has already culled, and the dots that survive are
    /// meant to overhang the edge — so the shader's aperture test must be off, or
    /// it would cut exactly the dots the design says to leave whole.
    #[test]
    fn dot_center_clipping_disables_the_shader_test() {
        let make = |clip| {
            let d = Dots::new([0.0, 0.0], 0.0, params(|p| {
                p.aperture = Aperture { clip, ..Default::default() };
            }));
            build_dots_push_constants(&d, 1.0, 800.0, 600.0).clip_per_pixel
        };
        assert_eq!(make(ApertureClip::DotCenter), 0);
        assert_eq!(make(ApertureClip::Pixel), 1);
    }

    /// With no second colour, both colour slots hold the same value, so the
    /// shader's `mix` cannot produce anything unexpected from a stale flag.
    #[test]
    fn one_color_pushes_the_same_color_twice() {
        let d = Dots::new([0.0, 0.0], 0.0, params(|p| {
            p.dot_color = crate::Color::WHITE;
            p.dot_color_alt = None;
        }));
        let pc = build_dots_push_constants(&d, 1.0, 800.0, 600.0);
        assert_eq!(pc.dot_color, pc.alt_color);
    }

    #[test]
    fn opacity_comes_from_above_the_body() {
        let d = Dots::new([0.0, 0.0], 0.0, DotsParams::default());
        assert_eq!(build_dots_push_constants(&d, 0.25, 800.0, 600.0).global_opacity, 0.25);
    }

    /// A config load must not resume a field mid-trial. Advancing then reloading
    /// gives frame 0, not frame 50.
    #[test]
    fn a_loaded_config_starts_at_frame_zero() {
        let mut d = field(params(|p| p.seed = 5));
        for _ in 0..50 {
            d.advance(HZ);
        }
        let back: Dots = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back.frame(), 0);
        assert_eq!(back.positions(), field(params(|p| p.seed = 5)).positions());
    }
}
