# Demo scenes

vstimd ships a handful of ready-made scenes so a fresh rig shows something in
ten seconds — before anyone writes a line of Python — and so there is a fixed
set of scenes to eyeball after a deploy or a renderer change.

They are **ordinary scene-configs**. Every demo is a `.config.json` file in the
server's `demos` project, in exactly the format
[`scene-config save`](../concepts/saving-loading.md) writes. There is no demo command
and no demo code path: anything a demo does, you can do, and a demo you like is
a starting point you can edit and re-save under your own name.

```console
$ vstimd-client scene-config list
demos/drifting_grating
demos/figure_ground_rdk
demos/first_light
demos/gratings_triggered
demos/moving_target
demos/photodiode_flicker
demos/trigger_gate

$ vstimd-client scene-config load demos/drifting_grating
```

The server installs the demos into its `demos` project
(`/var/lib/braemons/vstimd/projects/demos` on a packaged rig) at startup. What happens to a
file that is already there depends on whether you have touched it:

| Your copy | What the server does |
|---|---|
| not there | writes it |
| exactly as the server left it | replaces it when a new version ships a newer copy |
| edited by you | **leaves it alone, permanently** |

So a demo you edited is never overwritten — it stops tracking the shipped
version from that point on. Save it under your own name if you want to keep
both, and delete your copy (then restart) to take a shipped update after all. A
demo you delete comes back on the next start unless you save something else
under that name.

The startup log says which of the three happened: `installed demo configs`,
`updated demo configs to the shipped version`, or `kept local demo configs`.

Every demo puts an on-screen explanation of itself at the bottom of the frame,
including which pins drive it, so a rig with no client attached is still
self-describing.

## What each demo does

| Name | What it shows | Triggers |
|---|---|---|
| `demos/first_light` | Centre dot and four corner squares — the display is being driven, edge to edge | — |
| `demos/drifting_grating` | Full-field sinusoidal grating, 0.01 cyc/px, drifting at 4 cyc/s | — |
| `demos/figure_ground_rdk` | Two full-screen random-dot fields split by a circular aperture and its exact complement — a figure defined by direction alone, no luminance/density/texture cue | — |
| `demos/gratings_triggered` | Two masked gratings (45°, 135°) at the centre, each flashed for 2 s by its own input pin | in 11, 12 → out 36/37, 38/40 |
| `demos/moving_target` | Target sweeping left→right at 600 px/s, looping forever | out 36 each sweep |
| `demos/photodiode_flicker` | Photodiode patch inverting every frame, plus a 5 Hz full-field flicker | — |
| `demos/trigger_gate` | Square-wave patch visible exactly while an input pin is HIGH | in 7 |

The four demos that need no trigger start running the moment they are loaded.

## Triggers: what to wire up

The trigger demos use the VTL lines that the **Raspberry Pi 5** `gpiochip-daqd`
example maps to physical header pins, and that example is what the
[Pi 5 image](../operations/raspberry-pi-image.md) installs. On a rig flashed
from that image the demos therefore drive real pins with no extra
configuration; on any other board, copy the matching example over
`/etc/braemons/gpiochip-daqd-config.toml` (see
[Deployment](../operations/deployment.md)) or expect the demos to sit waiting
for a trigger that never arrives.

| Demo | VTL line | Header pin | Meaning |
|---|---|---|---|
| `demos/gratings_triggered` | `in_pin11` | 11 | rising edge → show the 45° grating for 2 s |
| | `out_pin36` | 36 | pulses at 45° onset |
| | `out_pin37` | 37 | pulses when the 45° flash ends |
| | `out_pin35` | 35 | HIGH from the end of the 45° flash until the next one starts |
| | `in_pin12` | 12 | rising edge → show the 135° grating for 2 s |
| | `out_pin38` | 38 | pulses at 135° onset |
| | `out_pin40` | 40 | pulses when the 135° flash ends |
| | `out_pin32` | 32 | HIGH from the end of the 135° flash until the next one starts |
| `demos/moving_target` | `out_pin36` | 36 | pulses at the end of every sweep |
| `demos/trigger_gate` | `in_pin7` | 7 | HIGH → patch visible, LOW → hidden |

Output pulses are one frame wide and are committed right after the vblank the
stimulus becomes visible on, which is what makes them usable as event marks —
see [Frame timing](../developer/frame-timing.md). The `out_pin35` / `out_pin32`
lines show the other mode: a level that holds the "finished" state until the
next presentation starts, for a client that polls rather than timestamps.

!!! note "Durations are counted in frames"

    `demos/gratings_triggered` uses `FlashForNFrames`, whose duration is counted
    in frames: 120 frames is 2 s at the 60 Hz the Pi 5 rig runs at, and
    something else on a display running at another rate.

    Both flashes carry `FinalAction.REARM`, so each one returns to `Armed` when
    it finishes and fires again on the next edge — trial after trial, with no
    client in the loop. Drop that bit if you want an animation that fires
    exactly once per arm.

## No trigger source yet?

You do not need any wiring to see the trigger demos work: a software trigger
sets the same VTL input bit a physical pin would.

```python
from vstimd import Connection
from vstimd.vtl import VtlHandle, VtlKind

pin11 = VtlHandle.named("in_pin11", VtlKind.INPUT)

with Connection() as conn:
    conn.scene_config.load("demos/gratings_triggered")
    conn.vtl.set_line(pin11, True)     # rising edge → 45° grating, 2 s
    conn.vtl.set_line(pin11, False)
```

## Building them yourself

Every demo has a tutorial that rebuilds it from an empty scene with the Python
command API, and a runnable script to go with it — see
[Build the demos yourself](../tutorials/index.md). Reading a demo backwards
into the calls that would produce it is the fastest way to learn the API, and
the scripts are a better starting point for your own scene than a JSON file is.

| Demo | Tutorial | Script |
|---|---|---|
| `demos/first_light` | [First light](../tutorials/first-light.md) | `examples/demos/first_light.py` |
| `demos/drifting_grating` | [Drifting grating](../tutorials/drifting-grating.md) | `examples/demos/drifting_grating.py` |
| `demos/figure_ground_rdk` | — | `examples/demos/figure_ground_rdk.py` |
| `demos/gratings_triggered` | [Gratings, triggers & a saved config](../tutorials/gratings-triggers-config.md) | `examples/demos/gratings_triggered.py` |
| `demos/moving_target` | [Moving target](../tutorials/moving-target.md) | `examples/demos/moving_target.py` |
| `demos/photodiode_flicker` | [Photodiode & flicker](../tutorials/photodiode-flicker.md) | `examples/demos/photodiode_flicker.py` |
| `demos/trigger_gate` | [Trigger gate](../tutorials/trigger-gate.md) | `examples/demos/trigger_gate.py` |

## Making your own

Load the demo that is closest to what you want, change it — from the overlay,
the [web UI](../client/web.md), or a client — and save it under a name of your
own:

```console
$ vstimd-client scene-config load demos/gratings_triggered
$ # …adjust orientations, sizes, durations…
$ vstimd-client scene-config save my_experiment
```

Saving into the `demos` project works too, but the server re-seeds that project
on every start, so keep real experiments in a project of your own.
