# Coordinate System

All 2-D stimulus positions use a **pixel-space coordinate system**:

- **Origin** at the screen centre
- **X** increases to the right
- **Y** increases upward
- **Units** are pixels

```
                  +Y
                   │
                   │
    ───────────────┼───────────────  X
                   │
                   │
                  -Y
```

A rectangle at `(x=0, y=0)` is centred on screen. A rectangle at `(x=200, y=0)` is 200 pixels
to the right of centre.

## Examples

| Position | Meaning |
|---|---|
| `x=0, y=0` | Screen centre |
| `x=500, y=0` | 500 px right of centre |
| `x=0, y=-300` | 300 px below centre |
| `x=-100, y=200` | Upper-left quadrant |

## Screen size

Query the display dimensions at runtime:

=== "Python"

    ```python
    info = conn.system.query_server_info()
    # Top-right corner of the screen:
    x_max = info.width_px / 2
    y_max = info.height_px / 2
    ```

The rig's display size is a property of the rig, so query it rather than
hard-coding it: a script written against a 1920 × 1080 panel then still puts a
stimulus in the same *place* on a 2560 × 1440 one.

## Rotation

Stimulus rotation is `rotation_deg`, measured in degrees **counter-clockwise**
from the positive X axis. A rectangle created with `rotation_deg=45` is rotated
45° CCW; `conn.stimuli.set_rotation(handle, 45)` does the same to a live one.

It is spelled `rotation_deg` everywhere — in the config JSON, in both clients,
and in the overlay. There is no `orientation` and no `angle`. A grating's stripe
orientation is this same placement rotation rather than a parameter of its own;
see [Gratings](../stimuli/gratings.md#masks).

## Sizes

A size is always a **full extent**, never a half-extent: a circle takes a
`diameter_px`, a patch a full `width_px` × `height_px`. This is the other
convention that holds across every stimulus — see
[Stimuli](../stimuli/index.md#two-conventions-that-hold-everywhere).

## Notes

- The coordinate system matches PsychoPy's `units="pix"` mode, which is what
  lets the [PsychoPy layer](command-api.md#psychopy-compatibility) pass
  positions straight through.
- Stimuli placed in 3-D space use a separate world-space transform in
  centimetres (`position_cm`) rather than this pixel plane, and 2-D and 3-D
  coexist in one frame. No 3-D stimulus type exists yet, so every stimulus you
  can create today lives in the pixel space above.
