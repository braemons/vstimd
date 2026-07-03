# Building & packaging

This page covers producing deployable artifacts — binaries with the embedded web
UI, `.deb`/`.rpm` packages, and the Docker-based builders and integration test. To
*install and run* a finished package on a rig, see
[Deployment](../operations/deployment.md).

## Build a deployable binary

The repo ships a `Makefile` whose `install` target is `DESTDIR`-aware — the same
target the `.deb` and `.rpm` backends use.

```bash
# Build the React UI (client/web) and bake it into the binary via the embed-ui
# feature, so the running server serves the full control UI at http://<device>:8080.
# Requires Node/npm + cargo. Run WITHOUT sudo: root has no cargo/rustup in PATH.
make build

# Server only — no embedded UI, no Node needed (serves a placeholder page at /,
# WebSocket API still functional):
make build-server
```

### Makefile targets

| Target | Effect |
|---|---|
| `make build` | Build the React UI, then `cargo build --release --features embed-ui` (deployable binary with the browser UI baked in) |
| `make build-server` | `cargo build --release` — server only, no embedded UI, no Node needed |
| `make install` | Install the pre-built binary, unit file, and sysusers conf to `$(DESTDIR)$(PREFIX)/…` (run `make build` first — does *not* rebuild, so no cargo needed under sudo) |
| `make uninstall` | Stop, disable, and remove all installed files |
| `make setup-user` | Create the `vstimd` system user via `systemd-sysusers` |

Override defaults with variables:

```bash
sudo make install PREFIX=/usr/local UNITDIR=/usr/local/lib/systemd/system
```

### Installed file layout

| Source | Installed path |
|---|---|
| `target/release/vstimd` | `/usr/bin/vstimd` |
| `packaging/systemd/vstimd.service` | `/usr/lib/systemd/system/vstimd.service` |
| `packaging/sysusers/vstimd.conf` | `/usr/lib/sysusers.d/vstimd.conf` |

Both `.deb` and `.rpm` backends delegate file installation to `make install` via
`DESTDIR`, so the layout above is always consistent.

## Build the `.deb`

### Docker (recommended)

The container builds everything itself — Rust, Node (for the embedded web UI), and
the packaging tools — so no host toolchain or prior `cargo build` is needed.

```bash
make deb-amd64        # → dist/braemons-vstimd_<version>-1_amd64.deb

# …or invoke the builder directly:
docker build -f packaging/docker/Dockerfile.deb-builder -t vstimd-deb-builder .
docker run --rm -v $(pwd)/dist:/output vstimd-deb-builder
```

### Cross-compile for arm64 (Jetson / Raspberry Pi)

The Docker builder handles the arm64 cross-compile and the host-arch web UI build in
one step:

```bash
make deb-arm64        # → dist/braemons-vstimd_<version>-1_arm64.deb
```

### Native

Requires `debhelper` ≥ 13 and `dpkg-dev` (plus Node/npm for the UI) on the host.
`dpkg-buildpackage` expects `debian/` at the repo root, so symlink it first:

```bash
make build                       # embedded binary → target/release/vstimd
ln -sf packaging/debian debian
dpkg-buildpackage -b --no-sign
rm debian
# ../braemons-vstimd_<version>-1_amd64.deb
```

`postinst` calls `systemd-sysusers` to create the `vstimd` user and warns if a
display manager is enabled on the same VT.

## Build the `.rpm`

`packaging/rpm/vstimd.spec` uses the same `make install DESTDIR=…` backend. It needs
a pre-built binary and `rpmbuild`:

```bash
make build
rpmbuild -bb packaging/rpm/vstimd.spec \
    --define "_sourcedir $(pwd)/target/release"
```

A Docker-based `.rpm` builder (mirroring `Dockerfile.deb-builder`) and a Fedora
integration test are planned but not yet implemented.

## Docker integration test

Tests the full install + systemd lifecycle using the null renderer (no GPU
required). Requires Docker with cgroup v2 support and the `.deb` already built.

```bash
# 1. Build binary and .deb
cargo build --release
docker build -f packaging/docker/Dockerfile.deb-builder -t vstimd-deb-builder .
docker run --rm -v $(pwd)/packaging:/output vstimd-deb-builder

# 2. Build the test image
docker build -f packaging/docker/Dockerfile.test-deb -t vstimd-test-deb .

# 3. Run the test (privileged required for systemd)
packaging/docker/run-test.sh
```

`packaging/docker/test-service.sh` exercises: `dpkg -i` installs cleanly; the
`Type=notify` handshake succeeds within 20 s; ZMQ port 5555 is reachable; a clean
SIGTERM shutdown leaves no zombie process.

## CI packaging (roadmap)

The GitHub Actions pipeline currently builds, tests, and runs the null-renderer e2e
suite. Planned additions:

- **`make install` smoke test** — verify the install target places files correctly
  via `DESTDIR`, without a real system install.
- **`.deb` build job** — run the Docker deb builder on every PR and upload the
  package as an artifact.
- **`.rpm` build job** — same, once the RPM Docker builder exists.
- **Integration test job** — run `packaging/docker/run-test.sh` in CI to catch
  regressions in the full install + systemd lifecycle.
