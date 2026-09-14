//! The dot field: its state, and the per-frame update that advances it.

use crate::scene::deferred::Deferred;
use crate::scene::stimulus::Transform2D;

use super::dots_params::{
    Aperture, ApertureClip, CoherenceCount, DotsParams, NoiseRule, Region, Reinsertion, SignalRule,
};
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
    /// Each dot's fixed uniform draw in `[0, 1)`, **not** its resolved role. Under
    /// `CoherenceCount::Binomial` a dot carries the signal this frame when its roll
    /// is below the *current* `coherence`, so a live `SetCoherence` moves dots
    /// across the threshold at once — without it, a role drawn at birth would never
    /// change under an infinite lifetime and `SignalRule::Same`, and stepping
    /// coherence would have no visible effect (DOTS-02). Under
    /// `SignalRule::Different` the roll is redrawn every frame. Drawn at every birth
    /// whatever the count rule, so a birth's draw count stays fixed.
    signal_roll: Vec<f32>,
    use_alt_color: Vec<bool>,
    /// **One stream per dot**, not one for the field.
    ///
    /// A single shared stream, walked in index order, would make a dot's
    /// trajectory depend on *when* the field was resized: growing it consumes the
    /// draws the surviving dots would otherwise have taken, so every one of them
    /// moves differently from that frame on. That defeats the property the whole
    /// stimulus rests on. With a stream per dot, dot `i` is a function of the
    /// field seed, `i`, and how long it has lived — and touching any other dot
    /// cannot reach it.
    rng: Vec<DotsRng>,

    // Scratch for `CoherenceCount::Exact` with `SignalRule::Different`, which picks
    // exactly `k` of the live dots afresh every frame. Sized with the other arrays,
    // on the ZMQ thread, so the pick allocates nothing on the render thread.
    /// This frame's role per dot.
    signal_now: Vec<bool>,
    /// A permutation of `0..live`, partially shuffled to choose the signal dots.
    perm: Vec<u32>,
    /// The stream that choice is drawn from — see [`DotsRng::for_roles`].
    role_rng: DotsRng,

    /// Frames advanced since the field was seeded. The sample at frame N is a
    /// function of `(seed, N)`; this is the N.
    frame: u64,
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
            signal_roll: Vec::new(),
            use_alt_color: Vec::new(),
            rng: Vec::new(),
            signal_now: Vec::new(),
            perm: Vec::new(),
            role_rng: DotsRng::new(0),
            frame: 0,
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
    ///
    /// Births the whole *capacity*, not just the live count, so that every slot the
    /// arrays hold is a valid dot. A slot that is merely not live yet is one a
    /// `SetDotCount` can promote at any moment, including at a deferred flip on the
    /// render thread, where there is no opportunity to initialize it.
    pub fn reseed(&mut self) {
        let p = self.params.live;
        self.seeded_with = p.seed;
        self.frame = 0;
        self.role_rng = DotsRng::for_roles(p.seed);
        let capacity = (p.dot_count as usize).max(self.pos_px.len());
        self.resize_arrays(capacity);
        for i in 0..capacity {
            self.rng[i] = DotsRng::for_dot(p.seed, i);
        }
        self.birth_range(0, capacity, &p);
    }

    fn resize_arrays(&mut self, capacity: usize) {
        self.pos_px.resize(capacity, [0.0, 0.0]);
        self.dir_unit.resize(capacity, [1.0, 0.0]);
        self.signal_roll.resize(capacity, 0.0);
        self.use_alt_color.resize(capacity, false);
        self.rng.resize_with(capacity, || DotsRng::new(0));
        self.signal_now.resize(capacity, false);
        self.perm.resize(capacity, 0);
    }

    /// Grow the arrays so `count` dots fit, seeding and birthing the new slots.
    ///
    /// Called from the ZMQ thread when `dot_count` rises. Capacity never shrinks: a
    /// field that went 500 → 100 → 500 would otherwise pay an allocation on the way
    /// back up, possibly at a deferred flip on the render thread.
    ///
    /// This does not disturb a single existing dot. Each new slot is seeded from
    /// `(seed, index)` and drawn from its own stream, so growing the field is
    /// invisible to the dots already in it — which is the point of the per-dot
    /// streams and the reason this may be called mid-trial at all.
    fn ensure_capacity(&mut self, count: usize) {
        if count <= self.pos_px.len() {
            return;
        }
        let p = self.params.live;
        let old = self.pos_px.len();
        self.resize_arrays(count);
        for i in old..count {
            self.rng[i] = DotsRng::for_dot(p.seed, i);
        }
        self.birth_range(old, count, &p);
    }

    /// Birth every dot in `from..to`. Allocation-free: the arrays are already
    /// sized, which is what lets `flip` call it on the render thread.
    fn birth_range(&mut self, from: usize, to: usize, p: &DotsParams) {
        for i in from..to.min(self.pos_px.len()) {
            Self::birth(&mut self.rng[i], &mut self.pos_px, &mut self.dir_unit,
                        &mut self.signal_roll, &mut self.use_alt_color, i, p);
        }
    }

    /// Place dot `i` at a fresh uniform position with a fresh role, direction and
    /// colour, drawing from that dot's own stream.
    ///
    /// All four draws happen unconditionally, even when the parameters make some of
    /// them unused (a noise direction at `coherence = 1`, an alternate colour with
    /// no `dot_color_alt`). A birth therefore consumes a fixed number of outputs
    /// whatever the parameters are, which keeps a dot's stream position a function
    /// of how many frames it has lived rather than of the values it drew.
    ///
    /// The signal/noise role is stored as its raw uniform draw, not resolved
    /// against `coherence` here: the role is re-derived every frame in
    /// [`advance`](Self::advance) so that a live `SetCoherence` takes effect at
    /// once.
    ///
    /// An associated function taking the arrays rather than `&mut self`, because the
    /// callers already hold `&mut` on the field they are walking.
    fn birth(
        rng: &mut DotsRng,
        pos_px: &mut [[f32; 2]],
        dir_unit: &mut [[f32; 2]],
        signal_roll: &mut [f32],
        use_alt_color: &mut [bool],
        i: usize,
        p: &DotsParams,
    ) {
        pos_px[i] = p.field.sample(rng);
        dir_unit[i] = rng.unit_vector();
        signal_roll[i] = rng.f32_01();
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
    /// and N, not that it be computable in closed form. Each dot walks its own
    /// stream, seeded once from `(seed, index)` and advanced only here, and the step
    /// comes from the *nominal* refresh rate. The one thing this costs is seeking: a replay steps from frame
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
        let field = p.field;

        // Which lifetime group is reborn this frame. A dot's group is its index
        // modulo the lifetime, so membership needs no storage — and, more to the
        // point, births are staggered uniformly by construction. Staggering is the
        // single easiest thing to get wrong in an RDK; under this scheme it is not
        // a thing that can be got wrong. It is also the structure the formless
        // dot-field method needs, where a group *is* one lifetime step
        // (`dev/design/RDK_PLAN.md` §8).
        let life = p.dot_lifetime_frames as usize;
        let reborn_group = (life > 0).then(|| (self.frame as usize) % life);

        // An exact count: `k` of the `n` live dots, resolved against the *current*
        // coherence and count every frame, so a live `SetCoherence` or
        // `SetDotCount` takes effect at once.
        //
        // Under `Same` the signal dots are the first `k` by index — PsychoPy's
        // rule. Roles stay fixed while `k` does, and since positions and lifetime
        // groups owe nothing to index order, "the first k" is no spatial or temporal
        // pattern. Under `Different` a partial Fisher–Yates over the field-level role
        // stream picks `k` afresh.
        let exact = p.coherence_count == CoherenceCount::Exact;
        let k = p.exact_signal_count(n);
        if exact && p.signal_rule == SignalRule::Different {
            for (j, slot) in self.perm[..n].iter_mut().enumerate() {
                *slot = j as u32;
            }
            for j in 0..k {
                let r = j + self.role_rng.below((n - j) as u32) as usize;
                self.perm.swap(j, r);
            }
            self.signal_now[..n].fill(false);
            for &i in &self.perm[..k] {
                self.signal_now[i as usize] = true;
            }
        }

        for i in 0..n {
            if reborn_group == Some(i % life.max(1)) {
                Self::birth(&mut self.rng[i], &mut self.pos_px, &mut self.dir_unit,
                            &mut self.signal_roll, &mut self.use_alt_color, i, &p);
                continue;
            }

            let is_signal = match (exact, p.signal_rule) {
                (true, SignalRule::Same) => i < k,
                (true, SignalRule::Different) => self.signal_now[i],
                (false, rule) => {
                    if rule == SignalRule::Different {
                        self.signal_roll[i] = self.rng[i].f32_01();
                    }
                    self.signal_roll[i] < p.coherence
                }
            };
            let dir = if is_signal {
                signal_dir
            } else {
                match p.noise_rule {
                    NoiseRule::Position => {
                        // Not motion at all: the dot reappears somewhere else.
                        self.pos_px[i] = field.sample(&mut self.rng[i]);
                        continue;
                    }
                    NoiseRule::Direction => self.dir_unit[i],
                    NoiseRule::Walk => self.rng[i].unit_vector(),
                }
            };

            let d = [dir[0] * step, dir[1] * step];
            let moved = [self.pos_px[i][0] + d[0], self.pos_px[i][1] + d[1]];
            self.pos_px[i] = if field.contains(moved) {
                moved
            } else {
                let reentered = match p.reinsertion {
                    Reinsertion::Wrap => field.reenter(moved, d),
                    Reinsertion::Respawn => None,
                };
                reentered.unwrap_or_else(|| field.sample(&mut self.rng[i]))
            };
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
        let before = self.live_count();
        self.params.set(deferred, params);
        if !deferred {
            if params.seed != self.seeded_with {
                self.reseed();
            } else {
                self.birth_range(before, self.live_count(), &params);
            }
        }
    }

    /// Set how many dots are live.
    ///
    /// Capacity is taken immediately even in deferred mode — see
    /// [`ensure_capacity`](Self::ensure_capacity) — so the flip that raises the live
    /// count never allocates on the render thread.
    ///
    /// Dots that become live *now* are born now, at a fresh position, rather than
    /// resuming wherever they were when the count last dropped: a dot that appears
    /// is a new dot, and one restored to a position it held ten seconds ago would
    /// be a visible artifact. Dots that will become live at a deferred flip are
    /// born there instead, by [`flip`](Self::flip).
    pub fn set_dot_count(&mut self, deferred: bool, dot_count: u32) {
        self.ensure_capacity(dot_count as usize);
        let before = self.live_count();
        self.with_params(deferred, |p| p.dot_count = dot_count);
        if !deferred {
            let p = self.params.live;
            self.birth_range(before, self.live_count(), &p);
        }
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

    /// Set the field. The aperture is left alone, even one that was defaulted from
    /// the old field — it is a region of its own, and only the caller knows whether
    /// it was meant to follow.
    pub fn set_field(&mut self, deferred: bool, field: Region) {
        self.with_params(deferred, |p| p.field = field);
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

    /// Promote the deferred parameters, birthing any dot the new `dot_count` makes
    /// live.
    ///
    /// Runs on the render thread, so it must not allocate — and does not:
    /// `set_dot_count` took the capacity when the command arrived, and
    /// [`birth_range`](Self::birth_range) only writes into slots that already exist.
    /// Without this a raised `dot_count` would flip in dots holding whatever state
    /// they had when the count last dropped.
    pub fn flip(&mut self) {
        self.config.transform.flip();
        let before = self.live_count();
        self.config.params.flip();
        let p = self.params.live;
        self.birth_range(before, self.live_count(), &p);
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
    use super::dots_params::{DotShape, RegionShape};

    let p = d.params.live;
    let a = p.aperture;
    DotsPushConstants {
        screen_half: [screen_w * 0.5, screen_h * 0.5],
        field_center_px: d.transform.live.pos_px,
        aperture_offset_px: a.region.offset_px,
        aperture_half: [a.region.size_px[0] * 0.5, a.region.size_px[1] * 0.5],
        dot_radius_px: p.dot_size_px * 0.5,
        dot_shape: match p.dot_shape {
            DotShape::Round => 0,
            DotShape::Square => 1,
            DotShape::RoundSmooth => 2,
        },
        aperture_shape: match a.region.shape {
            RegionShape::Rect => 0,
            RegionShape::Ellipse => 1,
        },
        aperture_invert: u32::from(a.invert),
        // Under `DotCenter` the CPU has already dropped the dots that fail the
        // test, and the ones that pass are meant to overhang the edge uncut.
        clip_per_pixel: u32::from(a.clip == ApertureClip::Pixel),
        global_opacity: opacity,
        pixel_snap: u32::from(p.pixel_snap),
        _pad: 0,
        dot_color: p.dot_color.into(),
        alt_color: p.dot_color_alt.unwrap_or(p.dot_color).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::dots_params::{Aperture, DotShape, Region};
    use super::build_dots_push_constants;

    const HZ: f32 = 60.0;

    fn field(params: DotsParams) -> Dots {
        Dots::new([0.0, 0.0], 0.0, params)
    }

    fn params(f: impl FnOnce(&mut DotsParams)) -> DotsParams {
        let mut p = DotsParams { dot_count: 64, field: Region::rect([800.0, 600.0]), ..Default::default() };
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

    /// A live `SetCoherence` changes how the field moves *now* — even with an
    /// infinite lifetime and `SignalRule::Same`, where no dot is ever reborn. The
    /// role is re-derived from the current coherence every frame, so stepping it
    /// down turns signal dots into noise dots on the spot (DOTS-02).
    #[test]
    fn set_coherence_takes_effect_without_a_rebirth() {
        let signal_fraction = |d: &mut Dots| {
            let before = d.positions().to_vec();
            d.advance(HZ);
            before
                .iter()
                .zip(d.positions())
                .filter(|(b, a)| (a[0] - b[0] - 1.0).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3)
                .count() as f32
                / before.len() as f32
        };
        let mut d = field(params(|p| {
            p.dot_count = 4000;
            p.coherence = 1.0;
            p.direction_deg = 0.0;
            p.speed_px_per_s = 60.0;
            p.noise_rule = NoiseRule::Direction;
            p.dot_lifetime_frames = 0; // infinite — nothing is ever reborn
            p.signal_rule = SignalRule::Same;
        }));
        assert!((signal_fraction(&mut d) - 1.0).abs() < 0.01);

        d.set_coherence(false, 0.25);
        assert!((signal_fraction(&mut d) - 0.25).abs() < 0.03, "coherence change was ignored");

        d.set_coherence(false, 0.0);
        assert!(signal_fraction(&mut d) < 0.02, "still moving coherently at coherence 0");
    }

    /// Dots that moved exactly one coherent step (+1 px right at 60 px/s, 60 Hz) in
    /// the last frame, by index.
    fn signal_mask(d: &mut Dots) -> Vec<bool> {
        let before = d.positions().to_vec();
        d.advance(HZ);
        before
            .iter()
            .zip(d.positions())
            .map(|(b, a)| (a[0] - b[0] - 1.0).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3)
            .collect()
    }

    fn exact_field(signal_rule: SignalRule) -> Dots {
        field(params(|p| {
            p.dot_count = 100;
            p.coherence = 0.1;
            p.direction_deg = 0.0;
            p.speed_px_per_s = 60.0;
            p.noise_rule = NoiseRule::Walk;
            p.signal_rule = signal_rule;
            p.field = Region::rect([100_000.0, 100_000.0]); // no wraps to muddy the count
        }))
    }

    /// `Exact` is the default, and gives exactly `round(coherence · n)` signal dots on
    /// every frame — 10 of 100, not "about 10". Under `Same` they are the same 10.
    #[test]
    fn exact_coherence_holds_the_count_and_the_roles_under_same() {
        assert_eq!(DotsParams::default().coherence_count, CoherenceCount::Exact);
        let mut d = exact_field(SignalRule::Same);
        let first = signal_mask(&mut d);
        assert_eq!(first.iter().filter(|s| **s).count(), 10);
        for _ in 0..20 {
            assert_eq!(signal_mask(&mut d), first, "a signal dot changed role under Same");
        }
    }

    /// Under `Different` the count is still exact, but which dots carry it changes.
    #[test]
    fn exact_coherence_holds_the_count_but_redraws_roles_under_different() {
        let mut d = exact_field(SignalRule::Different);
        let masks: Vec<Vec<bool>> = (0..20).map(|_| signal_mask(&mut d)).collect();
        for m in &masks {
            assert_eq!(m.iter().filter(|s| **s).count(), 10, "the signal count drifted");
        }
        assert!(masks.windows(2).any(|w| w[0] != w[1]), "roles never changed under Different");
    }

    /// `Binomial` is the other rule, and its count does move.
    #[test]
    fn binomial_coherence_varies_the_count() {
        let mut d = exact_field(SignalRule::Different);
        d.config.params.live.coherence_count = CoherenceCount::Binomial;
        let counts: Vec<usize> =
            (0..30).map(|_| signal_mask(&mut d).iter().filter(|s| **s).count()).collect();
        assert!(counts.iter().any(|c| *c != counts[0]), "binomial count never varied: {counts:?}");
    }

    /// A live `SetCoherence` changes an exact count at once, even under `Same` with an
    /// infinite lifetime (DOTS-02).
    #[test]
    fn exact_coherence_follows_a_live_change() {
        let mut d = exact_field(SignalRule::Same);
        d.set_coherence(false, 0.37);
        assert_eq!(signal_mask(&mut d).iter().filter(|s| **s).count(), 37);
    }

    /// The same seed reproduces the exact-`Different` role choice, which comes from
    /// the field-level stream rather than the per-dot ones.
    #[test]
    fn exact_different_roles_replay_from_the_seed() {
        let run = || {
            let mut d = exact_field(SignalRule::Different);
            (0..10).map(|_| signal_mask(&mut d)).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    /// An ellipse field holds every dot inside it — at birth, and after every way a
    /// dot can leave or be placed: wrapping, respawning, position noise, rebirth.
    #[test]
    fn an_ellipse_field_holds_its_dots_under_every_rule() {
        let e = Region { offset_px: [40.0, -25.0], ..Region::ellipse([500.0, 300.0]) };
        for (reinsertion, noise_rule) in [
            (Reinsertion::Wrap, NoiseRule::Direction),
            (Reinsertion::Respawn, NoiseRule::Walk),
            (Reinsertion::Wrap, NoiseRule::Position),
        ] {
            let mut d = field(params(|p| {
                p.dot_count = 500;
                p.field = e;
                p.coherence = 0.5;
                p.speed_px_per_s = 1500.0;
                p.dot_lifetime_frames = 7;
                p.reinsertion = reinsertion;
                p.noise_rule = noise_rule;
            }));
            for f in 0..300 {
                for p in d.positions() {
                    assert!(e.contains(*p), "{reinsertion:?}/{noise_rule:?}: {p:?} outside at frame {f}");
                }
                d.advance(HZ);
            }
        }
    }

    /// Wrapping an ellipse field is continuous motion plus re-entry, not a respawn:
    /// with coherent dots, no lifetime and `Wrap`, every dot either moved by exactly
    /// one step or re-entered on the far side of its chord along the same line.
    #[test]
    fn an_ellipse_field_wraps_along_the_line_of_motion() {
        let mut d = field(params(|p| {
            p.dot_count = 300;
            p.field = Region::ellipse([400.0, 400.0]);
            p.coherence = 1.0;
            p.direction_deg = 0.0;
            p.speed_px_per_s = 600.0; // 10 px per frame
        }));
        for _ in 0..120 {
            let before = d.positions().to_vec();
            d.advance(HZ);
            for (b, a) in before.iter().zip(d.positions()) {
                assert!((a[1] - b[1]).abs() < 1e-3, "a wrap left the line of motion: {b:?} → {a:?}");
                let stepped = (a[0] - b[0] - 10.0).abs() < 1e-3;
                let reentered = a[0] < 0.0 && b[0] > 0.0;
                assert!(stepped || reentered, "{b:?} → {a:?}");
            }
        }
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
            p.aperture = Aperture { region: Region::ellipse([200.0, 200.0]), ..Default::default() };
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
        let inside = Aperture { region: Region::ellipse([200.0, 200.0]), ..Default::default() };
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
                region: Region::ellipse([50.0, 50.0]),
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

    /// **Growing the field must not disturb the dots already in it.**
    ///
    /// This is what the per-dot RNG streams buy. Under one shared stream walked in
    /// index order, birthing the new dots consumed the draws the existing ones
    /// would have taken, so every dot in the field moved differently from that
    /// frame on — and the sample stopped being a function of the seed and the frame
    /// index, becoming a function of when a `SetDotCount` happened to arrive.
    #[test]
    fn growing_the_field_does_not_move_the_dots_already_in_it() {
        let run = |grow_at: Option<usize>| {
            let mut d = field(params(|p| {
                p.dot_count = 50;
                p.coherence = 0.5; // noise dots draw per frame — the sensitive case
                p.noise_rule = NoiseRule::Walk;
                // An exact count couples the dots: growing the field changes `k`, and
                // with it some existing dots' roles. Only binomial roles are per-dot.
                p.coherence_count = CoherenceCount::Binomial;
                p.seed = 21;
            }));
            for f in 0..20 {
                if grow_at == Some(f) {
                    d.set_dot_count(false, 500);
                }
                d.advance(HZ);
            }
            d.positions()[..50].to_vec()
        };
        assert_eq!(
            run(None),
            run(Some(10)),
            "a mid-trial SetDotCount moved the dots that were already there"
        );
    }

    /// And the same for a growth that lands at a deferred flip.
    #[test]
    fn a_deferred_growth_does_not_move_the_dots_already_in_it() {
        let run = |grow: bool| {
            let mut d = field(params(|p| {
                p.dot_count = 50;
                p.coherence = 0.5;
                p.noise_rule = NoiseRule::Walk;
                p.coherence_count = CoherenceCount::Binomial;
                p.seed = 22;
            }));
            for f in 0..20 {
                if grow && f == 10 {
                    d.make_copy();
                    d.set_dot_count(true, 500);
                    d.flip();
                }
                d.advance(HZ);
            }
            d.positions()[..50].to_vec()
        };
        assert_eq!(run(false), run(true));
    }

    /// A dot that becomes live is a *new* dot: it appears at a fresh position in
    /// the field, not at whatever stale one it held when the count last dropped.
    #[test]
    fn dots_that_become_live_are_born_not_resumed() {
        let mut d = field(params(|p| { p.dot_count = 200; p.speed_px_per_s = 300.0; }));
        d.set_dot_count(false, 20);
        for _ in 0..60 {
            d.advance(HZ);
        }
        let stale = d.positions().to_vec();
        d.set_dot_count(false, 200);
        let now = d.positions();
        assert_eq!(&now[..20], &stale[..], "the live dots must be left alone");
        assert!(
            now[20..].iter().all(|p| p[0].abs() <= 400.0 && p[1].abs() <= 300.0),
            "a newly live dot must be inside the field"
        );
    }

    /// The same, through a deferred flip — the path with no chance to allocate.
    #[test]
    fn a_deferred_count_rise_births_at_the_flip() {
        let mut d = field(params(|p| p.dot_count = 200));
        d.set_dot_count(false, 20);
        let capacity_before = d.pos_px.len();
        d.make_copy();
        d.set_dot_count(true, 200);
        assert_eq!(d.live_count(), 20, "not yet");
        d.flip();
        assert_eq!(d.live_count(), 200);
        assert_eq!(d.pos_px.len(), capacity_before, "the flip must not allocate");
        // Every dot the flip made live is somewhere sensible, not at the origin
        // default the arrays were sized with.
        let at_origin = d.positions()[20..].iter().filter(|p| **p == [0.0, 0.0]).count();
        assert_eq!(at_origin, 0, "a dot flipped in without being born");
    }

    /// Reseeding births the whole capacity, not just the live count, so a slot that
    /// a later `SetDotCount` promotes is never a slot that was left as zeroes.
    #[test]
    fn reseed_births_the_whole_capacity() {
        let mut d = field(params(|p| p.dot_count = 300));
        d.set_dot_count(false, 10);
        d.set_seed(31); // reseeds with a live count of 10 and a capacity of 300
        d.set_dot_count(false, 300);
        assert!(
            d.positions().iter().all(|p| *p != [0.0, 0.0]),
            "a promoted slot was never born"
        );
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
            p.field = Region::ellipse([700.0, 500.0]);
            p.coherence_count = CoherenceCount::Binomial;
            p.pixel_snap = true;
            p.aperture = Aperture {
                region: Region { offset_px: [120.0, -80.0], ..Region::ellipse([450.0, 300.0]) },
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
            p.aperture = Aperture { region: Region::rect([400.0, 200.0]), ..Default::default() };
        }));
        let pc = build_dots_push_constants(&d, 1.0, 1920.0, 1080.0);
        assert_eq!(pc.screen_half, [960.0, 540.0]);
        assert_eq!(pc.dot_radius_px, 15.0, "dot_size_px is a diameter");
        assert_eq!(pc.aperture_half, [200.0, 100.0]);
        assert_eq!(pc.field_center_px, [100.0, -50.0], "the field centre is the transform");
    }

    /// An ellipse pushes both of its half-extents — the shader normalises by each.
    #[test]
    fn an_ellipse_aperture_pushes_both_half_extents() {
        let d = Dots::new([0.0, 0.0], 0.0, params(|p| {
            p.aperture = Aperture { region: Region::ellipse([900.0, 300.0]), ..Default::default() };
        }));
        let pc = build_dots_push_constants(&d, 1.0, 1920.0, 1080.0);
        assert_eq!(pc.aperture_half, [450.0, 150.0]);
        assert_eq!(pc.aperture_shape, 1);
    }

    #[test]
    fn dot_shape_and_snap_reach_the_shader() {
        let d = Dots::new([0.0, 0.0], 0.0, params(|p| {
            p.dot_shape = DotShape::RoundSmooth;
            p.pixel_snap = true;
        }));
        let pc = build_dots_push_constants(&d, 1.0, 800.0, 600.0);
        assert_eq!((pc.dot_shape, pc.pixel_snap), (2, 1));
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
