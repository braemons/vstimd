# Python client

The `vstimd` Python package is the reference client. It opens a ZMQ connection to a
server and exposes typed command namespaces, so you never touch protobuf or sockets
directly. For step-by-step walkthroughs see the [tutorials](../tutorials/index.md);
this page is the reference for the client's shape.

## Install

Requires Python ≥ 3.12. The distribution on PyPI is `vstimd-client`; the import
package is `vstimd`.

```sh
pip install vstimd-client
```

Until `0.1.0` is released the only versions on PyPI are release candidates, and
the command above installs the newest of them: pip and uv fall back to
pre-releases when nothing stable satisfies the request. Once a final release
exists, that same command gives you the stable version, and opting back into
candidates needs `pip install --pre vstimd-client` (or `uv add --prerelease
allow vstimd-client`).

To work from a checkout instead:

```sh
cd client/python
uv sync
make proto      # the protobuf stubs are generated, not committed
```

See [Installation](../getting-started/installation.md) for other options.

## Connect

A `Connection` opens a ZMQ REQ socket and exposes the command namespaces. Use it as a
context manager so the socket is always closed:

```python
from vstimd import Connection

with Connection() as conn:                 # default: tcp://localhost:5555
    info = conn.system.query_server_info()
    print(info.width_px, info.height_px, info.frame_rate_hz)
```

Pass an address for a remote device: `Connection("tcp://stimulus-pc:5555")`.

Every call is a **synchronous round-trip**: the client blocks until the server
acknowledges, and a server error is raised as a typed exception (see
[Errors](#errors)).

## Namespaces

| Namespace | What it does | Key methods |
|---|---|---|
| `conn.stimuli` | Generic per-stimulus commands | `set_position`, `set_rotation`, `set_fill_color`, `set_alpha`, `set_enabled`, `set_name`, `delete`, `query`, `bring_to_front`, `send_to_back`, `swap_draw_order` |
| `conn.stimuli.shapes` | Create/mutate shapes | `create_rect`, `create_circle`, `create_ellipse`, `set_rect_size`, `set_circle_diameter`, `set_ellipse_size`, `set_draw_mode`, `set_outline_color`, `set_outline_width` |
| `conn.stimuli.grating` | Create/mutate gratings | `create_grating`, `set_phase`, `set_sf`, `set_contrast`, `set_waveform`, `set_mask`, `set_drift_speed`, `set_drift_angle`, `set_fore_color`, … (opacity: use `conn.stimuli.set_alpha`) |
| `conn.stimuli.text` | Create/mutate text | `create_text`, `set_text`, `set_text_color` |
| `conn.system` | Scene-wide + queries | `set_background`, `set_all_enabled`, `clear_stimuli`, `clear_animations`, `clear_all`, `set_deferred_mode`, `list_stimuli`, `query_server_info`, `wait_for_frames`, `wait_until` |
| `conn.animations` | On-device animations | `create_flash`, `create_flicker`, `create_move_along_path_2d`, `create_couple_visibility_to_trigger_line`, `arm`, `disarm`, `cancel`, `query`, … |
| `conn.vtl` | Virtual Trigger Lines | `set_line_name`, `set_line`, `toggle_line`, `list_lines` |
| `conn.scene_config` | Save/load scenes | `save`, `load`, `list_scene_configs`, `retrieve`, `upload` |

The method list above is a map, not an exhaustive signature reference — the
authoritative signatures and docstrings live in the source under
[`client/python/vstimd/`](https://github.com/braemons/vstimd/tree/main/client/python/vstimd).

## Stimulus types

Creating a stimulus returns a **handle** you pass to later commands. Positions are in
**pixels from the screen centre, Y up** (see
[Coordinate system](../concepts/coordinate-system.md)); colours are RGBA in 0–1.

| Type | Create with |
|---|---|
| Rectangle | `conn.stimuli.shapes.create_rect(...)` |
| Circle | `conn.stimuli.shapes.create_circle(...)` |
| Ellipse | `conn.stimuli.shapes.create_ellipse(...)` |
| Grating | `conn.stimuli.grating.create_grating(...)` |
| Text | `conn.stimuli.text.create_text(...)` |

```python
from vstimd import Connection
from vstimd.stimuli import Color, RectParams, ShapeAppearance, Vec2

with Connection() as conn:
    rect = conn.stimuli.shapes.create_rect(
        position_px=Vec2(0, 0),
        params=RectParams(
            width_px=300,
            height_px=150,
            appearance=ShapeAppearance(fill_color=Color(1.0, 0.0, 0.0)),
        ),
    )
    conn.stimuli.set_position(rect, Vec2(-200, 100))
    conn.stimuli.set_enabled(rect, True)
    print(conn.stimuli.query(rect))       # full current state
```

## Errors

No method returns an error code. Every non-OK server response is raised as a
subclass of `VstimdError`, so a script that catches nothing still cannot carry
on past a command the server refused.

| Exception | Raised when |
|---|---|
| `HandleNotFoundError` | The stimulus handle does not exist |
| `WrongStimulusTypeError` | A mutation does not apply to this stimulus type |
| `WrongTargetError` | A system command was sent to a stimulus handle (or vice versa) |
| `CreationFailedError` | The server could not create the stimulus |
| `InvalidArgumentError` | A field value is out of range or invalid |
| `NotSupportedError` | The command is not supported in this build/config |
| `NotReadyError` | Server still initialising — retry after the first frame |
| `ConfigNotFoundError`, `ConfigIoError`, `ConfigFormatError`, `ConfigVersionError`, `ConfigAlreadyExistsError` | Config save/load failures |
| `UnknownServerError` | Unexpected server-side error, or a code this client predates |
| `ProtocolError` | The reply could not be decoded, or carried no result code |

Three of these group under `StimulusError` (`HandleNotFoundError`,
`WrongStimulusTypeError`, `WrongTargetError`) and the five config failures
under `ConfigError`, so you can catch a family rather than listing members:

```python
from vstimd.exceptions import ConfigError, ConfigNotFoundError

try:
    conn.scene_config.load("gratings")
except ConfigNotFoundError:
    conn.scene_config.save("gratings")          # first run on this rig
except ConfigError as exc:
    log.error("config unusable: %s", exc.detail)
```

Each exception carries what the server said and what it was answering:

```python
try:
    conn.stimuli.set_enabled(bad_handle, True)
except HandleNotFoundError as exc:
    exc.code       # ErrorCode.HANDLE_NOT_FOUND
    exc.detail     # the server's own message
    exc.command    # 'set_enabled' — which request failed
    exc.handle     # 7, or None for a scene-wide command
```

`str(exc)` folds that context in, so an uncaught one reads
`no such stimulus (set_enabled, handle 7)` rather than leaving you to work out
which of twenty mutations it came from.

## PsychoPy compatibility

The `vstimd.psychopy` layer mirrors `psychopy.visual` on top of the command API —
often a one-line import swap:

```python
# from psychopy import visual
from vstimd.psychopy import visual

win  = visual.Window(address="tcp://stimulus-pc:5555")
rect = visual.Rect(win, width=0.5, height=0.25, fillColor="red")
rect.draw()
win.flip()
```

!!! warning "Timing note"
    The PsychoPy-compatible layer drives updates through Python round-trips and does
    **not** expose the animation or VTL systems. For timing-critical reactions, use
    the command API directly and move them into
    [animations](../concepts/vtl-and-animations.md).

## Examples

Runnable scripts live in
[`client/python/examples/`](https://github.com/braemons/vstimd/tree/main/client/python/examples):

```sh
cd client/python
uv run examples/flash_rects.py          # flashing rectangles
uv run examples/interactive_grating.py  # live grating controls
uv run examples/text_demo.py            # text rendering
```

## Other clients

- **C# / Bonsai** — supported over the same wire protocol.
- **MATLAB** — *planned; not yet available* (see
  [`client/matlab/`](https://github.com/braemons/vstimd/tree/main/client/matlab)).
- **Roll your own** — any language that can speak ZMQ + protobuf works; see the
  [wire protocol](../developer/protocol.md).
