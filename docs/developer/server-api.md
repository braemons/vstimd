# Server API (Rust)

The server internals are documented via `cargo doc`. Generate and open them locally:

```sh
cd server
cargo doc --no-deps --open
```

## Key types

| Type | Module | Description |
|---|---|---|
| `SceneState` | `scene::scene_state` | All stimulus data; the only mutable state shared between the render and ZMQ threads |
| `Stimulus` | `scene::stimulus` | Enum of all stimulus variants |
| `RigConfig` | `rig_config` | Device-level config loaded from `/etc/braemons/vstimd-rig-config.toml` (display mode, web server, overlay scale) |

`SceneState::handle_request` is the single entry point the ZMQ thread calls for every
command; integration tests in `server/tests/` call it directly without a GPU or ZMQ
(see the [`lib.rs`](architecture.md#scene-state) note).

## Build & test

```sh
cargo build --release
cargo test
cargo clippy

# Null renderer — ZMQ server only, no display (also: VSTIMD_NULL=1)
cargo run --release -- --null

# Desktop windowed (default is fullscreen)
cargo run --release -- --windowed 1280x720
```

## Contributing

See [Installation → build from source](../getting-started/installation.md#build-from-source)
for the toolchain and system dependencies, and [Building & packaging](building.md)
for producing deployable binaries and packages.
