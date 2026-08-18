# Tutorial: Moving target

**Rebuilds:** `demo_moving_target` · **Script:** `client/python/examples/demos/moving_target.py`

A target sweeps across the screen at a constant speed, restarts, and pulses an
output line at the end of every sweep — forever, with nothing driving it. Two
ideas here: motion the *server* owns, and an animation that both repeats itself
and reports each repeat on a pin.

The second one is what makes this the scene to load when you want to measure
end-to-end output latency: put a photodiode where the sweep ends, scope it
against pin 36, and the gap between them is what your rig actually costs you.

## 1. The output line

```python
from vstimd import Connection, FinalAction, VtlHandle, VtlKind

conn.vtl.set_line_name(0, 36, VtlKind.OUTPUT, name="out_pin36")
sweep_done = VtlHandle.output(0, 36)
```

One line, named so the overlay and the saved config both know what it is for.

## 2. The target

```python
target = conn.stimuli.shapes.create_circle(
    position_px=Vec2(-800, 0),
    name="target",
    params=CircleParams(
        diameter_px=60,
        appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
    ),
)
```

Created at the *start* of the sweep. The animation will overwrite the position
on every frame anyway, but a scene that looks right before anything runs is a
scene you can debug.

## 3. The sweep

```python
sweep = conn.animations.create_move_along_segments_2d(
    target,
    x=[-800.0, 800.0],
    y=[0.0, 0.0],
    speed_px_per_sec=600.0,
    name="sweep_left_to_right",
    final_action_mask=FinalAction.RESTART | FinalAction.FINAL_ACTION_TRIGGER_LINE,
    final_action_trigger_line=sweep_done,
)
conn.animations.arm(sweep)
```

`move_along_segments_2d` takes piecewise-linear waypoints and a **speed**, and
the server converts that into a per-frame step using its measured frame rate. So
600 px/s is 600 px/s on a 60 Hz display and on a 144 Hz display — the number of
frames the sweep takes differs, the motion does not.

Its sibling `create_move_along_path_2d` takes one position **per frame**
instead: full control, at the cost of having to know the frame rate yourself.
Use segments when you mean a speed, path when you mean an exact trajectory.

Then the two final actions:

- **`RESTART`** — on completion, begin again. This is what makes the sweep loop.
  Note it is not the same as `REARM`: `REARM` returns to `Armed` and waits for
  the next trigger, `RESTART` goes straight back to `Running`. With no
  `start_trigger` there is nothing to wait for, so `RESTART` is the one you
  want.
- **`FINAL_ACTION_TRIGGER_LINE`** — pulse `out_pin36` for one frame at the end
  of each sweep, committed right after the vblank, which is what makes it a
  usable mark. See [Frame timing](../developer/frame-timing.md).

!!! note "No `start_trigger`, so arming starts it"
    An animation waits in `Armed` only if it has something to wait for. This one
    does not, so `arm()` runs it immediately — and the saved config comes back
    running, too.

## 4. Save

```python
add_explanation(conn, EXPLANATION)
conn.config.save("my_moving_target")
```

## Run it

```console
$ cd client/python
$ uv run examples/demos/moving_target.py
Connecting to tcp://localhost:5555 …
Saved as 'my_moving_target' — sweeping now, and again on every load.
```

## Try changing it

- Add waypoints: `x=[-800, 0, 800, 0]`, `y=[0, 400, 0, -400]` gives a diamond at
  the same speed.
- Add `StartAction.START_ACTION_TRIGGER_LINE` on a second line to mark the
  *start* of each sweep as well, and measure the sweep duration on the scope.
- Give it a `start_trigger` and swap `RESTART` for `REARM`: now it is one sweep
  per external edge, which is a perfectly good moving-stimulus trial.
- Slow it to 60 px/s and check with a photodiode that the target really moves
  one pixel per frame, rather than two pixels every second frame.

## The complete script

??? example "`client/python/examples/demos/moving_target.py`"

    ```python
    --8<-- "client/python/examples/demos/moving_target.py"
    ```

## Next

- **[Photodiode & flicker](photodiode-flicker.md)** — measuring what the display is really doing.
- **[Integrating recording systems](../concepts/recording-integration.md)** — turning those pulses into events.
