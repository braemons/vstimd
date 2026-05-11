# Visual Stimulation Daemon - vstimd

vstimd is a visual stimulus server with strong guarantees for accurate and
precise frame timing as well as low-latency with a broad compatibility with
clients (Python/psychpy, MATLAB, Bonsai). This is achieved by handling rendering
on a dedicated device and controlling it through cross-platform, cross-language
user-friendly clients.

The primary deployment platform is the [Direct Rendering
Manager](https://en.wikipedia.org/wiki/Direct_Rendering_Manager) (DRM) on NVIDIA
Jetson Orin Nano, Raspberry Pi 5, and other Linux-based systems. Using DRM and
by-passing any windowing system enables stable and low-latency rendering with
few skipped frames.

vstimd combines ideas and concepts from Michael Stephan's 
[StimServer](https://github.com/esi-neuroscience/StimServer) C++ visual stimulus
server and Andreas Kreiter's **VStim** project.  


## Goals

- Spiritual successor to the C++ StimServer, using ZeroMQ in place of Windows named pipes for cross-platform IPC
- GPU-accelerated 2-D and 3-D stimulus rendering via **Vulkan** (ash 0.38)
- Low-latency rendering loop with no blocking on the render thread
- Deterministic event logging for experiment replay and analysis
- Shared-memory position input (gaze/joystick) to avoid ZeroMQ round-trip latency

## Current Status

The IPC pipeline is working end-to-end:

- The server opens a fullscreen window and binds a ZMQ REP socket on `tcp://0.0.0.0:5555`
- Clients send protobuf-encoded `Request` messages; the server dispatches them to the scene and replies with a `Response`
- Stimulus creation (`CreateRect`, `CreateCircle`, `CreateEllipse`), mutation, and system commands are implemented
- The Python client (`client/python/`) wraps the ZMQ + protobuf layer and includes an example script that creates and flashes rectangles
- An egui debug overlay (press **F1**) is rendered via a custom Vulkan renderer (`render/vk/egui/`)

The `extern/` directory contains git submodules for external references during early development.

- `extern/StimServer/` — the original C++ reference implementation
- `extern/psychopy/` — PsychoPy, referenced for stimulus design ideas
- `extern/Psychtoolbox-3/` — Psychtoolbox, referenced for stimulus timing and display control
- `extern/ARCADE/` — ARCADE, referenced for a MATLAB client to the StimServer

## Quick Start

```sh
# Terminal 1 — start the server
cargo run --release
# Press D to spawn demo stimuli (cyan disc + magenta rect)
# Press F1 to toggle the debug overlay (frame timing, stimulus list, command log)

# Terminal 2 — run the flash example
cd client/python
uv run examples/flash_rects.py              # 4 flashes at 2 Hz
uv run examples/flash_rects.py --flashes 8 --hz 4
```

Or drive the server directly from Python:

```python
from vstimd import Connection

with Connection() as conn:
    h = conn.stimuli.create_rect(x=-200, y=0, width=300, height=200, r=1.0, g=0.0, b=0.0)
    conn.stimuli.set_enabled(h, False)
    conn.stimuli.delete(h)
    info = conn.system.query_server_info()
    print(info.version)
```

## Stack

| Crate / Library                                                                  | Role                                                |
| -------------------------------------------------------------------------------- | --------------------------------------------------- |
| [ash 0.38](https://github.com/ash-rs/ash)                                        | Raw Vulkan bindings (both DRM and desktop backends) |
| [ash-window 0.13](https://github.com/ash-rs/ash)                                 | Vulkan surface creation from winit window handle    |
| [winit 0.30](https://github.com/rust-windowing/winit)                            | Cross-platform window and event loop (desktop mode) |
| [kurbo 0.13](https://github.com/linebender/kurbo)                                | Bézier path representation and tessellation         |
| [prost 0.13](https://github.com/tokio-rs/prost)                                  | Protobuf encode/decode                              |
| [zeromq 0.4](https://github.com/zeromq/zmq.rs)                                   | Pure-Rust async ZMQ (no libzmq dependency)          |
| [tokio 1](https://tokio.rs)                                                      | Async runtime for the ZMQ server thread             |
| [bytemuck 1](https://github.com/Lokathor/bytemuck)                               | Safe `&[Vertex]` → `&[u8]` casts for buffer uploads |
| [egui 0.34](https://github.com/emilk/egui) + egui-winit + custom Vulkan renderer | Debug overlay (`render/vk/egui/`)                   |
| [pyzmq](https://pyzmq.readthedocs.io)                                            | Python ZMQ bindings (client)                        |
| [protobuf](https://pypi.org/project/protobuf/)                                   | Python protobuf runtime (client)                    |

## Architecture

### Server

The server runs two concurrent threads sharing `Arc<RwLock<SceneState>>`:

| Thread             | Role                                                                                    |
| ------------------ | --------------------------------------------------------------------------------------- |
| **winit / render** | Tessellates stimuli, uploads to GPU, presents frames at vsync                           |
| **ZMQ server**     | Receives protobuf requests, dispatches to `SceneState::handle_request`, sends responses |

The ZMQ thread holds the write lock only while dispatching one command; the render thread can always acquire it between frames.

### Wire Protocol

`proto/v1/` defines the schema across four files (`common.proto`, `service.proto`, `stimuli.proto`, `system.proto`). All messages are protobuf-encoded over a ZMQ REQ/REP socket pair.

**Creation (system target → returns new handle):**

| Command         | Effect               |
| --------------- | -------------------- |
| `CreateRect`    | Create a rectangle   |
| `CreateCircle`  | Create a disc/circle |
| `CreateEllipse` | Create an ellipse    |

**Stimulus commands (handle > 0):**

| Command           | Effect                 |
| ----------------- | ---------------------- |
| `SetEnabled`      | Show / hide a stimulus |
| `Delete`          | Remove a stimulus      |
| `SetPosition`     | Move to (x, y)         |
| `SetOrientation`  | Set rotation angle     |
| `SetFillColor`    | Set fill RGBA          |
| `SetAlpha`        | Set opacity            |
| `SetOutlineColor` | Set outline RGBA       |
| `SetOutlineWidth` | Set outline line width |
| `SetRectSize`     | Resize a rectangle     |
| `SetDiscRadius`   | Resize a disc          |
| `SetEllipseSize`  | Resize an ellipse      |
| `QueryStimulus`   | Query current state    |

**System commands (system target):**

| Command           | Effect                                       |
| ----------------- | -------------------------------------------- |
| `SetBackground`   | Set background color                         |
| `SetDeferredMode` | Batch-commit multiple mutations atomically   |
| `DeleteAll`       | Remove all stimuli                           |
| `SetAllEnabled`   | Show / hide all stimuli                      |
| `QueryServerInfo` | Get display size, frame rate, server version |

### Python Client

`client/python/vstimd/` is the Python package. `Connection` exposes two sub-clients:

- `conn.stimuli` — `StimuliClient`: create, mutate, and delete individual stimuli
- `conn.system` — `SystemClient`: scene-wide mutations and server queries

Protobuf stubs in `_proto/` are generated from the four `proto/v1/` schema files.

### Planned Architecture

See [`docs/PLAN.md`](docs/PLAN.md) for the full design and roadmap.

## Building

```sh
# Rust server
cargo build
cargo build --release
cargo test
cargo clippy

# Run options
cargo run --release                   # fullscreen (auto-detects DRM or desktop)
cargo run --release -- --windowed 1280x720
cargo run --release -- --null         # ZMQ only, no display (also: VSTIMD_NULL=1)

# Python client (requires uv)
cd client/python
uv sync
uv run examples/flash_rects.py
```

## Relationship to VStim

| Version            | Language  | Renderer                           | Notes                                                                                        |
| ------------------ | --------- | ---------------------------------- | -------------------------------------------------------------------------------------------- |
| VStim v1           | C++ / MFC | Direct3D 9                         | Original monolithic stimulus software                                                        |
| VStim v2           | C++ / MFC | Direct3D 11                        | Monolithic rewrite with improved renderer                                                    |
| StimServer         | C++ / MFC | Direct3D 11                        | Standalone server with client/server architecture over Windows named pipes (binary protocol) |
| vstimd (this repo) | Rust      | Vulkan (ash 0.38, Linux + Windows) | Rust rewrite combining VStim and StimServer, cross-platform                                  |
