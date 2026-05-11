# Python Client (`vstimd_client`)

> **Status:** Low-level `Connection` class implemented (protobuf/ZMQ, 3 commands). PsychoPy-compatible `visual` layer is planned — not yet started.
> **Location:** `client/python/` directory
> **See also:** `PLAN.md` §9 (Migration Path), `STIMULUS_DATA_MODEL.md`

---

## Table of Contents

1. [Goal](#1-goal)
2. [Design Decisions](#2-design-decisions)
3. [Package Structure](#3-package-structure)
4. [Wire Protocol](#4-wire-protocol)
5. [Class Design](#5-class-design)
6. [Unit System](#6-unit-system)
7. [Color Normalisation](#7-color-normalisation)
8. [flip() Semantics and Deferred Mode](#8-flip-semantics-and-deferred-mode)
9. [Asset Transfer](#9-asset-transfer)
10. [Testing Strategy](#10-testing-strategy)
11. [Compatibility Check](#11-compatibility-check)
12. [Migration Guide](#12-migration-guide)
13. [What Is Not in v1](#13-what-is-not-in-v1)
14. [Roadmap](#14-roadmap)

---

## 1. Goal

Existing neuroscience experiments are written against `psychopy.visual`. The goal of
`vstimd_client` is to let experimenters swap one import line:

```python
# Before
from psychopy import visual

# After
from vstimd_client import visual
```

and have their scripts work without changes, while the rendering is now handled by
`vstimd` (GPU-accelerated, sub-millisecond latency, Linux-capable) instead of
PsychoPy's local renderer.

---

## 2. Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Class hierarchy | Flat — no mixins, no shared base class behavior | PsychoPy has a deep inheritance hierarchy (MRO surprises, hidden state); vstimd_client is deliberately flat — each class is self-contained |
| Repeated fields | Duplicated explicitly in every class | Intentional — each class is self-contained; matches the plan in `STIMULUS_DATA_MODEL.md` |
| Wire format | **Protobuf over ZMQ REQ/REP** (implemented) | Compact, schema-versioned, multi-language; same `.proto` file shared by server and all clients |
| Position input | ZMQ commands only | High-rate gaze/joystick position stays in shared memory (see `INPUT_LATENCY.md`) |
| flip() model | Configurable: `deferred=True` (default) or `deferred=False` | `deferred=True` gives the experimenter explicit control over *when* changes take effect: all property changes since the last `flip()` are held back and applied simultaneously on the next `flip()` call, which is typically synchronised to a vsync |
| Coordinate origin | Window centre, Y-up, pixels | Matches PsychoPy default and the server's 2-D coordinate system |
| Scope v1 (current) | `Connection` class, 3 commands (`create_rect`, `set_enabled`, `delete`) | Low-level foundation; enough to drive `flash_rects.py` example |
| Scope v2 (planned) | `visual` module: `Window` + `Rect`, `Circle`, `Polygon`, `Line`, `ShapeStim` | PsychoPy-compatible API layer on top of `Connection`; sufficient to port most 2-D experiments |

---

## 3. Package Structure

### Current (implemented)

```
client/python/                       ← installable Python package
├── pyproject.toml                   ← build + dependencies (pyzmq, protobuf)
├── examples/
│   └── flash_rects.py               ← create two rects, flash them, delete them
└── vstimd_client/
    ├── __init__.py                  ← exports Connection
    ├── _connection.py               ← ZMQ REQ socket + protobuf send/recv; 3 commands
    └── _proto/
        ├── __init__.py
        └── vstimd_pb2.py        ← generated from server/proto/vstimd.proto
```

### Planned (visual layer)

```
vstimd_client/
    ├── __init__.py                  ← re-exports Connection + visual module
    ├── _connection.py               ← extended: batch send, deferred mode, reconnect
    ├── _proto/
    │   └── vstimd_pb2.py        ← regenerated as server schema expands
    ├── visual.py                    ← Window + all stimulus classes (PsychoPy-compatible)
    ├── _commands.py                 ← command dataclasses → protobuf serialisation
    ├── _units.py                    ← unit system conversion (pix/norm/height/deg/cm)
    └── _colors.py                   ← color space normalisation to RGBA float
tests/
    ├── conftest.py                  ← MockSocket, MockConnection, mock_win fixtures
    ├── test_api_compat.py           ← parametrized signature comparison vs psychopy
    ├── test_commands.py             ← unit tests: verify correct protobuf per operation
    └── test_integration.py         ← live-server tests (skipped by default)
compat/
    └── check_compat.py              ← standalone human-readable report + fixture gen
```

Install:

```bash
pip install -e client/python/
# or with uv:
uv pip install -e client/python/
```

---

## 4. Wire Protocol

### Transport

ZeroMQ REQ/REP socket pair. The client sends one **protobuf-encoded** frame and waits
for one reply. The schema is defined in `server/proto/vstimd.proto` and shared
between the server (Rust/prost) and the Python client (`vstimd_pb2.py`).

```python
# Low-level usage (current API)
from vstimd_client import Connection

with Connection("tcp://localhost:5555") as conn:
    handle = conn.create_rect(x=0, y=0, width=200, height=100, r=1.0, g=0.0, b=0.0)
    conn.set_enabled(handle, False)
    conn.delete(handle)
```

Internally `_connection.py` builds a `Request` protobuf message and calls
`SerializeToString()` before sending; responses are decoded with `ParseFromString()`.

### Handles

Server-allocated: `create_*` commands return a `handle` integer in the `Response`.
The client stores this handle and passes it to subsequent `set_enabled` / `delete` /
`move_to` etc. calls. Handle 0 is reserved for system commands.

### Batching (deferred mode) — planned for visual layer

In deferred mode, all commands staged since the last `flip()` are sent as a single ZMQ
**multipart message** (one serialized protobuf frame per command), followed by a
`CmdDeferredMode { start: false }` sentinel. The server holds staged changes until it
receives the sentinel, then applies them all atomically on the next rendered frame.

```
Client                              Server
  |                                   |
  |-- [req1][req2]...[reqN]---------> |  (multipart — one recv call)
  |   [DeferredMode{start:false}]     |  ← server flips all changes, renders frame
  |<-- Response{handle: -1} ---------|
```

### Protobuf stub regeneration

`vstimd_pb2.py` is generated from `server/proto/vstimd.proto`. Regenerate
after schema changes:

```bash
cd server
protoc --python_out=../client/python/vstimd_client/_proto proto/vstimd.proto
```

The stub must be kept in sync with the server binary. A mismatch causes silent decode
errors (unknown fields are silently ignored by protobuf; missing required fields raise
`DecodeError`).

---

## 5. Class Design

> **Note:** §5–§12 describe the planned `visual` layer (v2). None of this is implemented yet.

### Marker base

```python
class _vstimdBase:
    """Marker base. No methods or fields — only used for isinstance() checks."""
```

No behaviour is inherited. All fields and methods are repeated in full in each class.
This is intentional (see §2).

### Window

Owns the ZMQ connection. All stimulus objects hold a reference to their `Window` and
call `win._dispatch()` when properties change.

Key parameters:

| Parameter | Default | Notes |
|---|---|---|
| `size` | `(800, 600)` | pixels |
| `color` | `(0, 0, 0)` | background clear color |
| `units` | `'pix'` | default unit system for all stimuli |
| `monitor` | `None` | psychopy Monitor object; required for `deg`/`cm` units |
| `deferred` | `True` | frame-buffer mode (see §8) |
| `address` | `'tcp://localhost:5555'` | **server IP and port go here** |

### Shape classes

`Circle`, `Rect`, `Polygon`, `Line`, `ShapeStim` all expose the same interface:

**Constructor parameters (shared):** `win`, `units`, `pos`, `lineWidth`, `lineColor`,
`fillColor`, `colorSpace`, `ori`, `opacity`, `contrast`, `name`, `autoDraw`, `autoLog`

**Shape-specific parameters:**
- `Circle`: `radius`, `edges`
- `Rect`: `width`, `height`
- `Polygon`: `edges`, `radius`
- `Line`: `start`, `end`
- `ShapeStim`: `vertices`, `closeShape`

**Properties (getter + setter):** `pos`, `ori`, `opacity`, `fillColor`, `lineColor`,
`lineWidth`, `autoDraw`

**Methods:** `draw()`, `setPos()`, `setOri()`, `setSize()`, `setColor()`,
`setFillColor()`, `setLineColor()`, `setOpacity()`, `setAutoDraw()`

Each setter either enqueues (deferred) or immediately sends a ZMQ command.

---

## 6. Unit System

All coordinates are converted to **pixels** before being sent to the server.
Conversion is handled by `_units.py:to_pixels()`.

| Unit | Conversion |
|---|---|
| `pix` | identity |
| `norm` | `x_px = x_norm × width/2`, `y_px = y_norm × height/2` |
| `height` | `x_px = x_h × height`, `y_px = y_h × height` |
| `deg` | requires `monitor.deg2pix()` |
| `cm` | requires `monitor.cm2pix()` |

The unit is resolved per-stimulus: the stimulus's own `units` parameter takes priority;
if empty, the Window's `units` is used.

Arithmetic operations on `setPos` / `setOri` / `setSize` (`+`, `-`, `*`, `/`) are
applied **after** unit conversion, in pixel space.

---

## 7. Color Normalisation

Handled by `_colors.py:normalize_color()`. All colors are normalised to
`[r, g, b, a]` float `0.0–1.0` before being sent.

Accepted input formats:
- Named string: `'red'`, `'white'`, `'gray'`, ...
- Hex string: `'#ff0000'`, `'#f00'`
- Float tuple, psychopy rgb convention (-1..1): `(1.0, -1.0, 0.0)`
- Float tuple, plain 0..1: `(1.0, 0.0, 0.0)`
- Int tuple rgb255: `(255, 0, 128)` — detected by `colorSpace='rgb255'`
- Single float (greyscale, -1..1): `0.5`
- `None` → sent as `null` (no color / transparent)

---

## 8. flip() Semantics and Deferred Mode

The central purpose of deferred mode is **temporal control**: the experimenter decides
exactly when a set of stimulus changes becomes visible on screen. Property assignments
during a loop iteration are held back on the client, then all applied simultaneously
when `flip()` is called — typically aligned to a vsync by the server. This is the same
contract that PsychoPy provides with its local frame buffer.

### deferred=True (default)

1. All property setters call `win._enqueue()` — the JSON command is added to a local list.
   Nothing is sent to the server yet.
2. `flip()` assembles the batch:
   - staged property commands
   - `set_enabled=True` for any handles in `win._to_draw_once` (from `.draw()` calls)
   - `deferred_flip` sentinel
3. The batch is sent as one ZMQ multipart message.
4. The server receives the batch, applies every change, then renders the frame.
   All changes from this flip become visible at the same time.
5. After the reply, `set_enabled=False` is sent for all one-shot handles, clearing them.

### deferred=False (immediate)

Each property setter sends a ZMQ command immediately as it is called. The server applies
each change as it arrives — changes may therefore become visible at different times
relative to the render loop. `flip()` is a no-op. Suitable for setup, debugging, or
interactive use where exact timing is not required.

### draw() and autoDraw

| Mechanism | Behaviour |
|---|---|
| `autoDraw=True` | `set_enabled(True)` sent once; stimulus always rendered until disabled |
| `autoDraw=False` | `set_enabled(False)`; stimulus not rendered |
| `.draw()` | Adds handle to `win._to_draw_once`; enabled for one flip, then disabled |

This exactly replicates PsychoPy's "draw-once-per-loop-iteration" pattern.

---

## 9. Asset Transfer

Three mechanisms, not all in v1:

### A. Path reference (v1, default)

Send the absolute filesystem path. Server loads asset from disk. Used when `image`
is a string path (same host assumed).

```json
{"cmd": "create_image_stim", "handle": 5, "image_path": "/data/face.png"}
```

### B. Inline binary (v1, small assets)

Asset bytes are base64-encoded and embedded in the JSON. Used automatically when
`image` is a `numpy.ndarray`, `PIL.Image`, or `bytes` object.
Size limit: 1 MB (configurable via `Window(inline_limit_kb=1024)`). Larger raises an error.

```json
{"cmd": "create_image_stim", "handle": 5,
 "image_data": "iVBORw0KGg...", "image_format": "png"}
```

### C. Chunked upload (v2, large / remote assets)

`upload_begin` → N × `upload_chunk` → `upload_end`. Returns a server-side asset ID
that can be used in subsequent create commands. Raises `NotImplementedError` in v1.

### Custom shaders

WGSL source is always sent as a plain string in the JSON command — no upload needed.

---

## 10. Testing Strategy

Tests are in `client/python/tests/`. They are structured in three layers:

### Layer 1 — Unit tests (`test_commands.py`) — no server required

Uses `MockConnection` / `MockSocket` (defined in `conftest.py`) to capture all outgoing
ZMQ frames without any network activity.

**What is tested:**
- Correct `"cmd"` field for each stimulus type
- Correct field values (handle, radius, pos, colors, etc.)
- Color normalisation (named, hex, float, none)
- Property setters produce correct `set_*` commands
- `setPos` with arithmetic operations (`+`, `-`, `*`, `/`) updates `pos` correctly
  and sends the result
- `autoDraw=True` sends `set_enabled=True`
- `draw()` one-shot: `set_enabled=True` before flip, `set_enabled=False` after
- `deferred_flip` is included at the end of every deferred `flip()` batch
- `deferred=False` mode sends commands immediately and omits `deferred_flip`

**Run:**
```bash
cd client/python
uv run pytest tests/test_commands.py -v
```

### Layer 2 — API compatibility tests (`test_api_compat.py`) — no server required

Parametrized tests that compare `vstimd_client.visual` signatures against
`psychopy.visual` using `inspect.signature`. These tests require:
1. `psychopy` installed (optional dev dependency)
2. A generated fixture file `tests/_compat_fixtures.py` (see §11)

**What is tested:**
- Every class in `CHECKED_CLASSES` exists in `vstimd_client.visual`
- Every public method from `psychopy.visual.<Class>` exists in the vstimd counterpart
- Every `__init__` parameter from psychopy is present in the vstimd counterpart

**Run:**
```bash
# First generate fixtures (requires psychopy installed):
cd client/python
uv run python compat/check_compat.py --output-pytest-fixtures tests/_compat_fixtures.py
uv run pytest tests/test_api_compat.py -v
```

### Layer 3 — Integration tests (`test_integration.py`) — requires live server

Skipped by default. Uses a real `Connection` and a real `vstimd` process.
Server address is set via `VSTIM_SERVER_ADDR` env var.

**What is tested:**
- `Window` open/close round-trip
- `Circle` appears on screen with correct properties (queried via `query_stimulus`)
- `Rect` position updates are applied
- Deferred batch is atomic: two `setPos` calls in one frame → final position wins

**Run:**
```bash
cd client/python
VSTIM_SERVER_ADDR=tcp://192.168.1.10:5555 \
  uv run pytest tests/test_integration.py --run-integration -v
```

### MockSocket / MockConnection design

`MockSocket` captures every outgoing JSON frame into a list. Helper methods:
- `sent_commands()` — full list of all captured command dicts
- `commands_by(cmd)` — filter by `"cmd"` field
- `last_cmd(cmd)` — most recent command of a given type
- `clear()` — reset between assertions

`MockConnection` subclasses `Connection` and replaces the real ZMQ socket with a
`MockSocket`. `conftest.py` provides:
- `mock_win` fixture — a `Window` with `MockConnection` injected (bypasses ZMQ connect)
- `mock_socket` fixture — direct access to the underlying `MockSocket`

The `mock_win` is constructed by setting fields directly on the object
(`object.__new__` + manual assignment) to avoid triggering the real `Window.__init__`
and its `open_window` ZMQ call.

---

## 11. Compatibility Check

### Standalone report

```bash
cd client/python
uv run python compat/check_compat.py
```

Output example:

```
vstimd_client.visual  ←→  psychopy.visual  compatibility check
==============================================================
  Window             OK  (2 extensions: address, deferred)
  Circle             OK
  Rect               OK
  Polygon            OK
  Line               OK
  ShapeStim          OK
```

Exit code 0 = fully compatible; 1 = at least one missing parameter or method.

### Generating pytest fixtures

```bash
cd client/python
uv run python compat/check_compat.py --output-pytest-fixtures tests/_compat_fixtures.py
```

Writes `tests/_compat_fixtures.py` with:
- `CHECKED_CLASSES` — list of class names to check
- `REQUIRED_PARAMS` — `[(class_name, param_name), ...]` for all `__init__` params
- `ALL_METHODS` — `[(class_name, method_name), ...]` for all public methods

Commit this file so `test_api_compat.py` can run in CI without `psychopy` installed.

---

## 12. Migration Guide

### Minimal change

```python
# Old
from psychopy import visual
win = visual.Window(size=(1920, 1080), color=(-1, -1, -1), units='pix')

# New — add address, everything else unchanged
from vstimd_client import visual
win = visual.Window(size=(1920, 1080), color=(-1, -1, -1), units='pix',
                    address='tcp://192.168.1.10:5555')
```

### What works unchanged

- All shape constructor calls and keyword arguments
- Property setters: `stim.pos = ...`, `stim.ori = ...`, `stim.opacity = ...`, etc.
- `setPos(delta, '+')` and other arithmetic operations
- `autoDraw` flag
- `draw()` one-shot pattern
- `win.flip()`
- `win.close()`
- Named colors, hex colors, float tuples, int rgb255 tuples

### What is a no-op stub (accepted but not forwarded)

- `autoLog=` on Window and stimuli
- `contrast=` on stimuli (stored but not sent to server yet)
- `monitor=` when units are `pix`/`norm`/`height` (only needed for `deg`/`cm`)

### What raises NotImplementedError in v1

- `stim.contains()`
- `stim.overlaps()`
- Chunked asset upload for assets > 1 MB on a remote host

### What raises AttributeError (not in v1)

- `visual.GratingStim`
- `visual.ImageStim`
- `visual.TextStim`
- `visual.DotStim`
- `visual.MovieStim`

---

## 13. What Is Not Yet Implemented

### Implemented now
- `Connection` class with `create_rect`, `set_enabled`, `delete`
- Protobuf wire format (always was — JSON was never used)
- `examples/flash_rects.py`

### Planned (visual layer — not started)

| Feature | Status |
|---|---|
| `visual` module (`Window`, `Rect`, `Circle`, `Polygon`, `Line`, `ShapeStim`) | Planned v2 |
| Deferred mode / `flip()` batching | Planned v2 — requires multipart ZMQ send |
| Unit system (`pix`/`norm`/`height`/`deg`/`cm`) | Planned v2 |
| Color normalisation (named, hex, float, rgb255) | Planned v2 |
| `autoDraw` / `draw()` one-shot pattern | Planned v2 |
| `_commands.py` dataclasses | Planned v2 |
| Tests (`test_commands.py`, `test_api_compat.py`, `test_integration.py`) | Planned v2 |
| PsychoPy compatibility check (`compat/check_compat.py`) | Planned v2 |
| ImageStim, TextStim, GratingStim, DotStim | Planned v3+ (requires server-side support) |
| MovieStim | Planned v4 (requires server Phase 12) |
| Chunked asset upload | Planned v3 |
| `contains()` / `overlaps()` | Planned v3 |
| `query_stimulus` / scene inspection | Planned v2 (requires server Phase 17/18) |
| MATLAB client | Long-term |

---

## 14. Roadmap

| Version | Additions |
|---|---|
| **v1** (now) | `Connection` class; protobuf/ZMQ; `create_rect`, `set_enabled`, `delete`; `flash_rects.py` example |
| **v2** | `visual` module: `Window` + `Rect`, `Circle`, `Polygon`, `Line`, `ShapeStim`; deferred `flip()`; unit system; color normalisation; full test suite; PsychoPy compat check |
| **v3** | `ImageStim` (path + inline binary); `TextStim`; `query_stimulus`; `contains()`/`overlaps()`; chunked upload; `GratingStim` |
| **v4** | `MovieStim`; MATLAB client parity |
