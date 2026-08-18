# Tutorial: First light

**Rebuilds:** `demo_first_light` · **Script:** `client/python/examples/demos/first_light.py`

The smallest complete scene there is: a title, a dot in the middle, and a square
in each corner. No animations, no triggers, nothing that can be mistimed. Load
it on a rig you have just wired up and one glance answers the only question that
matters at that point — is the display being driven, edge to edge?

This page builds it from an empty scene.

!!! info "Prerequisites"
    A running server (`cargo run --release`, or `--null` for a headless check)
    and the Python client (`cd client/python && uv sync`). If you have not met
    handles and `create_*` yet, read [the command API](../concepts/command-api.md)
    first.

## 1. Clear the scene and set the background

```python
from vstimd import Connection
from vstimd.stimuli.stimuli_models import Color, Vec2

with Connection("tcp://localhost:5555") as conn:
    clean_slate(conn)                          # see the overview page
    conn.system.set_background(0.05, 0.05, 0.05)
```

Every snippet from here on runs inside that `with` block; they are shown
unindented so each one stands on its own.

Near-black rather than black: a true 0.0 background hides the difference between
"the display is showing a black frame" and "the display is off", which is
precisely the distinction this scene exists to make. Note that
`set_background` takes loose floats, not a `Color`.

## 2. A title

```python
conn.stimuli.text.create_text(
    position_px=Vec2(0, 220),
    name="title",
    params=TextParams(
        text="vstimd — first light",
        letter_height_px=80,
        text_color=Color(1.0, 1.0, 1.0),
        box_size_px=Vec2(1600, 120),
    ),
)
```

Text is laid out inside a box: `box_size` sets the box, `letter_height` sets the
glyph size in pixels, and `position` places the box centre.
Positions are pixels from the screen centre with **Y up** — see
[Coordinate system](../concepts/coordinate-system.md) — so `y=220` is above
the middle.

## 3. A centre dot

```python
conn.stimuli.shapes.create_circle(
    position_px=Vec2(0, 60),
    name="fixation_dot",
    params=CircleParams(
        diameter_px=16,
        appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
    ),
)
```

Circles take a diameter, the way every other shape takes its full size, so this
one is 16 px across. It sits slightly above centre to stay clear of the title's
descenders.

## 4. Four corner squares

```python
CORNERS = [
    ("corner_tl", -900.0,  480.0),
    ("corner_tr",  900.0,  480.0),
    ("corner_bl", -900.0, -480.0),
    ("corner_br",  900.0, -480.0),
]

for name, x, y in CORNERS:
    conn.stimuli.shapes.create_rect(
        position_px=Vec2(x, y),
        name=name,
        params=RectParams(
            width_px=80,
            height_px=80,
            appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
        ),
    )
```

Their centres are 1800 × 960 px apart, which leaves a margin inside a
1920 × 1080 frame. That margin is the point: if a square is clipped or missing,
the display is not showing you the whole frame — overscan, a wrong mode, or a
compositor in the way.

!!! note "The saved config uses the same numbers as the command"
    `width_px=80, height_px=80` is recorded as `"size_px": [80.0, 80.0]` in the config
    JSON — full width and height, exactly what you passed.

## 5. The caption, and save

```python
add_explanation(conn, EXPLANATION)         # see the overview page
conn.config.save("my_first_light")
```

`save` writes `my_first_light.config.json` into the server's config directory
and raises `ConfigAlreadyExistsError` if that name is taken — pass
`overwrite=True` to replace it. From then on:

```python
conn.config.load("my_first_light")
```

restores the whole scene, on this server or any other one you upload the file
to.

## Run it

```console
$ cd client/python
$ uv run examples/demos/first_light.py
Connecting to tcp://localhost:5555 …
Saved as 'my_first_light' — load it again with conn.config.load('my_first_light')
```

## Try changing it

- Give the corner squares distinct colours, so you can tell in a photo which
  corner is which.
- Add a fifth square at the exact centre of each edge to check for
  non-uniform scaling.
- Re-save under a second name and use `conn.config.list_configs()` to see both.

## The complete script

??? example "`client/python/examples/demos/first_light.py`"

    ```python
    --8<-- "client/python/examples/demos/first_light.py"
    ```

## Next

- **[Drifting grating](drifting-grating.md)** — the same static-scene skills, plus motion the server owns.
