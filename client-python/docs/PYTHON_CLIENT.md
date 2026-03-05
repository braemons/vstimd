# Python Client (`wonderlamp_client`)

`wonderlamp_client` is a PsychoPy-compatible Python package that controls `wonderlamp_server`
over ZeroMQ.  Experiment scripts can swap:

```python
# Before
from psychopy import visual

# After
from wonderlamp_client import visual
```

and have their code mostly work without changes.

---

## Install

```bash
pip install -e wonderlamp_client/
# or, with test dependencies:
pip install -e "wonderlamp_client/[dev]"
```

Requires Python ≥ 3.10 and `pyzmq >= 25`.

---

## Connecting to the server

The connection address is set on the `Window` object via the `address` parameter.
This is the **only place** you specify the server's IP and port.

```python
# Same machine (default)
win = visual.Window(size=(1920, 1080))

# Remote machine — change host and/or port as needed
win = visual.Window(
    size=(1920, 1080),
    address='tcp://192.168.1.10:5555',
)

# Different port on localhost
win = visual.Window(address='tcp://localhost:9000')
```

The ZMQ endpoint format is `tcp://<host>:<port>`.

---

## Migration from psychopy

| psychopy | wonderlamp_client | Notes |
|---|---|---|
| `from psychopy import visual` | `from wonderlamp_client import visual` | direct swap |
| `Window(size=...)` | `Window(size=..., address='tcp://host:port')` | add `address` |
| `Circle(win, ...)` | identical | ✓ |
| `Rect(win, ...)` | identical | ✓ |
| `Polygon(win, ...)` | identical | ✓ |
| `Line(win, ...)` | identical | ✓ |
| `ShapeStim(win, ...)` | identical | ✓ |
| `GratingStim` | not in v1 | raises `AttributeError` |
| `TextStim` | not in v1 | raises `AttributeError` |
| `ImageStim` | not in v1 | raises `AttributeError` |
| `win.flip()` | identical | sends batch to server |
| `stim.draw()` | identical | one-shot per frame |
| `stim.autoDraw = True` | identical | always rendered |
| `contains()` / `overlaps()` | not in v1 | raises `NotImplementedError` |

### What is a no-op stub

- `monitor=` on Window is accepted for future deg/cm units but ignored if units are `pix`/`norm`/`height`
- `autoLog=` is accepted but logging is not wired up yet
- `contrast=` is accepted but not forwarded to the server yet

---

## Deferred vs immediate mode

### `deferred=True` (default — matches psychopy frame model)

Property setters stage commands locally.  Nothing is sent to the server until
`win.flip()` is called.  All staged commands are sent as a single ZMQ multipart
message — the server applies them atomically before the next render frame.

```python
win = visual.Window(deferred=True)   # default
circle = visual.Circle(win, radius=50)
circle.pos = (100, 0)   # staged, not sent yet
circle.opacity = 0.8    # staged
win.flip()              # ← sends everything here
```

### `deferred=False` (immediate)

Every property setter sends a ZMQ command immediately.  `win.flip()` is a no-op.
Use this for interactive / exploratory use, not for time-critical experiments.

```python
win = visual.Window(deferred=False)
circle.pos = (100, 0)   # sent immediately
```

---

## Unit system

All coordinates are converted to pixels before being sent to the server.
The server's origin is the window centre (matches psychopy default).

Supported units: `pix` (default), `norm`, `height`.

`deg` and `cm` require a psychopy `Monitor` object:

```python
from psychopy.monitors import Monitor
mon = Monitor('testMonitor', width=52.0, distance=57.0)
win = visual.Window(monitor=mon, units='deg')
circle = visual.Circle(win, radius=2.0, units='deg')
```

---

## Asset transfer (v1)

In v1, the only supported mechanism is **path reference**: pass an absolute
filesystem path as a string.  The server loads the asset from disk.

Inline binary (numpy arrays, PIL Images) and chunked upload for remote/large
assets are planned for v2.

---

## Running the compatibility check

```bash
# Print human-readable report
python wonderlamp_client/compat/check_compat.py

# Generate pytest fixture file and run tests
python wonderlamp_client/compat/check_compat.py --output-pytest-fixtures wonderlamp_client/tests/_compat_fixtures.py
pytest wonderlamp_client/tests/test_api_compat.py -v
```

Example output:

```
wonderlamp_client.visual  ←→  psychopy.visual  compatibility check
==============================================================
  Window             OK  (2 extensions: address, deferred)
  Circle             OK
  Rect               OK
  Polygon            OK
  Line               OK
  ShapeStim          OK
```

---

## Running unit tests

```bash
# Unit and API compat tests (no server required)
pytest wonderlamp_client/tests/ -v

# Integration tests (requires live server)
VSTIM_SERVER_ADDR=tcp://192.168.1.10:5555 pytest wonderlamp_client/tests/test_integration.py --run-integration -v
```
