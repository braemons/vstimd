# Tutorial: Photodiode & flicker

**Rebuilds:** `demo_photodiode_flicker` · **Script:** `client/python/examples/demos/photodiode_flicker.py`

This is the measurement scene. The corner photodiode patch inverts on every
single frame, so a photodiode taped to that corner reports what the display is
*actually* doing rather than what its EDID claims; a large patch flickering at
5 Hz gives you a visible cross-check you can count by eye.

Building it teaches two things: durations counted in frames, and the one scene
setting that is not a stimulus.

## 1. The visible patch

```python
patch = conn.stimuli.shapes.create_rect(
    position_px=Vec2(0, 100),
    name="flicker_patch",
    params=RectParams(
        width_px=1400,
        height_px=600,
        appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
    ),
)
```

Big and white on a near-black background: you want to see this from across the
room, and a photometer wants a lot of it.

## 2. Flicker, counted in frames

```python
from vstimd import StartAction

flicker = conn.animations.create_flicker(
    patch,
    on_frames=6, off_frames=6,
    start_on_phase=True,
    name="field_flicker_5hz",
    start_action_mask=StartAction.ENABLE,
)
conn.animations.arm(flicker)
```

Six frames on plus six off is a 12-frame period — 5 Hz at 60 Hz, and something
else at any other refresh rate. That is the honest way to specify a flicker: the
display can only change on frame boundaries, so a period that is not a whole
number of frames does not exist. (`on_ms=`/`off_ms=` are available and convert
using the server's measured rate, which is convenient and slightly less
truthful.)

Omitting `total_frames` means it never stops. `StartAction.ENABLE` makes the
first on-phase actually show the patch rather than assuming it is already
visible — worth setting even when the stimulus starts enabled, so the animation
does not depend on the state it inherits.

## 3. The photodiode patch

The corner patch is a **scene setting**, not a stimulus: the renderer draws it
itself, from `scene.photodiode`, so it has no handle and cannot be moved or
deleted by a stray command. In v0.1 it has no command of its own either — the
ways to set it are the on-device overlay, or the config JSON:

```python
import json

scene = json.loads(conn.config.retrieve())
scene["scene"]["photodiode"]["enabled"] = True
scene["scene"]["photodiode"]["flicker"] = True
conn.config.upload("my_photodiode_flicker", json.dumps(scene),
                   overwrite=True, apply_now=True)
```

`retrieve` hands you the current scene as JSON — the exact format of a
`.config.json` file — and `upload` writes it back under a name, with
`apply_now=True` also applying it immediately. Anything you can express in a
config file, you can reach this way, which is the escape hatch for settings the
command API has not grown a verb for yet.

`enabled` draws the patch; `flicker` is what makes it invert every frame.
Without `flicker` it is a static square, useful as a fixed luminance reference.

!!! tip "The patch is why a photodiode is worth the trouble"
    A pulse on a VTL line tells you when vstimd *committed* a frame. The
    photodiode tells you when photons actually changed. On a well-behaved rig
    those agree to within a frame; the gap between them is the number you
    quote in a methods section. See
    [Frame timing](../developer/frame-timing.md).

## 4. Save

The script saves first with `conn.config.save(...)` and then re-uploads the
patched JSON under the same name, so the file on the device has the photodiode
settings in it and a later `load` restores them.

## Run it

```console
$ cd client/python
$ uv run examples/demos/photodiode_flicker.py
Connecting to tcp://localhost:5555 …
Saved as 'my_photodiode_flicker'.
Photodiode patch on and inverting every frame.
```

## What to look for

- **Per-frame inversion, square trace.** The photodiode signal should be a clean
  square wave at half the refresh rate (each full cycle is two frames). Missing
  or doubled cycles are dropped or repeated frames.
- **The 5 Hz patch counts out.** Ten transitions per second, by eye.
- **Compare against `frame_rate`.** `conn.system.query_server_info().frame_rate_hz`
  is what vstimd measured. If the photodiode disagrees, believe the photodiode.

## Try changing it

- Set `on_frames=1, off_frames=1` on the big patch too, and see whether the
  display can really resolve a one-frame change at that size.
- Set `total_frames=600` and watch the flicker stop after 10 s at 60 Hz.
- Turn `flicker` off but leave `enabled` on, and use the patch as a static
  luminance reference for a photometer calibration.

## The complete script

??? example "`client/python/examples/demos/photodiode_flicker.py`"

    ```python
    --8<-- "client/python/examples/demos/photodiode_flicker.py"
    ```

## Next

- **[Frame timing](../developer/frame-timing.md)** — what the numbers you just measured mean.
- **[Trigger gate](trigger-gate.md)** — the input-side equivalent of this check.
