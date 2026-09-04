# Random dot kinematograms

A dot field — `Dots` — is a set of moving dots in which some fraction carries a
common direction and the rest is noise. It covers the classic motion-discrimination
RDK, and, because the aperture is a separate thing from the field, the
**figure-ground** RDK in which a region is defined by its motion and by nothing else.

```python
from vstimd import Connection
from vstimd.stimuli import Aperture, ApertureShape, DotsParams

with Connection() as conn:
    h = conn.stimuli.dots.create_dots(params=DotsParams(
        field_width_px=400, field_height_px=400,
        aperture=Aperture(shape=ApertureShape.CIRCLE, width_px=400),
        dot_count=150, dot_size_px=8,
        direction_deg=0.0, speed_px_per_s=120.0, coherence=0.5,
        seed=1,
    ))
```

## The field is not the aperture

Two separate things, and keeping them separate is what makes the second family of
stimulus expressible:

- the **field** is a rectangle (`field_width_px` × `field_height_px`, centred on the
  stimulus position) where `dot_count` dots live and where `reinsertion` acts. It is
  invisible;
- the **aperture** is a mask over it, with its own shape, its own size, its own
  `offset_px` from the field centre, and an `invert` flag.

For a classic RDK they coincide: a circular aperture the same size as the field, and
the two behave as one thing. For a figure-ground RDK they must not. Its background
dots fill the screen while being visible only *outside* a circle, and its figure
dots only inside the same circle — one field, one aperture, one `invert`:

```python
from dataclasses import replace

circle = Aperture(shape=ApertureShape.CIRCLE, width_px=900, offset_px=rf_center)
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

`dot_size_px` is a **diameter**, like every size in vstimd. `dot_shape` is
`DotShape.ROUND` (the default) or `DotShape.SQUARE` — Psychtoolbox's
`dot_type=0`. `dot_color` is the field's colour; `dot_color_alt`, left `None`
for a single-colour field, assigns a second colour to each dot at birth with
probability ½ — Psychtoolbox's `bwSameTrial`.

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
| `set_field_size(handle, width_px, height_px)` | The field rectangle. |
| `set_dot_lifetime(handle, dot_lifetime_frames)` | Frames before a dot is reborn. |
| `set_seed(handle, seed)` | Reseeds the field: redraws the sample and restarts it at frame 0. Never deferred — a seed is not a value that can be half-applied. |

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
keeps density exactly constant, and because the wrap boundary is not the aperture
boundary it leaks no edge cue. `RESPAWN` puts the dot at a fresh random position.

## The statistics of a field

An RDK is a *sample*, and what exactly is being sampled differs between
implementations in ways that move psychophysical thresholds by a factor of two
or more ([Pilly & Seitz, 2009](#literature)). This section states vstimd's
sampling rules exactly, so a methods section can be written from it.

### How a dot is drawn

At birth — which is frame 0 for every dot, and every `dot_lifetime_frames`
thereafter — a dot draws four values from **its own** random stream, in order:

1. a position, uniform over the field rectangle (independent in x and y);
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

### Coherence is a per-dot Bernoulli, not a fixed count

Every frame, a dot carries the signal **iff its signal roll is below the
current `coherence`**. So the number of signal dots on a given frame is

    n_signal ~ Binomial(dot_count, coherence)

with mean `coherence × dot_count` and SD `√(dot_count · c · (1 − c))` — *not*
exactly `round(coherence × dot_count)`, which is what PsychoPy and most
Psychtoolbox scripts produce by shuffling a fixed-size index set.

This matters at small `dot_count`. With 100 dots at 50 % coherence the
realised coherence has an SD of 5 percentage points frame to frame; with 900
dots it is 1.7. If your design needs the count pinned exactly — a
single-interval task where the nominal coherence is the independent variable —
either use enough dots that the binomial spread is small relative to your
coherence steps, or report the nominal value and note the sampling rule. The
choice here is deliberate: a per-dot threshold is what lets `set_coherence`
take effect on the very next frame, moving dots across the threshold *in
place*, which a shuffled fixed-size set cannot do without disturbing dots that
should not have changed role.

Because the roll is stored raw and re-tested every frame, `SignalRule.SAME`
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
destroyed by leaving the field, so density is constant by construction and
identical in every region of the field. `RESPAWN` also conserves count, but
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
(see [Coordinate system](../concepts/coordinate-system.md)). A Psychtoolbox `dotSize = 1.5` is a
*radius* — the dot is 3° across — and `R = 45/2` is a circle 45° across.

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

From PsychoPy, `dotLife = -1` means infinite, which is `0` here —
`lifetime_from_psychopy` translates.

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
| [PsychoPy `DotStim`](https://psychopy.org/api/visual/dotstim.html) | The closest relative: `signalDots` / `noiseDots` are exactly `signal_rule` / `noise_rule`, from the same Scase et al. taxonomy. Differences: `dotLife = -1` is infinite where vstimd uses `0` (`lifetime_from_psychopy` translates), sizes are in the window's units rather than always pixels, and the signal set is a fixed count rather than a per-dot Bernoulli (see [Coherence](#coherence-is-a-per-dot-bernoulli-not-a-fixed-count)). |
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
