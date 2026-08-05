# Building & packaging

This page covers producing deployable artifacts — binaries with the embedded web
UI, `.deb`/`.rpm` packages, and the Docker-based builders and integration test. To
*install and run* a finished package on a rig, see
[Deployment](../operations/deployment.md).

## Versioning

**The git tag is the only place the version is defined.** There is no number to
bump in `Cargo.toml`: all three crates inherit a deliberately invalid `0.0.0`
sentinel from `[workspace.package]`, and the real version is stamped at build
time by [`packaging/scripts/git-version.sh`](https://github.com/braemons/vstimd/blob/main/packaging/scripts/git-version.sh).

Cargo has no equivalent of Python's `setuptools-scm`: `package.version` must be a
literal, and a build script cannot change it because the version is resolved into
the dependency graph before build scripts run. So rather than keep a hand-edited
number that would drift out of step with the tag, the manifests admit they do not
know. `vstimd --version` reports a value injected by `server/build.rs`, and the
package versions come from the `Makefile`.

Releasing is therefore just tagging:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

The tag is normalised for dpkg and rpm, neither of which allows `-` inside a
version:

| Git tag | Package version | Notes |
| --- | --- | --- |
| `v0.2.0` | `0.2.0` | |
| `v0.2.0-alpha1` | `0.2.0~alpha1` | `~` sorts *before* `0.2.0`, so the release supersedes the alpha |
| 2 commits past `v0.2.0-alpha1` | `0.2.0~alpha1+2.gabc1234` | `+` sorts *after* the bare tag |
| …with uncommitted changes | `…+dirty` | |

Nothing falls back to a default. A build with no reachable tag and no explicit
version **fails**, because a package that quietly claims a made-up version is
worse than a package that failed to build. Building outside a tagged checkout —
which is what the packaging containers do, since a Docker build context has no
`.git` — means saying so:

```bash
make deb-arm64 VSTIMD_VERSION=0.2.0
```

If you ever see an artifact versioned `0.0.0`, the stamping was bypassed.

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
