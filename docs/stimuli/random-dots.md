# Random dot kinematograms

A dot field — `Dots` — is a set of moving dots in which some fraction carries a
common direction and the rest is noise. It covers the classic motion-discrimination
RDK, and, because the aperture is a separate thing from the field, the
**figure-ground** RDK in which a region is defined by its motion and by nothing else.

```python
from vstimd import Connection
from vstimd.stimuli import DotsParams, Region

with Connection() as conn:
    h = conn.stimuli.dots.create_dots(params=DotsParams(
        field=Region.circle(400),
        dot_count=150, dot_size_px=8,
        direction_deg=0.0, speed_px_per_s=120.0, coherence=0.5,
        seed=1,
    ))
```

## Parameters

`DotsParams` — everything about the field except the dots themselves. As
everywhere else, a numeric field left at `0` means *"use the server's
default"*, with two deliberate exceptions noted below.

| Field | Default | Meaning |
|---|---|---|
| `field` | `None` → an 800 × 600 px `Region.rect` | The [`Region`](#region) dots live in: where they are born, where they re-enter, and what `dot_count` counts. Invisible — see [The field is not the aperture](#the-field-is-not-the-aperture). A zero width or height takes the default's. |
| `dot_count` | `0` → 200 | Number of dots in the field. A count, not a density — it is the number a methods section quotes; `dots_for_density` converts. |
| `aperture` | `Aperture()` — the field itself | Where dots are *visible*. See [Aperture](#aperture) below. |
| `dot_size_px` | `0.0` → 6 px | Dot **diameter**, never a radius. |
| `dot_color` | white | RGBA in 0–1. |
| `dot_color_alt` | `None` | A second colour, assigned per dot at birth with probability ½ — Psychtoolbox's `bwSameTrial`. `None` gives a single-colour field. |
| `dot_shape` | `DotShape.ROUND` | `ROUND` (hard edge), `SQUARE` (Psychtoolbox's `dot_type` 0/4, PsychoPy's `DotStim`), or `ROUND_SMOOTH` (a one-pixel anti-aliased edge — Psychtoolbox's `dot_type` 1–3). |
| `pixel_snap` | `False` | Centre each dot on a pixel centre and draw only the pixels strictly within its radius — see [Pixel-exact dots](#pixel-exact-dots). |
| `direction_deg` | `0.0` | Direction of coherent motion, CCW, 0° = right — the same convention as `rotation_deg`. Psychtoolbox angles are mirrored; see [Porting](#porting-from-psychtoolbox). |
| `speed_px_per_s` | `None` → 100 | Coherent speed, per **second**. Typed `float \| None` because `0.0` is a legitimate value — a static field. |
| `coherence` | `None` → 1.0 | Fraction of dots carrying the coherent direction, `[0, 1]`. Also `float \| None`, because `0.0` means pure noise. Clamped server-side. |
| `coherence_count` | `CoherenceCount.EXACT` | `EXACT`: exactly `round(coherence × dot_count)` signal dots on every frame. `BINOMIAL`: each dot independently. See [Coherence](#coherence-exact-or-binomial). |
| `signal_rule` | `SignalRule.SAME` | Whether a dot's signal/noise role is fixed for its life. See [Motion rules](#motion-rules). |
| `noise_rule` | `NoiseRule.DIRECTION` | How a noise dot moves. |
| `reinsertion` | `Reinsertion.WRAP` | What happens to a dot leaving the field. |
| `dot_lifetime_frames` | `0` (infinite) | Frames before a dot is reborn. PsychoPy spells infinite `-1`; `lifetime_from_psychopy` translates. |
| `seed` | `0` | The RNG seed. **Record it** — see [Reproducibility](#reproducibility). |

There is no `rotation_deg` on a dot field: a field of dots has no orientation of
its own, only a `direction_deg` on its motion.

### Region

One shape type serves both the field and the aperture, so a shape added once is
available to both.

| Field | Default | Meaning |
|---|---|---|
| `shape` | `RegionShape.RECT` | `RECT` or `ELLIPSE`. A circle is an ellipse with equal width and height. |
| `width_px` | `0.0` → the fallback's | Full extent, never a half-extent. |
| `height_px` | `0.0` → the fallback's | Full extent. |
| `offset_px` | `Vec2(0, 0)` | Centre relative to the stimulus position. |

`Region.rect(w, h)`, `Region.ellipse(w, h)` and `Region.circle(diameter)` build
one; each takes an optional `offset_px`.

### Aperture

| Field | Default | Meaning |
|---|---|---|
| `region` | `None` → the field itself | Where dots are visible. A zero width or height takes the field's. |
| `invert` | `False` | Draw *outside* the region instead of inside. This one flag is the whole of "background dots, everywhere but the figure". |
| `clip` | `ApertureClip.DOT_CENTER` | How the edge cuts a dot — see [Clipping](#clipping-dot_center-vs-pixel). |

## The field is not the aperture

Two separate things, and keeping them separate is what makes the second family of
stimulus expressible:

- the **field** is a region where `dot_count` dots live, are born and re-enter. It
  is invisible, and `dot_count` dots are always inside it;
- the **aperture** is a mask over it — a region of its own, with its own shape,
  size and offset, and an `invert` flag. It never moves a dot or changes the count.

For a classic RDK they coincide, and the aperture is best left unset: a circular
field (`field=Region.circle(d)`) holds exactly `dot_count` dots in the circle and
draws every one of them. This is PsychoPy's `fieldShape='circle'`. A circular
*aperture* over a rectangular field would instead show a varying `π/4` of the dots,
and the exact coherence count would no longer be a count of what is visible. For a figure-ground RDK they must not. Its background
dots fill the screen while being visible only *outside* a circle, and its figure
dots only inside the same circle — one field, one aperture, one `invert`:

```python
from dataclasses import replace

circle = Aperture(region=Region.circle(900, offset_px=rf_center))
ground = conn.stimuli.dots.create_dots(params=replace(
    common, aperture=replace(circle, invert=True), direction_deg=0.0, seed=1))
figure = conn.stimuli.dots.create_dots(params=replace(
    common, aperture=circle, direction_deg=90.0, seed=2))
```

The two apertures partition the field exactly, so density is identical inside and
out and nothing but direction distinguishes the regions.

### Clipping: `DOT_CENTER` vs `PIXEL`

`ApertureClip.DOT_CENTER` (the default) draws a dot whole when its **centre** is
inside, so dots overhang the edge uncut. `ApertureClip.PIXEL` cuts them at the edge.

For a motion-defined figure the default is the one you want. Cutting dots at the
boundary draws a crisp outline of the aperture, and a crisp outline is a *static
form cue* — visible in a single freeze-frame, which is precisely what the stimulus
exists to avoid. Use `PIXEL` when the aperture is meant to be seen, as in a classic
RDK behind a hard circular window.

## Appearance

`dot_size_px` is a **diameter**, like every size in vstimd. `dot_shape` is one of:

| `DotShape` | Pixels drawn | Matches |
|---|---|---|
| `ROUND` | those whose centre is within the radius, hard-edged | — |
| `SQUARE` | those whose centre is within the square | Psychtoolbox `dot_type` 0/4; PsychoPy `DotStim` |
| `ROUND_SMOOTH` | a disc whose edge ramps over one pixel, blended | Psychtoolbox `dot_type` 1–3 |

`dot_color` is the field's colour; `dot_color_alt`, left `None` for a single-colour
field, assigns a second colour to each dot at birth with probability ½ —
Psychtoolbox's `bwSameTrial`.

### Pixel-exact dots

`pixel_snap=True` reproduces a Psychtoolbox script that rounds each position to a
pixel and blits a binary mask built as `sqrt(dx² + dy²) < radius`: the dot centre
moves to the centre of the pixel it falls in, and exactly the pixels a whole number
of pixels away and *strictly* within the radius are drawn. The mask has a hard edge
whatever the `dot_shape`, except that `SQUARE` stays square. Motion is still
integrated at sub-pixel precision; only the drawing is snapped.

## Mutating a live field

Every `DotsParams` field a trial might change between draws has a matching
setter on `conn.stimuli.dots`, so a running field can be steered without
recreating it:

| Setter | Changes |
|---|---|
| `set_direction(handle, direction_deg)` | Direction of coherent motion — applied as a change of *velocity* from wherever the dots currently are, not a jump back onto a line through their birth positions. This is what makes a mid-trial direction switch (Psychtoolbox's `noFigureFrames`) expressible. |
| `set_speed(handle, speed_px_per_s)` | Coherent speed. |
| `set_coherence(handle, coherence)` | Fraction of dots carrying the coherent direction. |
| `set_dot_count(handle, dot_count)` | Number of dots in the field. |
| `set_dot_size(handle, dot_size_px)` | Dot diameter. |
| `set_dot_color(handle, color, color_alt=None)` | Both colours together — omitting `color_alt` clears it. |
| `set_aperture(handle, aperture)` | The whole `Aperture`, replaced in one call. |
| `set_field(handle, region)` | The field. A zero width or height keeps the current one; the aperture is left alone. |
| `set_dot_lifetime(handle, dot_lifetime_frames)` | Frames before a dot is reborn. |
| `set_seed(handle, seed)` | Reseeds the field: redraws the sample and restarts it at frame 0. Never deferred — a seed is not a value that can be half-applied. |
| `set_params(handle, params)` | Every parameter at once, including those with no setter of their own (the motion rules, `coherence_count`, `dot_shape`, `pixel_snap`). `params.seed` is ignored — use `set_seed`. Send a complete block, typically a query's with `dataclasses.replace`. |

## Reproducibility

**The sample is a function of `seed` and the frame index alone.** Replaying a saved
config reproduces the stimulus, not merely one like it. Three things follow:

- Record the seed. It is part of the config, not something drawn at create time and
  forgotten. `set_seed` redraws the sample and restarts it at frame 0.
- The per-frame step is `speed_px_per_s` divided by the display's **nominal**
  refresh rate, never the measured one, so a config moves identically on two runs of
  the same rig ([#120](https://github.com/braemons/vstimd/issues/120)).
- Positions are integrated forward from frame 0. There is no seek: a replay steps
  through the frames, exactly as a drifting grating's phase does.

## Motion rules

`signal_rule` × `noise_rule` is the Scase, Braddick & Raymond (1996) taxonomy, the
same one PsychoPy exposes as `signalDots` / `noiseDots`. The six combinations are
not perceptually equivalent and papers differ on which they used, so both are
explicit rather than assumed:

| | |
|---|---|
| `SignalRule.SAME` | a dot's signal/noise role is fixed for its life |
| `SignalRule.DIFFERENT` | roles are redrawn every frame |
| `NoiseRule.POSITION` | a noise dot takes a fresh random position each frame |
| `NoiseRule.DIRECTION` | a noise dot gets a random but *constant* direction |
| `NoiseRule.WALK` | a noise dot re-randomises direction each frame, at signal speed |

`dot_lifetime_frames` is `0` for infinite. Births are staggered uniformly by
construction — a dot's lifetime group is its index modulo the lifetime — so a field
never flickers in lockstep, which is the classic way a hand-rolled RDK goes wrong.

`reinsertion` decides what happens to a dot leaving the field: `WRAP` (the default)
keeps density constant, and because the wrap boundary is not the aperture boundary
it leaks no edge cue. A rectangular field wraps each axis, like a torus; an
elliptical one re-enters the dot where its line of motion enters the ellipse,
carried in by the distance it overshot. `RESPAWN` puts the dot at a fresh uniform
position in the field, which is PsychoPy's rule.

## The statistics of a field

An RDK is a *sample*, and what exactly is being sampled differs between
implementations in ways that move psychophysical thresholds by a factor of two
or more ([Pilly & Seitz, 2009](#literature)). This section states vstimd's
sampling rules exactly, so a methods section can be written from it.

### How a dot is drawn

At birth — which is frame 0 for every dot, and every `dot_lifetime_frames`
thereafter — a dot draws four values from **its own** random stream, in order:

1. a position, uniform over the field — independent in x and y for a rectangle,
   `√u` in radius for an ellipse — from exactly two draws either way;
2. a direction, uniform on `[0, 2π)`, used only if it turns out to be a noise
   dot under `NoiseRule.DIRECTION`;
3. a *signal roll*, uniform on `[0, 1)`;
4. a colour bit, Bernoulli(½), used only if `dot_color_alt` is set.

All four are drawn whether or not the current parameters use them, so the
number of draws a dot has consumed is a function of how many frames it has
lived and nothing else. Each dot has its own stream, seeded from
`(seed, dot index)`, rather than sharing one walked in index order — which is
what makes a dot's trajectory independent of *when* a `set_dot_count` arrived,
and dot *i* a function of `(seed, i, frames lived)` alone.

The generator is PCG-XSH-RR 64/32 seeded through SplitMix64, implemented
in-tree and frozen by a test vector: its output stream is part of the
scene-config format, not an implementation detail, because a config records a
seed and nothing else about the sample.

### Coherence: exact or binomial

Under `CoherenceCount.EXACT` (the default) exactly

    k = round(coherence × dot_count)

dots carry the signal on every frame, rounding half to even as Python's `round`
does — PsychoPy's rule, and what most Psychtoolbox scripts do by shuffling a
fixed-size index set. At 100 dots and 10 % coherence that is 10 signal dots, every
frame. `k` is recomputed from the current values each frame, so `set_coherence` and
`set_dot_count` take effect on the next one.

- Under `SignalRule.SAME` the signal dots are the first `k` by index. Positions and
  lifetime groups owe nothing to index order, so this is no spatial or temporal
  pattern, and the same dots keep the signal while `k` is unchanged.
- Under `SignalRule.DIFFERENT` a fresh `k` of the dots are chosen every frame, by a
  partial Fisher–Yates shuffle over a field-level random stream seeded from `seed`.

An exact count couples the dots: raising `dot_count` changes `k`, and with it the
role of some dots already in the field.

Under `CoherenceCount.BINOMIAL` a dot carries the signal **iff its signal roll is
below the current `coherence`**, so the number of signal dots on a frame is

    n_signal ~ Binomial(dot_count, coherence)

with mean `coherence × dot_count` and SD `√(dot_count · c · (1 − c))` — at 100 dots
and 50 % coherence an SD of 5 percentage points frame to frame. Its one advantage is
that each dot's role depends on its own stream alone, so changing `dot_count` never
changes the role of a dot already in the field.

Under `BINOMIAL`, because the roll is stored raw and re-tested every frame, `SignalRule.SAME`
means the *same subset* keeps carrying the signal for as long as the dots live
(the roll only changes at rebirth), while `SignalRule.DIFFERENT` redraws the
roll each frame, making a dot's role independent across frames.

### What each noise rule is, statistically

| Rule | Per-frame displacement of a noise dot | Displacement after *k* frames |
|---|---|---|
| `POSITION` | none — the dot is *replaced* at a fresh uniform position | uniform over the field, independent of *k*; carries no motion signal at all, only a positional refresh |
| `DIRECTION` | a fixed step of length `speed/refresh` in a direction drawn once at birth | ballistic: `k · step` in a fixed, uniformly random direction |
| `WALK` | a step of length `speed/refresh` in a fresh uniform direction | a 2-D random walk: RMS displacement `step · √k`, mean zero |

Three consequences worth knowing:

- **`POSITION` noise is not "motion noise"** in the motion-energy sense. It
  injects broadband spatiotemporal energy at the moment of replacement rather
  than coherent local motion in a wrong direction, which is exactly why Scase
  et al. treated it as a distinct category rather than a variant.
- **`DIRECTION` noise preserves speed and local motion structure** — every dot
  is a valid moving dot, only some of them point the wrong way. This is the
  closest thing to "signal plus directional noise" and the default here.
- **`WALK` noise decorrelates over time** at fixed speed, so its energy is
  spread over directions within a single dot's trajectory rather than across
  dots.

The vector sum over noise dots is zero in expectation under all three, but its
variance — the accidental net motion a given sample happens to contain — falls
as `1/√n_noise`. At low coherence and low dot counts, a nontrivial fraction of
trials contains a noise field whose accidental drift is comparable to the
signal. Recording the `seed` is what makes that analysable after the fact
rather than an unmodelled source of trial-to-trial variance.

### Density, lifetime and reinsertion

`WRAP` (the default) conserves dot count exactly: no dot is ever created or
destroyed by leaving the field. On a rectangle density is constant by construction;
on an ellipse, re-entry along the line of motion keeps a uniform field uniform for
straight-line motion. `RESPAWN` also conserves count, but
redistributes: a dot leaving one edge reappears anywhere, so the field is
uniform only in expectation and momentarily non-uniform in any one frame.

Because the wrap boundary is the *field* and the visible boundary is the
*aperture*, neither reinsertion rule leaks an edge cue — the wrap happens off
the visible region entirely whenever the aperture is smaller than the field.

With a finite `dot_lifetime_frames`, exactly `dot_count / lifetime` dots are
reborn per frame, because a dot's lifetime group is its index modulo the
lifetime. Births are therefore staggered *deterministically and uniformly*,
not by drawing a random age at initialisation. Uniform staggering is the
single easiest thing to get wrong in a hand-rolled RDK — a field where every
dot is born on the same frame flickers in lockstep at `refresh / lifetime` Hz,
which is both visible and a temporal-frequency artefact in any analysis locked
to stimulus onset.

Finite lifetimes exist to defeat *tracking*: with infinite lifetimes an
observer (or a decoder) can follow one signal dot across many frames and
recover the direction at arbitrarily low coherence, so the task stops
measuring global motion integration and starts measuring attentive tracking
([Britten et al., 1992](#literature); [Braddick, 1974](#literature) for the
correspondence problem this sits inside).

### Aperture statistics

A dot is included by the `DOT_CENTER` test if its **centre** falls inside the
aperture, so the number of dots visible through an aperture of area `A_ap` in a
field of area `A_field` is `Binomial(dot_count, A_ap / A_field)`, and the
*density* inside the aperture equals the field density in expectation. This is
what makes the figure-ground construction work: two complementary apertures
over identically-parameterised fields have equal expected density on both
sides of the boundary, so density carries no information about where the
figure is.

Under `PIXEL` clipping the visible dot *count* is higher (a dot straddling the
boundary contributes on both sides) but the visible dot *area* is conserved.
For a motion-defined figure this is the wrong trade: the cut edge draws the
aperture outline as a static form cue. See
[Clipping](#clipping-dot_center-vs-pixel).

## Porting from Psychtoolbox

Two conversions, both of which fail *silently* — the stimulus still renders, at half
the intended size or mirrored about the horizontal. Do them once, at the boundary,
with the helpers in `vstimd.stimuli`:

**Sizes are radii there and diameters here.** Every size in vstimd is a full extent
(see [Stimuli](index.md#two-conventions-that-hold-everywhere)). A Psychtoolbox
`dotSize = 1.5` is a *radius* — the dot is 3° across — and `R = 45/2` is a circle
45° across.

```python
from vstimd.stimuli import diameter_from_radius
dot_size_px = diameter_from_radius(1.5) * px_per_deg   # 3 deg across
aperture_px = diameter_from_radius(45 / 2) * px_per_deg  # 45 deg across
```

**Directions are mirrored.** Psychtoolbox adds `sin(angle)` to a *row index*, which
grows downward, so its angles run clockwise. vstimd is Y-up and counter-clockwise,
like `rotation_deg`. `3*pi/2` — which is **upward** on a Psychtoolbox screen —
is 90° here, not 270°:

```python
from vstimd.stimuli import direction_from_ptb_rad
direction_from_ptb_rad(3 * math.pi / 2)   # 90.0
```

**Dots are drawn the way the script draws them.** `Screen('DrawDots')` with
`dot_type` 0 or 4 is `DotShape.SQUARE`, 1–3 is `DotShape.ROUND_SMOOTH`. A script
that rounds positions and blits a `dist < radius` mask is `pixel_snap=True` — see
[Pixel-exact dots](#pixel-exact-dots).

From PsychoPy, `dotLife = -1` means infinite, which is `0` here —
`lifetime_from_psychopy` translates. Or use `vstimd.psychopy.visual.DotStim`
directly.

## Degrees of visual angle

vstimd stores pixels; the RDK literature reports deg/s and dots/deg². The conversion
needs the rig geometry, which the server does not carry, so it lives in the client —
where the experimenter knows their viewing distance — and the config records exactly
what was shown:

```python
from vstimd.stimuli import dots_for_density, px_per_deg

ppd = px_per_deg(screen_width_px=1920, screen_width_cm=52.0, viewing_distance_cm=57.0)
dot_count = dots_for_density(1 / 25, field_width_deg=96.0, field_height_deg=54.0)
speed_px_per_s = 50.0 * ppd   # 50 deg/s
```

`dot_count` is the stored parameter rather than a density, because it is the number
a methods section quotes and the number the config has to record.

## A worked example

`client/python/examples/figure_ground_rdk.py` is a complete port of a Psychtoolbox
figure-ground stimulus, including the four protocol conditions, with every
conversion above done at the boundary and commented. `dev/design/RDK_PLAN.md` records
the design and the reading of the original that produced it.

For the smaller, shipped-demo version of the same idea, built up call by call, see
the **[Figure-ground RDK tutorial](../tutorials/figure-ground-rdk.md)**, which rebuilds
`demos/figure_ground_rdk` from an empty scene.

## Other implementations

Worth reading alongside this page, both to check that a port means what you
think it means and because their parameter names are the ones most methods
sections are written in:

| Package | Notes for a vstimd user |
|---|---|
| [PsychoPy `DotStim`](https://psychopy.org/api/visual/dotstim.html) | The closest relative, and drop-in through `vstimd.psychopy.visual.DotStim`, which sets `field` to its `fieldShape`, `dot_shape=SQUARE`, `reinsertion=RESPAWN` and `coherence_count=EXACT`. `signalDots` / `noiseDots` are exactly `signal_rule` / `noise_rule`. Differences: `dotLife = -1` is infinite where vstimd uses `0` (`lifetime_from_psychopy` translates), speeds are per frame there and per second here, and PsychoPy moves dots on each `draw()` where vstimd moves them on each display frame. |
| [MWorks `moving_dots`](https://mworks.github.io/documentation/latest/components/moving_dots.html) | Specifies a **dot density** in dots/deg² and a field *radius*, deriving the count; vstimd stores the count, which `dots_for_density` converts to. Lifetime is in seconds there, frames here. Directions for noise dots are randomised per dot, matching `NoiseRule.DIRECTION`. |
| Psychtoolbox (`DotDemo` and the many lab forks) | No single canonical implementation — each lab's script makes its own choices, which is precisely why `signal_rule`/`noise_rule` are explicit here. Two conversions always apply: radii → diameters, and clockwise/Y-down angles → CCW/Y-up (see [Porting from Psychtoolbox](#porting-from-psychtoolbox)). Aperture handling is typically a one-pixel mask test followed by a whole-dot blit, which is what `ApertureClip.DOT_CENTER` reproduces. |

## Literature

The parameter choices on this page are not arbitrary; these are the sources
they come from, and the ones to cite when reporting a stimulus built with
them.

**The taxonomy**

- Scase, M. O., Braddick, O. J., & Raymond, J. E. (1996). What is noise for the
  motion system? *Vision Research*, 36(16), 2579–2586.
  [doi:10.1016/0042-6989(95)00325-8](https://doi.org/10.1016/0042-6989(95)00325-8)
  — the signal × noise categories that `signal_rule` and `noise_rule` name.

**Why the algorithm matters**

- Pilly, P. K., & Seitz, A. R. (2009). What a difference a parameter makes: A
  psychophysical comparison of random dot motion algorithms. *Vision Research*,
  49(13), 1599–1612.
  [doi:10.1016/j.visres.2009.03.019](https://doi.org/10.1016/j.visres.2009.03.019)
  — measures how much coherence thresholds move between the common algorithms.
  Read this before comparing your thresholds with a paper that used a different
  package.

**The coherence RDK as a task**

- Newsome, W. T., & Paré, E. B. (1988). A selective impairment of motion
  perception following lesions of the middle temporal visual area (MT).
  *Journal of Neuroscience*, 8(6), 2201–2211.
  [doi:10.1523/JNEUROSCI.08-06-02201.1988](https://doi.org/10.1523/JNEUROSCI.08-06-02201.1988)
- Britten, K. H., Shadlen, M. N., Newsome, W. T., & Movshon, J. A. (1992). The
  analysis of visual motion: A comparison of neuronal and psychophysical
  performance. *Journal of Neuroscience*, 12(12), 4745–4765.
  [doi:10.1523/JNEUROSCI.12-12-04745.1992](https://doi.org/10.1523/JNEUROSCI.12-12-04745.1992)
  — the neurometric/psychometric comparison that made coherence the standard
  independent variable, and the reason limited dot lifetimes are used.

**Correspondence and dot lifetime**

- Braddick, O. (1974). A short-range process in apparent motion. *Vision
  Research*, 14(7), 519–527.
  [doi:10.1016/0042-6989(74)90041-8](https://doi.org/10.1016/0042-6989(74)90041-8)
  — the correspondence problem, and `d_max`: a step larger than roughly 15′ of
  arc breaks the short-range process, which puts an upper bound on
  `speed_px_per_s` for a given refresh rate.

**Motion-defined form (the figure-ground case)**

- Regan, D. (1989). Orientation discrimination for objects defined by relative
  motion and objects defined by luminance contrast. *Vision Research*, 29(10),
  1389–1400.
  [doi:10.1016/0042-6989(89)90193-2](https://doi.org/10.1016/0042-6989(89)90193-2)
- Lamme, V. A. F. (1995). The neurophysiology of figure-ground segregation in
  primary visual cortex. *Journal of Neuroscience*, 15(2), 1605–1615.
  [doi:10.1523/JNEUROSCI.15-02-01605.1995](https://doi.org/10.1523/JNEUROSCI.15-02-01605.1995)
  — the paradigm `demos/figure_ground_rdk` implements.

**The random number generator**

- O'Neill, M. E. (2014). *PCG: A family of simple fast space-efficient
  statistically good algorithms for random number generation.* Harvey Mudd
  College technical report HMC-CS-2014-0905.
  [pcg-random.org](https://www.pcg-random.org/)
