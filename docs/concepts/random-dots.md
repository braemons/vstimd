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

## Porting from Psychtoolbox

Two conversions, both of which fail *silently* — the stimulus still renders, at half
the intended size or mirrored about the horizontal. Do them once, at the boundary,
with the helpers in `vstimd.stimuli`:

**Sizes are radii there and diameters here.** Every size in vstimd is a full extent
(see [Coordinate system](coordinate-system.md)). A Psychtoolbox `dotSize = 1.5` is a
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
