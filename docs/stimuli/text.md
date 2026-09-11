# Text

Laid-out glyphs, optionally inside a filled and bordered box. Used for
instructions, for on-screen state a rig can be read from across the room, and —
in every shipped demo — for a caption that makes a display self-describing with
no client attached.

```python
from vstimd import Connection
from vstimd.stimuli import Color, TextParams, Vec2

with Connection() as conn:
    conn.stimuli.text.create_text(
        position_px=Vec2(0, -340),
        name="explanation",
        params=TextParams(
            text="Trial 1 of 240 — press any key to begin",
            letter_height_px=24,
            text_color=Color(0.9, 0.9, 0.9),
            fill_color=Color(0.0, 0.0, 0.0, 0.65),   # translucent backing box
            box_size_px=Vec2(1500, 320),
        ),
    )
```

## Parameters

| Field | Default | Meaning |
|---|---|---|
| `text` | `""` | The string to render. Newlines are honoured; wrapping is governed by `box_size_px`. |
| `font` | `""` → the server's default face | Font family name, resolved on the server — which is the machine that has the fonts, not necessarily the one running your script. |
| `letter_height_px` | `0.0` → the server default | Glyph height in pixels. |
| `box_size_px` | `(0, 0)` | The layout box, as a full extent. Text wraps within it. `(0, 0)` means unbounded — no wrapping, and the box is whatever the text needs. |
| `anchor` | `"center"` | Which point of the box sits at `pos_px`: `"center"`, `"top-left"`, `"top-right"`, `"bottom-left"`, or `"bottom-right"`. Anything else is read as `"center"`. |
| `text_color` | white | Glyph colour, RGBA in 0–1. |
| `fill_color` | transparent | The box background. **`alpha == 0` means no fill is drawn at all**, not "draw it black" — which is how a caption gets a translucent backing plate (`Color(0, 0, 0, 0.65)`) or none. |
| `border_color` | transparent | The box border, drawn only when `alpha > 0`, by the same rule as `fill_color`. |
| `flip_horiz` | `False` | Mirror the rendered text left-right, for a display viewed through a mirror — a common rig geometry, and one where flipping the whole scene in the client is the wrong fix. |
| `language_style` | `LanguageStyle.LTR` | `LTR`, `RTL`, or `ARABIC` — the bidi and shaping mode used to lay glyphs out. `ARABIC` selects contextual joining, which `RTL` alone does not do. |

## The box and the anchor

`pos_px` positions the *box*, and `anchor` says which of its points lands
there. With the default `"center"`, a caption at `Vec2(0, -340)` is centred
horizontally and sits 340 px below the screen centre — the position every demo
uses. With `"top-left"`, the same `pos_px` puts the box's top-left corner
there instead, which is what you want when pinning status text to a screen
corner regardless of how long the string turns out to be.

`box_size_px` is a full extent like every other size in vstimd, so a
`Vec2(1500, 320)` box is 1500 px wide, not ±1500.

## Mutating live text

| Setter | Changes |
|---|---|
| `conn.stimuli.text.set_text(handle, text)` | The string — the one you call per trial. |
| `conn.stimuli.text.set_text_color(handle, color)` | Glyph colour. |
| `conn.stimuli.set_position(handle, pos_px)` | Where the box sits. |
| `conn.stimuli.set_alpha(handle, opacity)` | Fades glyphs, fill and border together, each keeping its own alpha. |

Font, size, anchor, box and language style are fixed at create time: they
change the layout, and re-laying out text is not something to do inside a
trial. Create a second text stimulus and toggle `enabled` instead.

## Next

- **[First light](../tutorials/first-light.md)** — text and shapes in the
  smallest complete scene.
- **[Shapes](shapes.md)** — the other static stimulus family.
