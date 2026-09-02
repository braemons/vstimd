# Build the demos yourself

The [demo scenes](../getting-started/demos.md) are ordinary configs, so every
one of them is something you could have built from a client. These six pages do
exactly that: each takes one shipped demo apart and rebuilds it from an empty
scene with the Python command API, then saves the result under a name of your
own.

That makes them the practical follow-on to the two API concepts pages. Where
[the command API](../concepts/command-api.md) teaches the calls and
[triggers & animations](../concepts/vtl-and-animations.md) teaches the on-device
execution model, these pages show both being used together to produce a scene
you can actually run an experiment with — and then persist, so the rig can boot
into it with no client attached.

| Tutorial | Rebuilds | Teaches |
|---|---|---|
| [First light](first-light.md) | `demos/first_light` | shapes, text, the scene as a unit |
| [Drifting grating](drifting-grating.md) | `demos/drifting_grating` | gratings, and motion the server owns |
| [Gratings, triggers & a saved config](gratings-triggers-config.md) | `demos/gratings_triggered` | arming stimuli against input lines, marking onsets on output lines, saving and booting into the result |
| [Moving target](moving-target.md) | `demos/moving_target` | path animations, looping, one pulse per repeat |
| [Photodiode & flicker](photodiode-flicker.md) | `demos/photodiode_flicker` | frame-counted timing, the photodiode patch |
| [Trigger gate](trigger-gate.md) | `demos/trigger_gate` | level-coupled visibility, driving a line from software |

Each page has a companion script in `client/python/examples/demos/`, runnable
as-is, and ends with that script in full under **The complete script** — the
real file, included at build time, so it cannot drift from what the page walks
through:

```console
$ cd client/python
$ uv run examples/demos/first_light.py                       # tcp://localhost:5555
$ uv run examples/demos/first_light.py tcp://vstimd-ab12.local:5555
$ uv run examples/demos/first_light.py --save-as my_scene -f
```

Every script takes the server address as an optional first argument, `--save-as`
for the config name to write, and `-f` to overwrite an existing one. Nothing in
them needs a display: run the server with `--null` and the scripts work exactly
the same, which is how they are tested.

!!! info "These pages are tested against the demos they describe"
    `tests/e2e/cases/test_demo_examples.py` runs each script for real and
    compares the resulting scene with the shipped demo config, so a tutorial
    that drifts away from the demo — or from the API — fails CI rather than
    quietly going stale.

    ```console
    $ cd client/python && make test-e2e-null
    ```

!!! note "Reading the snippets"
    Each page shows the calls one section at a time, unindented. In the script
    itself they all sit inside a single connection:

    ```python
    with Connection("tcp://localhost:5555") as conn:
        ...
    ```

## What every script does first

Three things repeat in all six, so they live in
`examples/demos/_common.py` rather than in each script.

**Start from nothing.** `clear_all` takes the animations and the stimuli
together — an animation outlives the stimuli it drives, so clearing one without
the other leaves half a scene. The VTL name map is I/O config rather than scene
content and survives, so the names go separately:

```python


def clean_slate(conn):
    conn.system.clear_all()
    for line in conn.vtl.list_lines():
        conn.vtl.set_line_name(line.bank, line.bit, line.kind, name="")
```

!!! tip "Three clear commands"
    `clear_stimuli()` and `clear_animations()` do one each; `clear_all()` does
    both, animations first. None of them touch the background, the default
    colours, the photodiode patch or the VTL names.

**Explain yourself on screen.** Every demo carries a caption across the bottom
of the frame, so a rig with no client attached still says what it is doing. It
is an ordinary text stimulus:

```python


def add_explanation(conn, text):
    return conn.stimuli.text.create_text(
        position_px=Vec2(0, -340),
        name="explanation",
        params=TextParams(
            text=text,
            letter_height_px=24,
            text_color=Color(0.9, 0.9, 0.9),
            fill_color=Color(0.0, 0.0, 0.0, 0.65),
            box_size_px=Vec2(1500, 320),
        ),
    )
```

**Name things.** Every stimulus and animation gets a `name=`. Names are what
the overlay, the web UI, and `list_stimuli()` show you, and they are saved with
the config — a scene of five unnamed handles is a scene nobody can edit six
months later.

## Two things worth knowing before you start

!!! note "Sizes are full sizes everywhere"
    `RectParams(width_px=80, height_px=80)` makes an 80 × 80 px square, and the
    saved JSON records `"size_px": [80.0, 80.0]`. Gratings and ellipses store
    `size` the same way, so a demo config can be read straight off as the
    arguments to pass. Circles are no exception: they take a `diameter`, not a
    radius, so every stimulus is sized the same way.

!!! note "The photodiode patch has no command yet"
    `demos/photodiode_flicker` and `demos/gratings_triggered` switch on the
    corner photodiode patch, which is a scene setting rather than a stimulus
    and has no command of its own in v0.1. The scripts set it by editing the
    retrieved config JSON and uploading it back — see
    [Photodiode & flicker](photodiode-flicker.md). From the on-device overlay
    it is a keypress.

## Next

- **[First light](first-light.md)** — start here; it is the smallest complete scene.
- **[Demo scenes](../getting-started/demos.md)** — what the shipped demos do, and which pins they use.
- **[Saving & loading](../concepts/saving-loading.md)** — the persistence model these scripts end on.
