# Building vstimd

## Linux

### Ubuntu / Debian

```sh
sudo apt install build-essential pkg-config \
    libdrm-dev libudev-dev libinput-dev \
    protobuf-compiler
```

### Fedora / RHEL

```sh
sudo dnf install gcc pkg-config \
    libdrm-devel systemd-devel libinput-devel \
    protobuf-compiler
```

### Rust toolchain

Install via [rustup](https://rustup.rs/) — the package-manager version of Rust is often too old:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The required toolchain version is specified in `rust-toolchain.toml` and will be installed automatically on first `cargo` invocation.

## Building

```sh
cargo build --release
cargo test
cargo clippy
```

## Running

```sh
# Auto-detects DRM (bare-metal) or desktop (X11/Wayland) mode
cargo run --release

# Windowed desktop mode
cargo run --release -- --windowed 1280x720

# Null renderer — ZMQ server only, no display
cargo run --release -- --null
```

## Python client

Requires [uv](https://docs.astral.sh/uv/) and `make`. Generate the protobuf stubs before first use:

```sh
cd client/python
uv sync
make proto
uv run examples/flash_rects.py
```

See `client/python/Makefile` for additional targets (`test`, `test-e2e`, `build`, etc.).
