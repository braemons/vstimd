# Python Client

The `vstimd` Python package provides a high-level client for the vstimd server.

## Installation

```sh
pip install ./client/python
# or, for development:
cd client/python && uv sync
```

## Quick example

```python
from vstimd import Connection
from vstimd.stimuli import Vec2, Color

with Connection("tcp://localhost:5555") as conn:
    h = conn.stimuli.shapes.create_rect(pos=Vec2(0, 0), width=200, height=100,
                                        color=Color(1.0, 0.0, 0.0))

    conn.system.set_deferred_mode(active=True)
    conn.stimuli.set_enabled(h, True)
    conn.stimuli.set_position(h, pos=Vec2(100, 50))
    conn.system.set_deferred_mode(active=False)

    info = conn.system.query_server_info()
    print(f"{info.width}×{info.height} @ {info.frame_rate:.1f} Hz")

    conn.stimuli.delete(h)
```

## Package layout

| Module | Contents |
|---|---|
| `vstimd.Connection` | ZMQ socket + `.stimuli` + `.system` sub-clients |
| `vstimd.stimuli.StimuliClient` | Create, mutate, query individual stimuli |
| `vstimd.system.SystemClient` | Scene-wide commands and server queries |
| `vstimd.psychopy` | PsychoPy-compatible layer (`Window`, `Rect`, `Circle`, `GratingStim`) |
| `vstimd.exceptions` | Exception hierarchy for server error codes |

## Handles

Every `create_*` call returns an integer **handle** — the server's identifier for that
stimulus. Pass it to subsequent mutation and delete calls. Handles are unique per server
session and are not reused after deletion.

```python
h = conn.stimuli.create_rect(...)   # h is an int, e.g. 1
conn.stimuli.set_enabled(h, False)
conn.stimuli.delete(h)
# h is now invalid — do not use it again
```

## Error handling

Server errors raise exceptions from `vstimd.exceptions`:

```python
from vstimd.exceptions import HandleNotFoundError

try:
    conn.stimuli.set_enabled(999, True)
except HandleNotFoundError:
    print("no stimulus with that handle")
```

| Exception | Server error code |
|---|---|
| `HandleNotFoundError` | `ERROR_CODE_HANDLE_NOT_FOUND` |
| `WrongStimulusTypeError` | `ERROR_CODE_WRONG_STIMULUS_TYPE` |
| `WrongTargetError` | `ERROR_CODE_WRONG_TARGET` |
| `CreationFailedError` | `ERROR_CODE_CREATION_FAILED` |
| `InvalidArgumentError` | `ERROR_CODE_INVALID_ARGUMENT` |
| `NotSupportedError` | `ERROR_CODE_NOT_SUPPORTED` |
| `NotReadyError` | `ERROR_CODE_NOT_READY` |
