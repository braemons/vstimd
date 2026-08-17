# Visual Stimulation Daemon — vstimd

[![Build and Test](https://github.com/braemons/vstimd/actions/workflows/ci.yml/badge.svg)](https://github.com/braemons/vstimd/actions/workflows/ci.yml)
[![Docs](https://readthedocs.org/projects/vstimd/badge/?version=latest)](https://vstimd.readthedocs.io/)

> **Status:** early alpha — under active development, not validated for
> experiments or data collection.

**vstimd** is a visual stimulus server for neuroscience and psychophysics. It
runs on a dedicated Linux device, renders with Vulkan straight onto KMS/DRM (no
X11, no Wayland, no compositor), and takes commands over the network from any
language that speaks ZMQ + protobuf.

It is **trigger-driven**: you set up a scene ahead of time — stimuli plus small
on-device animations — and the device then produces frame-accurate stimulation
the moment a Virtual Trigger Line fires, fed by a hardware DAQ. The
timing-critical path never leaves the box.

vstimd combines ideas from Michael Stephan's
[StimServer](https://github.com/esi-neuroscience/StimServer) and Andreas
Kreiter's **VStim**.

## 📖 [Documentation → vstimd.readthedocs.io](https://vstimd.readthedocs.io/)

- [Why vstimd?](https://vstimd.readthedocs.io/en/latest/why-vstimd/) — what a dedicated timing device buys you
- [Installation](https://vstimd.readthedocs.io/en/latest/getting-started/installation/) — packages, apt archive, source
- [Raspberry Pi 5 image](https://vstimd.readthedocs.io/en/latest/operations/raspberry-pi-image/) — flash a card, get a rig
- [Tutorials](https://vstimd.readthedocs.io/en/latest/tutorial/) · [Python client](https://vstimd.readthedocs.io/en/latest/client/python/) · [Developer guide](https://vstimd.readthedocs.io/en/latest/developer/architecture/)

## Getting started

**Run a rig.** Every [release](https://github.com/braemons/vstimd/releases) ships
`.deb`s (amd64/arm64), `.rpm`s (x86_64/aarch64), and a ready-to-flash Raspberry Pi
5 image. Packages are also served from the
[braemons apt archive](https://github.com/braemons/packages), so rigs upgrade in
place.

**Build and run it locally.**

```sh
cargo run --release                     # fullscreen (auto-detects DRM or desktop)
cargo run --release -- --windowed 1280x720
cargo run --release -- --null           # ZMQ only, no display
```

Press <kbd>D</kbd> for demo stimuli, <kbd>F1</kbd>–<kbd>F7</kbd> for overlay
panels, <kbd>Esc</kbd> to exit. Build dependencies and the packaging targets are
in [Installation](https://vstimd.readthedocs.io/en/latest/getting-started/installation/)
and [Building & packaging](https://vstimd.readthedocs.io/en/latest/developer/building/).

**Drive it from Python.**

```sh
cd client/python && uv sync
uv run examples/flash_rects.py
```

```python
from vstimd import Connection
from vstimd.stimuli import Color, RectParams, ShapeAppearance, Vec2

with Connection("tcp://vstimd-a1b2c3.local:5555") as conn:
    h = conn.stimuli.shapes.create_rect(
        position=Vec2(0, 0),
        params=RectParams(width=300, height=200,
                          appearance=ShapeAppearance(fill_color=Color(1.0, 0.0, 0.0))),
    )
    conn.stimuli.set_enabled(h, True)
```

Or open `http://<rig>:8080` for the built-in web control UI, and use
`vstimd-client discover` to find rigs on the network.

## Contributing

Design notes and roadmaps live in [`dev/`](dev/) — start with
[`dev/PLAN.md`](dev/PLAN.md). Build, test and packaging workflows are in
[`CLAUDE.md`](CLAUDE.md) and the
[developer guide](https://vstimd.readthedocs.io/en/latest/developer/building/).

## License

GNU AGPLv3, except the Python client in [`client/python`](client/python), which
is GNU LGPLv3 so that importing it does not place your experiment's own code
under copyleft. Copyright © 2026 Joscha Schmiedt, University of Bremen.
