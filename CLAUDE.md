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
- Stimulus types: `Stimulus { common, body }` — shared state (flags, opacity) above a
  `StimulusBody` enum, composition throughout (not trait objects or inheritance)
- `StimulusBody` is the **renderer's** taxonomy (one arm per pipeline/cache: `Shape`,
  `Grating`, `Text`, `Mesh3d`); the finer user-facing names (`Rect`, `Circle`, `Cube3D`)
  live in the geometry enums. Internal body names must never reach a client — the
  user-facing taxonomy is the native `scene::StimulusType`, which every geometry maps
  to via `stimulus_type()` and which owns the only client-visible spelling
  (`type_name()`). `ipc/convert` maps it to the wire enum, exhaustively, so adding a
  type is a compile error until its wire value is chosen. It is called `Body` and not
  `Kind` deliberately: it carries the stimulus data, and `Kind` reads as a synonym of
  `StimulusType` while being strictly coarser (all four shapes are one `Shape` arm).
  Never name a client-facing thing `kind`
- The config format *is* the runtime shape (no DTO). Types owning runtime state
  (`StimulusFlags`, `Grating`, `Text`) hide it behind a `serde` impl delegating to an inner
  `*Config`; GPU resources never live in the scene tree at all
- Render thread must never block or heap-allocate on event emission
- ZMQ bind address: `tcp://0.0.0.0:5555` — `tcp://*:5555` fails (zeromq crate resolves host as DNS)
- 2-D and 3-D coexist in one frame (3-D rendered first, 2-D overlaid)

**Module layout (`server/src/`):**
- `ipc/` — ZMQ transport plus the protobuf dispatcher. `handle_request` is an inherent method on
  `SceneState` split across `dispatch.rs` (routing + command summary) and one `*_commands.rs` per
  domain. A new command needs an arm in `dispatch.rs` and the body in its group module.
- `ipc/convert/` — **every** proto <-> scene conversion: one submodule per stimulus body
  (`grating.rs`, `text.rs`), one per non-stimulus domain (`animation.rs`, `vtl.rs`), and the
  shared ones in `mod.rs`. A conversion belongs here, never in `scene/` and never inside a
  `*_commands.rs`. Names are `X_from_proto` / `X_to_proto`, in that direction — nothing else,
  so the direction of a call is readable rather than looked up. Decode an enum as
  `Enum::try_from(v).unwrap_or(Unspecified)`.
- `proto.rs` stays at the crate root, not under `ipc/` — the scene and the web surface speak it too.
- `scene/` — state only; nothing here speaks protobuf.

**Two configs — always name which one.** They are unrelated:
- **rig-config** (`rig_config.rs`) — the physical rig: VTL shm, display mode, thread scheduling.
  TOML at `/etc/braemons/vstimd-rig-config.toml`, changes when the hardware does.
- **scene-config** (`scene_config_file.rs`) — one experiment: a `SceneConfig` (stimuli, animations,
  background, photodiode) plus named VTL trigger lines. JSON at
  `<config-dir>/vstimd_<name>.config.json`, changes per session. The payload type lives in
  `scene/scene_config.rs`; the load/save methods are on `SceneState` in `scene/scene_state.rs`.
- `render/` — the display application. `overlay_ui/` holds the egui overlay, one file per group
  under `overlay_ui/panels/`; `vk/` is the only Vulkan layer.
- `input/`, `system_info.rs`, `system_metrics.rs`, `benchmark.rs` are peers of `render/`, not part
  of it.
- `process/` — shutdown flag and render-thread scheduling. `log_buffer.rs` stays at the root:
  it is the log sink the overlay reads, not process management.

**Threading:** Two threads share `Arc<RwLock<SceneState>>`. Render thread holds write (tessellation) then read (draw); ZMQ thread holds write (one command at a time). The write lock is dropped before render acquires read, so ZMQ always has a window between frames.

**`lib.rs`** exposes all modules as a library crate so integration tests in `server/tests/` can call `SceneState::handle_request` directly without GPU or ZMQ.

**Jetson Nano DRM:** `VK_EXT_acquire_drm_display` doesn't work (split GPU/display nodes). Use `VK_KHR_display` instead.
