# Tutorial: Gratings, triggers & a saved config

**Rebuilds:** `demos/gratings_triggered` · **Script:** `client/python/examples/demos/gratings_triggered.py`

This is the one that ties everything together. In a single script you will build
two gratings, arm each of them against its own hardware input line, mark every
presentation on output lines a recording system can timestamp, save the whole
setup as a named config, and point the rig at that config so it boots into it.

At the end there is no client in the loop. The device runs trials on its own,
which is the workflow a rig operator actually wants, and the reason the command
API and the animation system exist side by side.

!!! info "Prerequisites"
    [The command API](../concepts/command-api.md) for handles and `create_*`, and
    [triggers & animations](../concepts/vtl-and-animations.md) for VTL lines, arming, and
    start/final actions. A running server; `--null` is fine, and no wiring is
    needed — [step 6](#6-fire-the-triggers-without-any-hardware) drives the
    inputs from software.

## 1. Name the lines you are going to use

A VTL line is addressed by bank and bit. Names are optional, but they are shown
by the overlay and the web UI instead of bare bit numbers, and — this is the
part that matters here — they are **saved with the config**, so the I/O map
travels with the scene.

```python
from vstimd import Connection, VtlHandle, VtlKind

LINES = [
    ("in_pin11",  0, 11, VtlKind.INPUT),
    ("in_pin12",  0, 12, VtlKind.INPUT),
    ("out_pin36", 0, 36, VtlKind.OUTPUT),
    ("out_pin37", 0, 37, VtlKind.OUTPUT),
    ("out_pin38", 0, 38, VtlKind.OUTPUT),
    ("out_pin40", 0, 40, VtlKind.OUTPUT),
    ("out_pin35", 0, 35, VtlKind.OUTPUT),
    ("out_pin32", 0, 32, VtlKind.OUTPUT),
]

for name, bank, bit, kind in LINES:
    conn.vtl.set_line_name(bank, bit, kind, name=name)

in_45,   in_135   = VtlHandle.input(0, 11),  VtlHandle.input(0, 12)
on_45,   on_135   = VtlHandle.output(0, 36), VtlHandle.output(0, 38)
end_45,  end_135  = VtlHandle.output(0, 37), VtlHandle.output(0, 40)
done_45, done_135 = VtlHandle.output(0, 35), VtlHandle.output(0, 32)
```

The names here read like header pins because the shipped `gpiochip-daqd`
example for the Raspberry Pi 5 maps bit *n* to header pin *n*. That is a
property of the DAQ mapping, not of VTL: a line number is a line number until
something outside vstimd binds it to copper.

Input and output are **separate banks**, so bit 36 as an input and bit 36 as an
output are different lines. That is why `VtlHandle.input` and
`VtlHandle.output` exist rather than one constructor.

## 2. Two gratings, hidden

```python
from vstimd.stimuli.grating_models import GratingMask, GratingTexture

gratings = {}
for label, angle in (("45deg", 45.0), ("135deg", 135.0)):
    handle = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        rotation_deg=angle,
        name=f"grating_{label}",
        params=GratingParams(
            width_px=600,
            height_px=600,
            sf_cycles_per_px=0.02,
            contrast=1.0,
            waveform=GratingTexture.SIN,
            mask=GratingMask.RAISED_COS,
            mask_param=0.2,
        ),
    )
    conn.stimuli.set_enabled(handle, False)
    gratings[label] = handle
```

Two things to notice.

**They differ in exactly one parameter.** Same size, same spatial frequency,
same contrast, same mask — only the orientation changes, so a difference in the
response is a difference in orientation tuning and not in anything else.

**They start hidden.** From the next step on, the animation owns their
visibility. Creating a stimulus enabled and then disabling it, rather than
building it while something else is on screen, keeps the moment of onset under
the animation's control alone.

`GratingMask.RAISED_COS` with `mask_param=0.2` gives a circular aperture whose
outer 20 % is a raised-cosine fringe — no hard edge, so the patch has no
orientation-carrying border of its own.

## 3. A fixation dot that never moves

```python
from vstimd.stimuli.shapes_models import ShapeDrawMode

dot = conn.stimuli.shapes.create_circle(
    position_px=Vec2(0, 0),
    name="fixation_dot",
    params=CircleParams(
        diameter_px=12,
        appearance=ShapeAppearance(fill_color=Color(0.0, 0.0, 0.0)),
    ),
)
conn.stimuli.shapes.set_draw_mode(dot, ShapeDrawMode.FILLED_AND_OUTLINED)
conn.stimuli.shapes.set_outline_color(dot, Color(1.0, 1.0, 1.0))
```

Black core, white ring: it has to stay visible against the grey background
*and* against whichever grating appears behind it.

## 4. Arm each grating against its input line

This is the step the whole tutorial is for.

```python
from vstimd import FinalAction, StartAction, VtlEdge

for label, anim_name, trigger, onset, end, done in (
    ("45deg",  "flash_45deg_on_pin11",  in_45,  on_45,  end_45,  done_45),
    ("135deg", "flash_135deg_on_pin12", in_135, on_135, end_135, done_135),
):
    anim = conn.animations.create_flash(
        gratings[label],
        duration_frames=120,                  # 2 s at 60 Hz
        name=anim_name,
        start_trigger=trigger,
        start_edge=VtlEdge.RISING,
        start_action_mask=StartAction.ENABLE | StartAction.START_ACTION_TRIGGER_LINE,
        start_action_trigger_line=onset,
        final_action_mask=(
            FinalAction.DISABLE
            | FinalAction.REARM
            | FinalAction.FINAL_ACTION_TRIGGER_LINE
            | FinalAction.DONE_LEVEL
        ),
        final_action_trigger_line=end,
        final_action_level_line=done,
    )
    conn.animations.arm(anim)
```

Read it as three groups.

**When to start.** `start_trigger` plus `start_edge` says: sit in `Armed` until
a rising edge appears on this input line. Nothing else starts the flash.

**What to do at the start.** `StartAction.ENABLE` shows the stimuli;
`START_ACTION_TRIGGER_LINE` pulses `start_action_trigger_line` for one frame.
That pulse is committed right after the vblank the grating first appears on —
which is what makes it usable as an event mark rather than an approximation.
See [Frame timing](../developer/frame-timing.md).

**What to do at the end.** `DISABLE` hides the gratings again;
`FINAL_ACTION_TRIGGER_LINE` pulses the *end* line for one frame;
`DONE_LEVEL` drives a third line HIGH and leaves it there until this animation
next starts. The pulse answers *when* it finished, for a system that
timestamps; the level answers *whether* it has finished, for a system that
polls. They are separate lines so you can have both.

`REARM` is what makes this a protocol rather than a one-shot. Without it a
completed animation is `DONE` and ignores every further edge; with it, the
animation returns to `Armed` the moment it finishes, and the next edge fires
another presentation. Trial after trial, indefinitely.

!!! warning "Durations are frames, not seconds"
    `duration_frames=120` is 2 s on a 60 Hz display and 0.83 s on a 144 Hz one.
    That is deliberate — a stimulus duration that is not a whole number of
    frames is a lie. If you want to think in milliseconds, pass `duration_ms=`
    instead and the client converts using the server's measured frame rate.

!!! note "Creating is not arming"
    `create_*` leaves an animation `Idle`. `conn.animations.arm(handle)` is what
    makes it listen. An animation with no `start_trigger` starts running the
    moment it is armed — see [Moving target](moving-target.md).

## 5. Save it, and check the round trip

```python
conn.scene_config.save("my_gratings_triggered")
print(conn.scene_config.list_scene_configs())
```

`save` retrieves the current scene and writes it to the server's config
directory as `my_gratings_triggered.config.json`. It raises
`ConfigAlreadyExistsError` if the name is taken; pass `overwrite=True` to
replace.

What went into that file is the point: the two gratings, the fixation dot, the
caption, **both animations with their trigger wiring and action masks**, the
background, and the eight VTL names from step 1. Loading it into a fresh session
gives you back a rig that is armed and ready:

```python
with Connection("tcp://vstimd-ab12.local:5555") as conn:
    print(conn.scene_config.list_scene_configs())
    # ['demos/first_light', …, 'my_gratings_triggered']

    conn.scene_config.load("my_gratings_triggered")     # clears, then loads

    print([e.name for e in conn.system.list_stimuli()])
    print([a.name for a in conn.animations.list_animations()])
    print([line.name for line in conn.vtl.list_lines()])
```

!!! note "One thing the shipped demo adds here"
    `demos/gratings_triggered` also switches on the corner photodiode patch, so a
    photodiode timestamps the same onsets the pulses mark. That is a scene
    setting with no command of its own in v0.1, so the script sets it by editing
    the retrieved config JSON and uploading it back — the four lines are shown
    in [Photodiode & flicker](photodiode-flicker.md).

The animations come back in the state they were saved in, so after the load the
device is already waiting for edges on `in_pin11` and `in_pin12`. See
[Saving & loading](../concepts/saving-loading.md) for `additive=True`,
`retrieve`/`upload`, and where the files live.

## 6. Fire the triggers without any hardware

`set_line` on an **input** handle writes the same bit a DAQ edge would, so you
can exercise the whole path on a laptop:

```python
import time

conn.vtl.set_line(in_45, True)      # rising edge → 45° grating appears
conn.vtl.set_line(in_45, False)     # level after the edge does not matter
time.sleep(2.5)                     # 2 s flash, then it re-arms

conn.vtl.set_line(in_135, True)     # and again on the other line
conn.vtl.set_line(in_135, False)
```

`conn.vtl.toggle_line(handle)` does the same in one call and returns the new
level.

This is a stand-in for the real thing, and it is worth being clear about what it
does and does not prove. It exercises the animation, the actions, and the output
pulses exactly as a hardware edge would. It does **not** exercise your wiring,
your DAQ mapping, or the latency between the physical edge and vstimd seeing it.
For that, `demos/trigger_gate` and a scope are the tools —
see [Trigger gate](trigger-gate.md).

## 7. The payoff: make it what the rig boots into

A saved config is a scene the device can restore with no client attached. Point
the rig config at it and the rig comes up armed after a power cycle:

```toml
# /etc/braemons/vstimd-rig-config.toml
[startup]
# Named config (in --storage-dir) to load at boot. The literal "last" loads the
# auto-saved last-session slot. Omit or "" for an empty scene.
load_config = "my_gratings_triggered"
```

Restart `vstimd.service` and the gratings, the animations, and the VTL names are
all back, waiting for the first edge. The experiment PC is now optional: it can
connect to change parameters between blocks, or not connect at all.

Notes on that file:

- The template lives at `server/config/default-rig-config.toml`, which
  documents every key with the defaults commented out.
- On a rig with the [Samba shares](../operations/appliance-setup.md#6-admin-access-ssh-optional-samba)
  installed, `/etc/braemons` is a network share, so this is a file you can edit
  from a lab Windows or macOS machine without SSHing in.
- `--scene-config <name>` (or `--scene-config-file <path>`) on the command line
  overrides `[startup] load_config`.
- `save_on_quit = true` plus `load_config = "last"` gives you the other
  behaviour: come back up in whatever state the last session ended in.

## Run it

```console
$ cd client/python
$ uv run examples/demos/gratings_triggered.py --fire
Connecting to tcp://localhost:5555 …
Saved as 'my_gratings_triggered'. Configs on the device: demos/first_light, …
Firing in_pin11 (45°) …
Firing in_pin12 (135°) …
Both fired. On a wired rig those edges come from the DAQ instead.
```

## Try changing it

- Add a third orientation on a third input line. The loop in step 4 already
  takes a table — extend it.
- Swap `FinalAction.REARM` out and watch the second edge do nothing: the
  animation is `DONE`. `conn.animations.arm(handle)` brings it back.
- Give the flash a `cancel_trigger` so an abort line ends a presentation early,
  and a `cancel_action_mask` that pulses a separate "aborted" line.
- Set `duration_ms=250` instead of `duration_frames=120` and query the
  animation to see what frame count the client picked for your display.

## The complete script

??? example "`client/python/examples/demos/gratings_triggered.py`"

    ```python
    --8<-- "client/python/examples/demos/gratings_triggered.py"
    ```

## Next

- **[Moving target](moving-target.md)** — an animation that repeats itself with no trigger at all.
- **[Integrating recording systems](../concepts/recording-integration.md)** — what to do with those onset and end pulses.
- **[Saving & loading](../concepts/saving-loading.md)** — the full persistence model.
