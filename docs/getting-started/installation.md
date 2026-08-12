# Installation

!!! danger "Alpha software — not ready for production"
    vstimd is in **early alpha**. The APIs, wire protocol, and behaviour can change at
    any time, features are incomplete, and it has **not** been validated for experiments
    or data collection. Use it for evaluation and development only — **do not rely on it
    in production yet**.

## Which path?

| You want to… | Use |
|---|---|
| Stand up a Raspberry Pi 5 rig from scratch | [The SD card image](#raspberry-pi-5-sd-card-image) |
| Install on an existing Debian/Ubuntu or Fedora/RHEL machine, and keep it updated | [The apt archive](#apt-archive-debian-ubuntu) or [a release package](#release-packages-deb-rpm) |
| Hack on vstimd | [Build from source](#build-from-source) |
| Talk to a rig from your experiment PC | [The Python client](#python-client) — nothing else needed |

Every release publishes its artifacts on the
[GitHub releases page](https://github.com/braemons/vstimd/releases): `.deb`s for
amd64 and arm64, `.rpm`s for x86_64 and aarch64, and a ready-to-flash Raspberry
Pi 5 image. Tags containing a hyphen (`v0.1.0-alpha6`) are published as
**pre-releases**, so tick *Show pre-releases* while the project is in alpha.

---

## Server

### Raspberry Pi 5 SD card image

The fastest path to a working rig: `vstimd-<version>-raspios-lite-arm64.img.xz`
is Raspberry Pi OS Lite with vstimd, `gpiochip-daqd`, SSH, Samba and mDNS
already set up, booting straight into `vstimd.target`.

Full walkthrough — flashing with balenaEtcher, first boot, SSH and SMB access:
**[Raspberry Pi 5 appliance image](../operations/raspberry-pi-image.md)**.

### apt archive (Debian / Ubuntu)

Packages are served from the shared **braemons** archive at
<https://braemons.github.io/packages/>, which carries every braemons daemon —
one source entry and one key per rig, however many daemons it runs. This is the
option that gets you in-place upgrades with `apt upgrade`.

Setup instructions (adding the source and the signing key, and the `stable` vs
`testing` suites) live in the
[archive README](https://github.com/braemons/packages#using-it). Then:

```sh
sudo apt update
sudo apt install braemons-vstimd

# On rigs that drive GPIO trigger lines:
sudo apt install braemons-gpiochip-daqd
```

Rigs on a closed lab network can point at an `rsync`'d mirror of the archive
instead; signatures still verify, because they cover the archive contents rather
than where it was fetched from.

Images flashed from a recent release already have this configured.

### Release packages (.deb / .rpm)

For a one-off install without adding the archive, take the packages straight
from a [release](https://github.com/braemons/vstimd/releases):

=== "Debian / Ubuntu"

    ```sh
    sudo apt install ./braemons-vstimd_<version>-1_arm64.deb
    ```

    `apt install ./file.deb` rather than `dpkg -i` so dependencies are resolved.
    Architectures published: `amd64`, `arm64`.

=== "Fedora / RHEL"

    ```sh
    sudo dnf install ./braemons-vstimd-<version>-1.aarch64.rpm
    ```

    Architectures published: `x86_64`, `aarch64`.

Both formats install the binary with the web UI embedded, the systemd units,
the hostname/discovery service, an rsyslog + logrotate drop-in, and per-board
example configs. `postinst` creates the `vstimd` system user and registers a
"Boot to vstimd" bootloader entry. Then:

```sh
sudo systemctl enable --now vstimd
```

See [Deployment](../operations/deployment.md) for what to do next on a rig.

!!! note "`braemons-gpiochip-daqd` is a separate package"
    The GPIO trigger daemon ships as its own `.deb`, for both `amd64` and
    `arm64`, and is installed alongside vstimd on rigs that drive hardware
    trigger lines. It is not published as an `.rpm`. Rigs with no GPIO wiring do
    not need it at all.

### Build from source

You need a [Rust toolchain](https://rustup.rs) (stable, edition 2024) and
Node.js ≥ 22 (for the embedded web UI), plus:

=== "Ubuntu / Debian"

    ```sh
    sudo apt install build-essential pkg-config \
        libdrm-dev libudev-dev libinput-dev \
        protobuf-compiler
    ```

=== "Fedora / RHEL"

    ```sh
    sudo dnf install gcc pkg-config \
        libdrm-devel systemd-devel libinput-devel \
        protobuf-compiler
    ```

```sh
git clone https://github.com/braemons/vstimd.git
cd vstimd
make build          # as your user — root has no cargo/rustup in PATH
sudo make install
sudo make setup-user
```

`make build-server` skips the web UI and needs no Node. Full target list,
package builds and the installed file layout: [Building &
packaging](../developer/building.md).

!!! tip "Clone a tag"
    The version is derived from the most recent `v*` git tag — a checkout with
    no tags reachable **fails to build** rather than inventing a version. `git
    fetch --tags` if you cloned shallowly. See [Versioning &
    releasing](../developer/releasing.md).

---

## Python client

Requires Python ≥ 3.12 and [uv](https://docs.astral.sh/uv/).

```sh
cd client/python
uv sync
```

To install into an existing environment:

```sh
pip install ./client/python
```

The `discover` extra adds pure-Python mDNS so `vstimd-client discover` can find
rigs without Avahi installed locally:

```sh
pip install './client/python[discover]'
```

## MATLAB client (planned)

A MATLAB client is planned but does not exist yet. C# / Bonsai clients speak the
same [wire protocol](../developer/protocol.md), as can any language with ZMQ and
protobuf bindings.

## Building the docs

The documentation is built with [MkDocs](https://www.mkdocs.org) (1.x) and the
[Material](https://squidfunk.github.io/mkdocs-material/) theme. The build
environment is declared in `docs/pyproject.toml` and managed with
[uv](https://docs.astral.sh/uv/):

```sh
make docs           # live preview on http://127.0.0.1:8000
make docs-build     # static build to site/, matching the published one
```

The published site is built automatically by
[Read the Docs](https://readthedocs.org) (see `.readthedocs.yaml`), which runs the
same `uv run --project docs mkdocs build`.
