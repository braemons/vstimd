# Tutorial: Drifting grating

**Rebuilds:** `demo_drifting_grating` · **Script:** `client/python/examples/demos/drifting_grating.py`

One stimulus, one command, and something moves on every single frame forever
without Python being involved again. That is the whole lesson: a grating's drift
is a *property of the stimulus*, advanced by the render thread as it draws, not
an animation and not a loop in your client.

It is also the scene to load when you suspect a timing problem. Motion at a
fixed spatial and temporal frequency makes tearing, a dropped frame, or a
mismatched refresh rate visible to the naked eye in a way a static scene never
does.

## 1. Background first

```python
clean_slate(conn)
conn.system.set_background(0.5, 0.5, 0.5)
```

Mid grey, because a sinusoidal grating modulates around mean luminance. On a
black background the display's average output would jump every time you show or
hide the patch, which is exactly the confound you are trying to avoid in a
visual experiment.

## 2. The grating

```python
from vstimd.stimuli.grating_models import GratingMask, GratingTexture
from vstimd.stimuli.stimuli_models import Vec2

conn.stimuli.grating.create_grating(
    pos=Vec2(0, 0),
    width=2400, height=1400,
    sf=0.01,
    angle=0.0,                      # vertical stripes
    contrast=1.0,
    waveform=GratingTexture.SIN,
    mask=GratingMask.NONE,
    drift_speed=4.0,                # cycles/s
    name="full_field_grating",
)
```

Parameter by parameter:

| Parameter | Here | Meaning |
|---|---|---|
| `width`/`height` | 2400 × 1400 | Full size in px. Deliberately larger than a 1920 × 1080 frame, so no edge of the patch is ever on screen — that is what "full field" means. |
| `sf` | 0.01 | Spatial frequency in **cycles per pixel**: one cycle per 100 px. Not cycles per degree — vstimd does not know your viewing distance. |
| `angle` | 0° | Orientation of the grating. 0° gives vertical stripes. |
| `waveform` | `SIN` | Also `SQR`, `SAW`, `TRI`. |
| `mask` | `NONE` | No aperture — see [Gratings, triggers & a saved config](gratings-triggers-config.md) for a masked patch. |
| `drift_speed` | 4.0 | **Cycles per second**, drifting perpendicular to the stripes. |

At `sf=0.01` and `drift_speed=4.0`, the pattern moves 400 px/s.

!!! note "Cycles per pixel, degrees per what?"
    Everything geometric in vstimd is in pixels, and the conversion to degrees
    of visual angle belongs in your experiment code, where the viewing distance
    is known. Compute `sf` once at the top of your script and the rest stays
    honest.

## 3. Why there is no animation here

`drift_speed` is advanced by the render thread as part of drawing the frame, so
the phase is correct for *this* frame by construction. Nothing is scheduled,
nothing can arrive late, and the drift keeps going with no client connected at
all.

Two related knobs, if you need them:

- `drift_decoupled=True` frees the drift direction from the stripe orientation,
  and `drift_angle` then sets it — a plaid or a component of one.
- `conn.stimuli.grating.set_phase(handle, phase)` sets the phase directly, for
  when *you* want to control it per trial rather than let it run.

## 4. Caption and save

```python
add_explanation(conn, EXPLANATION)
conn.config.save("my_drifting_grating")
```

Because the drift lives in the stimulus, the saved config needs nothing extra:
load it and the grating is moving again immediately.

## Run it

```console
$ cd client/python
$ uv run examples/demos/drifting_grating.py
Connecting to tcp://localhost:5555 …
Saved as 'my_drifting_grating' — it starts drifting the moment it is loaded.
```

## Try changing it

- Change the orientation live and watch the drift direction follow it:
  `conn.stimuli.set_orientation(handle, 45)`.
- Drop `contrast` to 0.1 and check the display still resolves the modulation.
- Set `drift_speed` to half the frame rate in cycles/s at `sf` high enough to
  alias, and watch the motion reverse — a spatial-aliasing demo you get for free.

## Next

- **[Gratings, triggers & a saved config](gratings-triggers-config.md)** — masked gratings that appear on an external trigger.
- **[Photodiode & flicker](photodiode-flicker.md)** — if the motion here looked wrong, measure it.
