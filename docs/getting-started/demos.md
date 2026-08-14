# Demo scenes

vstimd ships a handful of ready-made scenes so a fresh rig shows something in
ten seconds — before anyone writes a line of Python — and so there is a fixed
set of scenes to eyeball after a deploy or a renderer change.

They are **ordinary configs**. Every demo is a `vstimd_demo_*.config.json` file
in the server's config directory, in exactly the format
[`config save`](../concepts/saving-loading.md) writes. There is no demo command
and no demo code path: anything a demo does, you can do, and a demo you like is
a starting point you can edit and re-save under your own name.

```console
$ vstimd-client config list
demo_drifting_grating
demo_first_light
demo_gratings_triggered
demo_moving_target
demo_photodiode_flicker
demo_trigger_gate

$ vstimd-client config load demo_drifting_grating
```

The server installs the demos into its config directory
(`/var/lib/braemons/vstimd` on a packaged rig) at startup: missing ones are
written, and ones it installed earlier and you never touched are refreshed when
a new version ships a newer copy. **A demo you edited is never overwritten** —
it stops tracking the shipped version from that point on, so save it under your
own name if you want both. A demo you delete comes back on the next start
unless you save something else under that name.

To take a shipped update after editing a demo, delete your copy and restart.

Every demo puts an on-screen explanation of itself at the bottom of the frame,
including which pins drive it, so a rig with no client attached is still
self-describing.

## What each demo does

| Name | What it shows | Triggers |
|---|---|---|
| `demo_first_light` | Centre dot and four corner squares — the display is being driven, edge to edge | — |
| `demo_drifting_grating` | Full-field sinusoidal grating, 0.01 cyc/px, drifting at 4 cyc/s | — |
| `demo_gratings_triggered` | Two masked gratings (45°, 135°) at the centre, each flashed for 2 s by its own input pin | in 11, 12 → out 36/37, 38/40 |
| `demo_moving_target` | Target sweeping left→right at 600 px/s, looping forever | out 36 each sweep |
| `demo_photodiode_flicker` | Photodiode patch inverting every frame, plus a 5 Hz full-field flicker | — |
| `demo_trigger_gate` | Square-wave patch visible exactly while an input pin is HIGH | in 7 |

The three demos that need no trigger start running the moment they are loaded.

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
| `demo_gratings_triggered` | `in_pin11` | 11 | rising edge → show the 45° grating for 2 s |
| | `out_pin36` | 36 | pulses at 45° onset |
| | `out_pin37` | 37 | pulses when the 45° flash ends |
| | `out_pin35` | 35 | HIGH from the end of the 45° flash until the next one starts |
| | `in_pin12` | 12 | rising edge → show the 135° grating for 2 s |
| | `out_pin38` | 38 | pulses at 135° onset |
| | `out_pin40` | 40 | pulses when the 135° flash ends |
| | `out_pin32` | 32 | HIGH from the end of the 135° flash until the next one starts |
| `demo_moving_target` | `out_pin36` | 36 | pulses at the end of every sweep |
| `demo_trigger_gate` | `in_pin7` | 7 | HIGH → patch visible, LOW → hidden |

Output pulses are one frame wide and are committed right after the vblank the
stimulus becomes visible on, which is what makes them usable as event marks —
see [Frame timing](../concepts/frame-timing.md). The `out_pin35` / `out_pin32`
lines show the other mode: a level that holds the "finished" state until the
next presentation starts, for a client that polls rather than timestamps.

!!! note "Durations are counted in frames"

    `demo_gratings_triggered` uses `FlashForNFrames`, whose duration is counted
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
    conn.config.load("demo_gratings_triggered")
    conn.vtl.set_line(pin11, True)     # rising edge → 45° grating, 2 s
    conn.vtl.set_line(pin11, False)
```

## Making your own

Load the demo that is closest to what you want, change it — from the overlay,
the [web UI](../client/web.md), or a client — and save it under a name of your
own:

```console
$ vstimd-client config load demo_gratings_triggered
$ # …adjust orientations, sizes, durations…
$ vstimd-client config save my_experiment
```

Saving under a `demo_`-prefixed name works too, but the next server start will
not replace it, so prefer your own names for real experiments.
