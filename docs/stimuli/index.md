# Stimuli

What vstimd can put on screen, and what every parameter of it means. These
pages are the reference to reach for once you know *which* stimulus you want;
[The command API](../concepts/command-api.md) is the walkthrough that gets you
here, and [Build the demos yourself](../tutorials/index.md) shows them used
together in runnable scenes.

| Page | Types |
|---|---|
| [Shapes](shapes.md) | `Rect`, `Circle`, `Ellipse` — flat coloured geometry, fill and outline (plus `Polygon`, not yet implemented) |
| [Gratings](gratings.md) | `Grating` — a masked, drifting sinusoidal (or square, saw, triangle) carrier |
| [Text](text.md) | `Text` — laid-out glyphs, with an optional box and border |
| [Random dot kinematograms](random-dots.md) | `Dots` — moving dot fields, coherence, and motion-defined figures |

## What every stimulus has

These are not part of any `*Params`. They are set at create time or through
`conn.stimuli` afterwards, and they mean the same thing for every type:

| Property | Set with | Meaning |
|---|---|---|
| `pos_px` | `create_*(position_px=...)`, `set_position` | Centre of the stimulus, in pixels from the screen centre, Y up — see [Coordinate system](../concepts/coordinate-system.md). |
| `rotation_deg` | `create_*(rotation_deg=...)` where offered, `set_rotation` | CCW, 0° = unrotated. `Dots` has none: a dot field has no orientation of its own, only a `direction_deg` on its motion. |
| `opacity` | `set_alpha` | One multiplier in `[0, 1]` applied to *every* colour the stimulus carries, on top of that colour's own alpha. It never overwrites a colour, so the relationship between a translucent fill and an opaque outline survives a fade. |
| `enabled` | `set_enabled` | Visibility, independent of existence. A disabled stimulus stays in the scene, keeps its handle, and costs nothing to bring back. |
| `name` | `create_*(name=...)`, `set_name` | What the overlay, the web UI, `list_stimuli()` and a saved config call it. Unnamed stimuli are legal and unreadable six months later. |
| draw order | `bring_to_front`, `send_to_back`, `swap_draw_order` | Stimuli draw in scene order, later over earlier. This is what makes two overlapping `Dots` fields resolve predictably — see the [figure-ground tutorial](../tutorials/figure-ground-rdk.md). |
| condition membership | `conn.conditions.set_stimulus_conditions` | Which [conditions](../concepts/conditions.md) the stimulus is active in; empty means every condition. |

## Two conventions that hold everywhere

!!! note "A size is always a full extent"
    A circle takes a `diameter_px`, never a radius; a patch takes a full
    `width_px` × `height_px`, never a half-size. This holds in the proto, the
    scene, the config JSON, both clients and the overlay, so a config that says
    `45` means the same 45 wherever it is read. Porting from a package that
    specifies radii — Psychtoolbox, and most of the RDK literature — means
    doubling at the boundary; `vstimd.stimuli.diameter_from_radius` is there to
    make that visible rather than silent.

!!! note "0 means *default*, not zero"
    A numeric params field left at its Python default of `0.0` tells the server
    "use your own default", not "make this literally zero-sized". A stimulus
    created with no arguments at all is never invisible — it comes back at a
    sane, visible size. Each page below lists what the server actually fills
    in.

    The two fields where zero is a legitimate value — `DotsParams.speed_px_per_s`
    (a static field) and `DotsParams.coherence` (pure noise) — are typed
    `float | None` instead, precisely so they cannot be caught by this rule.
