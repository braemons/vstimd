# vstimd Python client

Python client for the `vstimd` visual stimulus server. Talks to the server
over ZMQ using protobuf encoding.

## Install

```bash
pip install vstimd
```

Or with [uv](https://docs.astral.sh/uv/):

```bash
uv add vstimd
```

### Development install

```bash
cd client/python
uv sync
```

## Quick start

```python
from vstimd import Connection
from vstimd.stimuli import Vec2, Color

with Connection() as conn:
    h = conn.stimuli.shapes.create_rect(pos=Vec2(-200, 0), width=300, height=200,
                                        color=Color(1.0, 0.0, 0.0))
    conn.stimuli.set_enabled(h, False)
    conn.stimuli.delete(h)
    info = conn.system.query_server_info()
    print(info.version)
```

`Connection(address="tcp://localhost:5555")` — default address shown.

## `vstimd.psychopy` — PsychoPy-compatible layer

Drop-in replacement for `psychopy.visual`:

```python
# Before
from psychopy import visual

# After
from vstimd import psychopy as visual
```

The only required addition is `address=` on `Window`:

```python
win = visual.Window(address='tcp://192.168.1.10:5555')
circ = visual.Circle(win, radius=50, fillColor='red')
rect = visual.Rect(win, width=200, height=100, fillColor=(-1, 1, -1))
grat = visual.GratingStim(win, sf=0.05, mask='circle')
circ.draw()
win.flip()
```

### Implemented classes

| Class | Notes |
|---|---|
| `Window` | Owns the `Connection`; `flip()` flushes the command queue |
| `Rect` | `create_rect`, position, size, fill color, orientation, alpha |
| `Circle` | `create_circle`, position, radius, fill color, orientation, alpha |
| `GratingStim` | `create_grating`, all grating parameters; `mask` accepts `'circle'`, `'gauss'`, `'raisedCos'` |

All constructor arguments from `psychopy.visual` are accepted. Parameters that
have no server-side equivalent (`autoLog`, `depth`, `interpolate`, etc.) are
accepted and silently ignored for drop-in compatibility.

### Deferred (frame-buffer) mode

By default (`deferred=True`) property changes are sent to the server's deferred
queue immediately; `win.flip()` tells the server to apply the entire queue
atomically before the next vsync. Set `deferred=False` to apply each command
immediately as it arrives.

### Color formats accepted

Named strings (`'red'`), hex strings (`'#ff0000'`), PsychoPy `rgb` tuples
`(-1..1)`, plain `0..1` tuples, `rgb255` tuples, and scalar greyscale values.

## `vstimd-client` — command-line tool

Installing the package also installs a `vstimd-client` executable for the
system-level commands, plus mDNS discovery of servers on the local network:

```bash
pip install 'vstimd[discover]'   # [discover] adds the pure-Python mDNS backend
```

```console
$ vstimd-client discover
ID             HOSTNAME             ADDRESSES   ADDRESS
vstimd-a1b2c3  vstimd-a1b2c3.local  10.0.1.42   tcp://vstimd-a1b2c3.local:5555

$ vstimd-client --host vstimd-a1b2c3 info
version     0.4.1
resolution  1920x1080
frame rate  60.00 Hz
background  0.000 0.000 0.000 1.000
```

Discovery browses for `_vstimd._tcp` using the
[zeroconf](https://pypi.org/project/zeroconf/) package if it is installed, and
otherwise falls back to `avahi-browse`. The `ID` column is the server's
`id=` TXT record — the reliable identity, unlike the display name which Avahi
may suffix with `#2` on collision.

Other commands: `ls`, `background`, `delete-all`, `enable-all`/`disable-all`,
`wait-frames`, `wait-ready`, `shutdown`, and `config list|save|load|get|upload`.
The target server comes from `--address`, `--host`, `$VSTIMD_ADDRESS`, or
`tcp://localhost:5555`, in that order. `--json` makes every command emit
machine-readable output. See `vstimd-client --help` and the
[CLI docs](docs/cli.md) for details.

## Regenerating protobuf stubs

```bash
cd client/python
make proto   # requires grpcio-tools in the dev dependency group
```

## Tests

```bash
cd client/python

# Unit tests (no server required)
make test

# E2E against the null renderer (builds server binary automatically)
make test-e2e-null

# E2E against a real running server
VSTIM_SERVER_ADDR=tcp://192.168.1.10:5555 make test-e2e
```
