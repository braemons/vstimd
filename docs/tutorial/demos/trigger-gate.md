# Tutorial: Trigger gate

**Rebuilds:** `demo_trigger_gate` · **Script:** `client/python/examples/demos/trigger_gate.py`

A patch that is visible exactly while an input line is HIGH, and hidden while it
is LOW. No duration, no re-arming, no state that can get stuck — which is why
this is the scene to load when you are debugging input wiring rather than
running an experiment.

It is the counterpart to
[Gratings, triggers & a saved config](gratings-triggers-config.md): same input
lines, opposite model. There, an *edge* started a timed presentation; here, a
*level* is mirrored onto the screen, frame by frame.

## 1. The gate line

```python
from vstimd import Connection, VtlHandle, VtlKind

conn.vtl.set_line_name(0, 7, VtlKind.INPUT, name="in_pin7")
gate = VtlHandle.input(0, 7)
```

## 2. A patch you cannot miss

```python
from vstimd.stimuli.grating_models import GratingMask, GratingTexture

patch = conn.stimuli.grating.create_grating(
    pos=Vec2(0, 0),
    width=700, height=700,
    sf=0.015,
    angle=90.0,                     # horizontal stripes
    contrast=1.0,
    waveform=GratingTexture.SQR,
    mask=GratingMask.CIRCLE,
    name="gated_grating",
)
```

A square wave through a hard circular mask: maximum contrast at a sharp edge, so
a single frame of it is unmistakable both by eye and on a photodiode trace.
`GratingMask.CIRCLE` is a hard aperture — no fringe, unlike the raised-cosine
mask used for the triggered gratings, because here you want the edge.

## 3. Couple visibility to the level

```python
gated = conn.animations.create_couple_visibility_to_trigger_line(
    gate, patch,
    polarity=True,
    name="gate_on_pin7",
)
conn.animations.arm(gated)
```

This animation never completes. Every frame it copies the line's level onto the
stimulus's visibility, and it keeps doing that until you disarm or delete it.
There is no `duration`, no `FinalAction`, and nothing to re-arm.

`polarity=True` means HIGH shows the patch. Pass `False` for an active-low
input, which is what you usually have when a TTL line idles high.

!!! tip "Level or edge?"
    Couple a level when the *external* system owns the timing and you want the
    screen to follow it — gating, a hold signal, a hardware-defined trial
    window. Use an edge-triggered [flash](gratings-triggers-config.md) when
    vstimd owns the duration and must guarantee it in frames. Mixing them up is
    the most common cause of a stimulus that is one frame too long at one end.

## 4. Save, then prove the wiring

```python
conn.config.save("my_trigger_gate")
```

With the config saved, the useful part starts. Drive the line from software
first, to confirm the *scene* is right:

```python
import time

for _ in range(3):
    conn.vtl.set_line(gate, True)      # patch visible
    time.sleep(0.7)
    conn.vtl.set_line(gate, False)     # patch hidden
    time.sleep(0.7)
```

`set_line` on an INPUT handle writes the same bit a DAQ edge would, so this
works on a laptop with nothing attached. Then unplug the software and use the
real source: if the patch no longer follows, the problem is in the wiring, the
`gpiochip-daqd` mapping, or the polarity — and *not* in vstimd, which you have
just eliminated.

`conn.vtl.list_lines()` shows the current level of every named line, which is
the quickest way to see whether an edge is arriving at all:

```python
for line in conn.vtl.list_lines():
    print(line.name, line.kind, "HIGH" if line.high else "LOW")
```

## Run it

```console
$ cd client/python
$ uv run examples/demos/trigger_gate.py --toggle
Connecting to tcp://localhost:5555 …
Saved as 'my_trigger_gate'.
  gate HIGH — patch visible
  gate LOW  — patch hidden
  …
```

## Try changing it

- Flip `polarity` to `False` and confirm the patch inverts its behaviour — the
  fastest test of an active-low input.
- Couple *two* stimuli to the same line by passing a list: one gate, several
  things appearing together.
- Couple visibility to an **output** line instead, so a marker on screen follows
  a pulse vstimd itself is emitting — a debugging aid for output timing.
- Add a second, independent gate on another line and check the two do not
  interfere.

## The complete script

??? example "`client/python/examples/demos/trigger_gate.py`"

    ```python
    --8<-- "client/python/examples/demos/trigger_gate.py"
    ```

## Next

- **[Gratings, triggers & a saved config](gratings-triggers-config.md)** — the edge-triggered counterpart, and how to persist a rig-ready setup.
- **[Triggers & animations](../vtl-and-animations.md)** — the full VTL and animation reference.
