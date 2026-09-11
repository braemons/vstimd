# Gratings

A periodic carrier — sinusoidal by default — inside a rectangular patch, with
an optional aperture mask and an optional drift. It is the workhorse stimulus
of visual physiology, and the one to load when you suspect a timing problem:
motion at a fixed spatial and temporal frequency makes a dropped frame visible
to the naked eye in a way a static scene never does.

```python
from vstimd import Connection
from vstimd.stimuli import GratingMask, GratingParams, GratingTexture, Vec2

with Connection() as conn:
    conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        rotation_deg=0.0,               # vertical stripes
        name="drifting_grating",
        params=GratingParams(
            width_px=400, height_px=400,
            sf_cycles_per_px=0.01,      # one cycle per 100 px
            contrast=1.0,
            waveform=GratingTexture.SIN,
            mask=GratingMask.GAUSS,
            drift_speed_hz=4.0,         # cycles per second
        ),
    )
```

## Parameters

| Field | Default | Meaning |
|---|---|---|
| `width_px` | `0.0` → 200 px | Patch width, before masking. A "full field" grating is simply one larger than the display, so no edge is ever on screen. |
| `height_px` | `0.0` → 200 px | Patch height. |
| `sf_cycles_per_px` | `0.0` → 0.05 | Spatial frequency in **cycles per pixel** — 0.01 is one cycle per 100 px. Not cycles per degree: vstimd does not know your viewing distance, so that conversion belongs at the top of your script, where it does. |
| `phase_cycles` | `0.0` | Phase offset, where `1.0` is one full cycle. Advances by itself every frame whenever `drift_speed_hz != 0`; set it directly only when you are driving phase yourself, per trial. |
| `contrast` | `0.0` → 1.0 | `[0, 1]`, scaling how far the carrier swings between `back_color` and `fore_color` about their mean. At 0 the patch is a flat mean-luminance field. |
| `waveform` | `GratingTexture.SIN` | `SIN`, `SQR`, `SAW`, or `TRI`. |
| `mask` | `GratingMask.NONE` | The aperture over the patch — see [Masks](#masks). |
| `mask_param` | `0.0` → mask-specific default | Only `GAUSS` and `RAISED_COS` read it; see below. |
| `drift_speed_hz` | `0.0` (static) | **Cycles per second.** Negative reverses the direction. |
| `drift_coupled` | `True` | `True`: the drift runs perpendicular to the stripes, following `rotation_deg` as you change it. `False`: `drift_angle_deg` sets the direction independently. |
| `drift_angle_deg` | `0.0` | CCW degrees, read only when `drift_coupled=False`. |
| `fore_color` | white | RGBA at the carrier's `+1` peak. |
| `back_color` | black | RGBA at the carrier's `−1` trough. |

At `sf_cycles_per_px=0.01` and `drift_speed_hz=4.0` the pattern travels
400 px/s: cycles per second × pixels per cycle.

!!! note "Orientation is placement, not a param"
    A grating's `rotation_deg` is the same 2-D placement rotation every other
    stimulus has, set at create time or with `conn.stimuli.set_rotation`. There
    is no `orientation` field on `GratingParams` — the stripes turn because the
    patch turns. At 0° the stripes are vertical.

## Masks

| Mask | Shape | `mask_param` |
|---|---|---|
| `NONE` | The full rectangular patch, hard edges. | unused |
| `CIRCLE` | Hard-edged circle inscribed in the patch. | unused |
| `GAUSS` | Gaussian envelope — a Gabor. | SD in normalised units where the patch radius is 1. Default ⅓. |
| `RAISED_COS` | Tukey window: flat at 1 across the inner portion, raised-cosine taper in the outer fringe. | Fringe proportion in `[0, 1]`. Default 0.2, matching PsychoPy's `mask='raisedCos'` (`fringeWidth=0.2`). |
| `HANN` | Cosine bell, `0.5·(1 + cos(π·r/R))` — tapers from the centre all the way to the edge, with no flat top. | unused (it has no free parameter) |

A tapered mask (`GAUSS`, `RAISED_COS`, `HANN`) is what keeps the patch edge
from being its own broadband stimulus. A hard `CIRCLE` introduces edge
transients at onset and offset — sometimes exactly what you want, often not.

## Drift is the server's job

`drift_speed_hz` is advanced by the render thread as part of drawing each
frame, so the phase is correct for *this* frame by construction. Nothing is
scheduled, nothing can arrive late, and the grating keeps drifting with no
client connected at all. This is why the
[drifting grating demo](../tutorials/drifting-grating.md) needs no animation:
the motion is a property of the stimulus, not a thing being driven.

The same applies to a saved config — load it and the grating is moving
immediately, from phase 0.

## Mutating a live grating

| Setter | Changes |
|---|---|
| `conn.stimuli.grating.set_phase(handle, phase_cycles)` | Phase directly, for per-trial control. |
| `conn.stimuli.grating.set_sf(handle, sf_cycles_per_px)` | Spatial frequency. |
| `conn.stimuli.grating.set_contrast(handle, contrast)` | Contrast — the usual thing to sweep across trials. |
| `conn.stimuli.grating.set_waveform(handle, waveform)` | Carrier profile. |
| `conn.stimuli.grating.set_mask(handle, mask)` | Aperture type. `mask_param` is fixed at create time. |
| `conn.stimuli.grating.set_drift_speed(handle, drift_speed_hz)` | Drift rate; 0 freezes it where it is. |
| `conn.stimuli.grating.set_drift_decoupled(handle, drift_decoupled)` | Frees the drift direction from the stripe orientation — note the setter takes *decoupled*, the inverse of the `drift_coupled` param. |
| `conn.stimuli.grating.set_drift_angle(handle, drift_angle_deg)` | Drift direction, when decoupled. |
| `conn.stimuli.grating.set_fore_color(handle, color)` / `set_back_color(...)` | Peak and trough colours. |
| `conn.stimuli.set_rotation(handle, rotation_deg)` | Stripe orientation — generic, since it is placement. |

## Next

- **[Drifting grating](../tutorials/drifting-grating.md)** — this stimulus built
  from an empty scene, call by call.
- **[Gratings, triggers & a saved config](../tutorials/gratings-triggers-config.md)**
  — masked gratings flashed by an external trigger.
- **[Random dot kinematograms](random-dots.md)** — the other moving stimulus,
  and a very different way of specifying motion.
