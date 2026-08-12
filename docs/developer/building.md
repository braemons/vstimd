# Building & packaging

This page covers producing deployable artifacts — binaries with the embedded web
UI, `.deb`/`.rpm` packages, and the Docker-based builders and integration test. To
*install and run* a finished package on a rig, see
[Deployment](../operations/deployment.md).

!!! info "Where does the version come from?"
    The git tag, and nothing else — there is no number to bump in `Cargo.toml`,
    which carries a `0.0.0` sentinel. See
    [Versioning & releasing](releasing.md) before you go looking for it.

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
| `make web` | Build only the React bundle |
| `make install` | Install the pre-built binary, scripts, units, sysusers conf, rig config and examples to `$(DESTDIR)$(PREFIX)/…` (run `make build` first — does *not* rebuild, so no cargo needed under sudo) |
| `make uninstall` | Stop, disable, and remove all installed files |
| `make setup-user` | Create the `vstimd` system user via `systemd-sysusers` |
| `make deb-amd64` / `deb-arm64` / `deb` | Build `.deb`s in Docker → `dist/` |
| `make rpm-amd64` / `rpm-arm64` / `rpm` | Build `.rpm`s in Docker → `dist/` |
| `make packages` | Everything: `deb` + `rpm` |
| `make image` | [Raspberry Pi SD card image](#raspberry-pi-sd-image) → `dist/` |
| `make print-version` | The version this checkout resolves to |
| `make docs` / `docs-build` | MkDocs live preview / strict static build |

Override defaults with variables:

```bash
sudo make install PREFIX=/usr/local UNITDIR=/usr/local/lib/systemd/system
```

`make install` refuses to install a binary with no embedded web UI — a guard
against `make dev` / a bare `cargo build` having quietly replaced
`target/release/vstimd` since the last `make build`. Set `VSTIMD_ALLOW_NO_UI=1` if
that is what you actually want.

### Installed file layout

| Source | Installed path |
|---|---|
| `target/release/vstimd` | `/usr/bin/vstimd` |
| `packaging/scripts/vstimd-boot-entry` | `/usr/sbin/vstimd-boot-entry` |
| `packaging/scripts/vstimd-set-hostname` | `/usr/sbin/vstimd-set-hostname` |
| `packaging/systemd/vstimd.service` | `/usr/lib/systemd/system/vstimd.service` |
| `packaging/systemd/vstimd.target` | `/usr/lib/systemd/system/vstimd.target` |
| `packaging/systemd/vstimd-hostname.service` | `/usr/lib/systemd/system/vstimd-hostname.service` |
| `packaging/sysusers/vstimd.conf` | `/usr/lib/sysusers.d/vstimd.conf` |
| `packaging/avahi/vstimd.service.tmpl` | `/usr/share/braemons/vstimd/vstimd.service.avahi.tmpl` |
| `server/config/default-rig-config.toml` | `/etc/braemons/vstimd-rig-config.toml` (never overwritten if present) |
| `server/config/{jetson-orin-nano,raspberry-pi-5,raspberry-pi-4}.toml` | `/usr/share/braemons/vstimd/` |

The packages install the same paths, but each declares them itself rather than
shelling out to `make install`: the `.deb` from the `assets` list in
`[package.metadata.deb]` (`server/Cargo.toml`), the `.rpm` from the `%install`
block in `packaging/rpm/vstimd.spec`. **Adding a file means adding it in all three
places.** Both packages additionally ship `/etc/rsyslog.d/10-vstimd.conf` and
`/etc/logrotate.d/vstimd`, which `make install` does not; the `.rpm` currently
ships no rig config and no per-board examples.

## Build the `.deb`

### Docker (recommended)

The container builds everything itself — Rust, Node (for the embedded web UI), and
the packaging tools — so no host toolchain or prior `cargo build` is needed.

```bash
make deb-amd64        # → dist/braemons-vstimd_<version>-1_amd64.deb
                      #   + dist/braemons-gpiochip-daqd_<version>-1_amd64.deb
```

Invoking the builder by hand means passing the version yourself — a Docker build
context has no `.git`, so nothing inside the container can derive one:

```bash
docker build -f packaging/docker/Dockerfile.deb-builder \
    --build-arg VSTIMD_VERSION=$(make -s print-version) \
    -t vstimd-deb-builder .
docker run --rm -v $(pwd)/dist:/output vstimd-deb-builder
```

### Cross-compile for arm64 (Jetson / Raspberry Pi)

The Docker builder handles the arm64 cross-compile and the host-arch web UI build in
one step:

```bash
make deb-arm64        # → dist/…_arm64.deb, both packages
```

### Native

`cargo-deb` does the packaging (there is no `debian/control`/`rules` — only the
maintainer scripts live in `packaging/debian/`). With `cargo-deb` installed:

```bash
make deb-assemble VSTIMD_VERSION=$(make -s print-version)
# → target/debian/braemons-vstimd_<version>-1_<arch>.deb
#   plus braemons-gpiochip-daqd_…
```

`deb-assemble` is the target the container runs; invoking it directly builds for
the host architecture. `postinst` calls `systemd-sysusers` to create the `vstimd`
user, creates `/etc/braemons` and `/var/log/vstimd`, and registers the "Boot to
vstimd" bootloader entry.

## Build the `.rpm`

### Docker (recommended)

Same shape as the `.deb` builders — the container compiles and packages, so no
host `rpmbuild` is needed:

```bash
make rpm-amd64        # → dist/braemons-vstimd-<version>-1.x86_64.rpm
make rpm-arm64        # → dist/braemons-vstimd-<version>-1.aarch64.rpm
```

### Native

`packaging/rpm/vstimd.spec` does not compile anything — it packages a pre-built
binary, and it refuses to build without an explicit version:

```bash
make build
rpmbuild -bb packaging/rpm/vstimd.spec \
    --define "_builddir $(pwd)" \
    --define "pkg_version $(make -s print-version)"
```

## Raspberry Pi SD image

`make image` produces the ready-to-flash Raspberry Pi OS Lite (arm64) appliance
image that ships with every release — see
[Raspberry Pi 5 appliance image](../operations/raspberry-pi-image.md) for what
ends up inside it and how to flash it.

```bash
make image        # → dist/vstimd-<version>-raspios-lite-arm64.img.xz (+ .sha256)
```

It depends on `deb-arm64`, then runs `packaging/image/build-sd-image.sh` inside
`Dockerfile.image-builder`, which loop-mounts a stock Raspberry Pi OS image and
configures it in an arm64 chroot under `qemu-user-static`. Notes:

- Needs `--privileged` (loop devices, chroot) and a host with binfmt handlers
  registered: `docker run --rm --privileged multiarch/qemu-user-static --reset -p yes`.
- Slow — most of the time goes on DKMS-building the evdi module for every kernel
  in the image under emulation. Budget an hour or more; the release job allows five.
- Needs ~10 GB free: a ~6.5 GB working image plus the compressed output.
- The base image is cached in `packaging/image/.cache/`; `FORCE_DOWNLOAD=1`
  refreshes it.
- The login is `VSTIMD_IMAGE_USER` / `VSTIMD_IMAGE_PASSWORD` (default
  `vstimd-admin` / `vstimd`, password change forced at first login). Passing an
  empty password generates a random one per build and writes it to
  `dist/…-credentials.txt` — **that file must never be published**; the release
  workflow uploads only `*.img.xz*` for exactly this reason.
- Without `packaging/apt/braemons-archive-keyring.asc` the build still succeeds,
  printing `apt updates: DISABLED`, and produces an image with no update source.

## Docker integration test

Tests the full install + systemd lifecycle using the null renderer (no GPU
required). Requires Docker with cgroup v2 support and the `.deb` already built.

```bash
# 1. Build the .deb
make deb-amd64

# 2. Build the test image
docker build -f packaging/docker/Dockerfile.test-deb -t vstimd-test-deb .

# 3. Run the test (privileged required for systemd)
packaging/docker/run-test.sh
```

`packaging/docker/test-service.sh` exercises: `dpkg -i` installs cleanly; the
`Type=notify` handshake succeeds within 20 s; ZMQ port 5555 is reachable; a clean
SIGTERM shutdown leaves no zombie process.

## In CI

Two workflows, with a clean split:

| Workflow | Trigger | What it does |
|---|---|---|
| `ci.yml` | every push / PR | `cargo build --release`, `cargo test`, `clippy -D warnings`, Python unit tests + typecheck, and the e2e suite against both the null renderer and a real build |
| `release.yml` | `v*` tags, or `workflow_dispatch` | The four package builds and the SD image, in parallel, published to a GitHub Release |

Packaging is therefore **not** exercised on every PR — a change under
`packaging/` is best checked locally with `make deb-amd64` and
`packaging/docker/run-test.sh`, or by a manual `workflow_dispatch` run of
`release.yml` (which builds everything and uploads workflow artifacts without
cutting a release).

Still worth adding: a `make install` `DESTDIR` smoke test, and running
`packaging/docker/run-test.sh` in CI to catch install + systemd lifecycle
regressions.

See [Versioning & releasing](releasing.md) for what a tag produces and how it
reaches the apt archive.
