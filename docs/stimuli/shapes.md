# Shapes

Four flat, coloured geometries — rectangle, circle, ellipse, polygon — that
share one appearance model and differ only in how they are sized. They are the
fixation dots, the response targets, the photodiode patches and the frames of
most experiments.

```python
from vstimd import Connection
from vstimd.stimuli import CircleParams, Color, ShapeAppearance, Vec2

with Connection() as conn:
    fix = conn.stimuli.shapes.create_circle(
        position_px=Vec2(0, 0),
        name="fixation_dot",
        params=CircleParams(
            diameter_px=20,
            appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
        ),
    )
```

## Appearance

Every shape carries one `appearance: ShapeAppearance`. It is the only thing
they have in common besides [what every stimulus has](index.md#what-every-stimulus-has),
and the only params difference between the four is their geometry.

| Field | Default | Meaning |
|---|---|---|
| `fill_color` | `None` → inherits the scene's `default_fill` | RGBA in 0–1. `None` is absent on the wire and means *inherit*, so a create that does not mention colour does not silently override the scene defaults. A query always reports a concrete colour, because the server has always resolved one by then. |
| `outline_color` | `None` → inherits the scene's `default_outline` | Same inheritance as `fill_color`. |
| `outline_width_px` | `0.0` → the scene default (2 px) | A stroke width, not a switch: a 0-width outline draws nothing whatever `draw_mode` says, which is why 0 reads as "inherit" here like every other size. |
| `draw_mode` | `ShapeDrawMode.FILLED` | `FILLED`, `OUTLINED`, or `FILLED_AND_OUTLINED`. **This** is how an outline is turned on and off — not the width. |

## Rect

```python
conn.stimuli.shapes.create_rect(position_px=Vec2(0, 0), params=RectParams(...))
```

| Field | Default | Meaning |
|---|---|---|
| `width_px` | `0.0` → 100 px | Full width, not a half-width. |
| `height_px` | `0.0` → 100 px | Full height. |
| `appearance` | see above | Fill and outline. |

Rotate it with `rotation_deg` at create time or `conn.stimuli.set_rotation`
afterwards — the rectangle turns about its own centre, which is `pos_px`.

## Circle

```python
conn.stimuli.shapes.create_circle(position_px=Vec2(0, 0), params=CircleParams(...))
```

| Field | Default | Meaning |
|---|---|---|
| `diameter_px` | `0.0` → 100 px | **Diameter**, never a radius. |
| `appearance` | see above | Fill and outline. |

A circle is the one shape with a single size number, and it is the one most
often mis-ported: Psychtoolbox and most of the RDK literature specify radii.
Double at the boundary — see the note in [Stimuli](index.md#two-conventions-that-hold-everywhere).

## Ellipse

```python
conn.stimuli.shapes.create_ellipse(position_px=Vec2(0, 0), params=EllipseParams(...))
```

| Field | Default | Meaning |
|---|---|---|
| `width_px` | `0.0` → 100 px | Full width of the bounding box — the major or minor axis in full, not a semi-axis. |
| `height_px` | `0.0` → 100 px | Full height. |
| `appearance` | see above | Fill and outline. |

An ellipse with `width_px == height_px` is a circle; prefer `create_circle`
when it is conceptually one, so the saved config says what you meant.

## Polygon

```python
conn.stimuli.shapes.create_polygon(position_px=Vec2(0, 0), params=PolygonParams(...))
```

| Field | Default | Meaning |
|---|---|---|
| `vertices_px` | `[]` | Vertex positions **relative to the stimulus's own centre**, not absolute screen coordinates — so moving the polygon with `set_position` moves the whole outline, and the vertex list stays valid. |
| `close_shape` | `True` | Draw the edge from the last vertex back to the first. `False` leaves it open — a polyline rather than a polygon, which with `OUTLINED` is how you draw an arbitrary path. |
| `appearance` | see above | Fill and outline. A `FILLED` polygon is tessellated, so a self-intersecting vertex list fills by the tessellator's rule rather than by whatever you had in mind — check it on screen. |

## Mutating a live shape

| Setter | Changes |
|---|---|
| `conn.stimuli.shapes.set_rect_size(handle, width_px, height_px)` | Rect geometry. |
| `conn.stimuli.shapes.set_circle_diameter(handle, diameter_px)` | Circle geometry. |
| `conn.stimuli.shapes.set_ellipse_size(handle, width_px, height_px)` | Ellipse geometry. |
| `conn.stimuli.shapes.set_polygon_vertices(handle, vertices_px)` | The whole vertex list, replaced in one call. `close_shape` is fixed at create time. |
| `conn.stimuli.shapes.set_draw_mode(handle, draw_mode)` | Fill / outline / both. |
| `conn.stimuli.shapes.set_outline_color(handle, color)` | Outline colour. |
| `conn.stimuli.shapes.set_outline_width(handle, width_px)` | Outline stroke width. |
| `conn.stimuli.set_fill_color(handle, color)` | Fill colour — on the generic namespace, since a grating and text have a colour too. |

A size setter is type-checked against the stimulus it addresses: calling
`set_circle_diameter` on a rect raises rather than silently reinterpreting the
number.

## Next

- **[First light](../tutorials/first-light.md)** — shapes and text built into the
  smallest complete scene.
- **[Gratings](gratings.md)** — the next stimulus up, and the first one that
  moves on its own.
