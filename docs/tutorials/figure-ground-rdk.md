# Tutorial: Figure-ground RDK

**Rebuilds:** `demos/figure_ground_rdk` · **Script:** `client/python/examples/demos/figure_ground_rdk.py`

Two dot fields, one aperture. That is the whole trick behind a figure defined
by motion alone: nothing in any single frame — not density, not colour, not a
drawn edge — tells you where the figure is. Freeze the frame and it vanishes.
Let it run and the circle appears, made of nothing but which way the dots go.

See [Random dot kinematograms](../stimuli/random-dots.md) for the concepts
this tutorial puts to use — the field/aperture split, `invert`, and why
`ApertureClip.DOT_CENTER` matters here specifically.

## 1. Background first

```python
clean_slate(conn)
conn.system.set_background(0.5, 0.5, 0.5)
```

Mid grey, matching the Psychtoolbox original's `backgroundCol = 128`. A dot
field has no mean-luminance argument the way a grating does, but the same
principle applies: pick a background once and let every stimulus in the scene
sit on it.

## 2. One aperture, shared by both fields

```python
from vstimd.stimuli import Aperture, ApertureClip, ApertureShape

figure_circle = Aperture(
    shape=ApertureShape.CIRCLE,
    # height_px is ignored for a circle, but 0 means "the field" — pass the
    # diameter on both axes so the aperture round-trips as the circle it is.
    width_px=500.0,
    height_px=500.0,
    # Dots overhang the boundary uncut, as the MATLAB's centre-pixel test does
    # — cutting them at the edge would draw a crisp circle, a static form cue
    # a motion-defined figure must not have.
    clip=ApertureClip.DOT_CENTER,
)
```

Build the aperture once. The figure and the ground use the *same* one — same
shape, same size, same offset — which is what guarantees they partition the
screen exactly, with no gap and no double coverage. Only `invert` differs
between the two, and that happens at the point each field is created, not
here.

## 3. Shared field parameters

```python
from dataclasses import replace

from vstimd.stimuli import Color, DotsParams

common = DotsParams(
    field_width_px=1920.0, field_height_px=1080.0,
    dot_count=900, dot_size_px=8.0,
    dot_color=Color(1.0, 1.0, 1.0),
    speed_px_per_s=200.0, coherence=1.0,
)
```

Everything the two fields have in common — the field size, how many dots, how
big and what colour, how fast, and fully coherent — lives in one `DotsParams`.
`dataclasses.replace` then produces the two actual fields by overriding only
what must differ.

## 4. The ground and the figure

```python
from vstimd.stimuli import ApertureShape

ground = conn.stimuli.dots.create_dots(
    name="ground",
    params=replace(
        common, aperture=replace(figure_circle, invert=True),
        direction_deg=0.0, seed=1,
    ),
)
figure = conn.stimuli.dots.create_dots(
    name="figure",
    params=replace(common, aperture=figure_circle, direction_deg=90.0, seed=2),
)
```

`invert=True` on the ground's copy of the aperture is the entire mechanism:
visible everywhere *outside* the circle, drifting right (`0°`); the figure is
visible everywhere *inside* the same circle, drifting up (`90°`). Two
independent `seed`s, because these are two separate RNG streams — sharing one
would correlate the fields' noise dots for no reason.

Draw order matters at the boundary, where a dot from one field can overhang
into the other's territory: `ground` is created first, so `figure` draws over
it, exactly as in the reference implementation. Swap the order and the
boundary still looks ragged — just with the other field winning the overlap.

!!! note "Why `ApertureClip.DOT_CENTER` and not `PIXEL`"
    A per-pixel cut would draw the figure's circle as a crisp, static outline
    — visible in a single freeze-frame, which defeats the entire point of a
    motion-defined figure. `DOT_CENTER` tests only a dot's *centre* against the
    aperture and then draws it whole, so the boundary is exactly as ragged as
    the dot positions are, and only visible once the two fields are moving
    differently. See [Random dot kinematograms](../stimuli/random-dots.md#clipping-dot_center-vs-pixel).

## 5. Caption and save

```python
add_explanation(conn, EXPLANATION)
conn.scene_config.save("my_figure_ground_rdk")
```

Both fields are already drifting the instant `create_dots` returns — there is
no animation to arm and nothing further to start. Loading the saved config
reproduces the same two fields, driven by the same two seeds, from frame 0.

## Run it

```console
$ cd client/python
$ uv run examples/demos/figure_ground_rdk.py
Connecting to tcp://localhost:5555 …
Saved as 'my_figure_ground_rdk' — the figure is moving the moment it loads.
```

## Try changing it

- Set both fields to the same `direction_deg` and watch the figure disappear
  — motion is the *only* cue, so removing the direction difference removes the
  figure entirely.
- Drop `coherence` on the figure alone (`conn.stimuli.dots.set_coherence`) and
  watch how much noise the visual system tolerates before the circle stops
  popping out.
- Switch `clip` to `ApertureClip.PIXEL` and compare a single frozen frame
  before and after — the circle you were told not to be able to see.

## The complete script

??? example "`client/python/examples/demos/figure_ground_rdk.py`"

    ```python
    --8<-- "client/python/examples/demos/figure_ground_rdk.py"
    ```

## Next

- **[Random dot kinematograms](../stimuli/random-dots.md)** — the concepts
  page this tutorial builds on, including porting from Psychtoolbox and
  converting to/from degrees of visual angle.
- **[Drifting grating](drifting-grating.md)** — another stimulus that moves
  with no animation and no client in the loop.
