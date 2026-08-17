# CLAUDE.md

## Build & Run

```bash
cargo build --release
cargo test
cargo clippy

# Null renderer — ZMQ server only, no display (also: VSTIMD_NULL=1)
cargo run --release -- --null

# Desktop windowed (default is fullscreen)
cargo run --release -- --windowed 1280x720
```

## Python Client

**Always use `make` targets** — do not construct raw `uv run` commands manually:

```bash
cd client/python
make proto          # regenerate protobuf stubs from proto/
make test           # unit tests
make test-e2e       # e2e tests (requires running server)
make test-e2e-null  # e2e tests against null renderer
make typecheck      # ty type checking
```

## Architecture

See `docs/PLAN.md` for the full design and roadmap.

**Key decisions:**
- Stimulus types: flat `enum` with composition (not trait objects or inheritance)
- Render thread must never block or heap-allocate on event emission
- ZMQ bind address: `tcp://0.0.0.0:5555` — `tcp://*:5555` fails (zeromq crate resolves host as DNS)
- 2-D and 3-D coexist in one frame (3-D rendered first, 2-D overlaid)

**Module layout (`server/src/`):**
- `ipc/` — ZMQ transport plus the protobuf dispatcher. `handle_request` is an inherent method on
  `SceneState` split across `dispatch.rs` (routing + command summary) and one `*_commands.rs` per
  domain. A new command needs an arm in `dispatch.rs` and the body in its group module.
- `proto.rs` stays at the crate root, not under `ipc/` — the scene and the web surface speak it too.
- `scene/` — state only; nothing here speaks protobuf. `scene_config.rs` is the serializable
  `SceneConfig` type; the named-config load/save methods are on `SceneState` in `scene_state.rs`;
  the file format and directory layout are in `io_config.rs` at the root.
- `render/` — the display application. `overlay_ui/` holds the egui overlay, one file per group
  under `overlay_ui/panels/`; `vk/` is the only Vulkan layer.
- `input/`, `system_info.rs`, `system_metrics.rs`, `benchmark.rs` are peers of `render/`, not part
  of it.

**Threading:** Two threads share `Arc<RwLock<SceneState>>`. Render thread holds write (tessellation) then read (draw); ZMQ thread holds write (one command at a time). The write lock is dropped before render acquires read, so ZMQ always has a window between frames.

**`lib.rs`** exposes all modules as a library crate so integration tests in `server/tests/` can call `SceneState::handle_request` directly without GPU or ZMQ.

**Jetson Nano DRM:** `VK_EXT_acquire_drm_display` doesn't work (split GPU/display nodes). Use `VK_KHR_display` instead.
