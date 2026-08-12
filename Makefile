PREFIX      ?= /usr
UNITDIR     ?= /lib/systemd/system
SYSUSERSDIR ?= /usr/lib/sysusers.d
CONFDIR     ?= /etc/braemons
SHAREDIR    ?= /usr/share/braemons/vstimd
# Cross-compile target. Empty = build for the host, which is what a developer
# running `make build` wants. The packaging containers set it so they can reuse
# these targets instead of restating the cargo invocations.
RUST_TARGET ?=
CARGO_TARGET_ARG := $(if $(RUST_TARGET),--target $(RUST_TARGET),)
BINARY      := target/$(if $(RUST_TARGET),$(RUST_TARGET)/,)release/vstimd
SERVICE     := packaging/systemd/vstimd.service
TARGET_UNIT := packaging/systemd/vstimd.target
HOSTNAME_UNIT   := packaging/systemd/vstimd-hostname.service
BOOT_SCRIPT     := packaging/scripts/vstimd-boot-entry
HOSTNAME_SCRIPT := packaging/scripts/vstimd-set-hostname
SYSUSERS    := packaging/sysusers/vstimd.conf
AVAHI_TEMPLATE  := packaging/avahi/vstimd.service.tmpl
# Samba share definitions. Installed read-only as an example: samba is only a
# Suggests, and nothing here activates until an admin adds the `include =` line
# to smb.conf (see the file's own header).
SAMBA_SHARES    := packaging/samba/vstimd-shares.conf
RIG_CONFIG  := server/config/default-rig-config.toml
EXAMPLES    := server/config/jetson-orin-nano.toml \
               server/config/raspberry-pi-5.toml \
               server/config/raspberry-pi-4.toml

DIST_DIR            ?= dist
DEB_BUILDER_IMAGE   ?= vstimd-deb-builder
RPM_BUILDER_IMAGE   ?= vstimd-rpm-builder
IMAGE_BUILDER_IMAGE ?= vstimd-image-builder
IMAGE_CACHE_DIR     ?= packaging/image/.cache

# The version of every artifact, from the one place it is defined: the git tag.
# The Cargo manifests carry a 0.0.0 sentinel because Cargo cannot derive a
# version from the repo (no setuptools-scm equivalent) — see the root
# Cargo.toml. Both crates ship in lockstep, so both get this same version.
#
# Override to build outside a tagged checkout; the packaging containers do
# exactly that, since a Docker build context has no .git:
#   make deb-arm64 VSTIMD_VERSION=0.1.0
#
# An explicit value short-circuits the script rather than being handed to it:
# the rpm builder's compile stage has no packaging/ dir to run it from.
#
# The outer $(strip) is load-bearing: the line continuation below puts a space
# into the else-branch, which then lands inside `--build-arg VSTIMD_VERSION=`
# and breaks the docker invocation. `echo` hides it, so print-version looks fine.
VSTIMD_VERSION ?=
RESOLVED_VERSION := $(strip $(if $(strip $(VSTIMD_VERSION)),$(strip $(VSTIMD_VERSION)),\
                      $(shell packaging/scripts/git-version.sh 2>/dev/null)))

# Missing version = hard stop, never a default: every path below bakes VERSION
# into a filename and into package metadata, so continuing with an invented one
# would produce artifacts that lie about what they are.
#
# Recursive (`=`, not `:=`) so the error fires where VERSION is *used* rather
# than when this file is parsed. $(or) does not evaluate later arguments once
# one is non-empty, so the error is inert whenever a version is available. That
# keeps version-free targets — `web`, `clean`, `docs` — working in a checkout
# with no tags, which is what lets the packaging containers build the web bundle
# before the version is introduced (see Dockerfile.deb-builder).
VERSION = $(or $(RESOLVED_VERSION),$(error Cannot determine the version. Run \
packaging/scripts/git-version.sh to see why, or pass VSTIMD_VERSION=<version>))

REVISION ?= 1

# Must match [package.metadata.deb] name in server/Cargo.toml / gpiochip-daqd/Cargo.toml
DEB_NAME      := braemons-vstimd
GPIOCHIP_DEB_NAME := braemons-gpiochip-daqd

# Recursive (`=`) so they do not expand VERSION — and so do not trip its error —
# until a target that actually names one of these paths runs.
DEB_AMD64 = $(DIST_DIR)/$(DEB_NAME)_$(VERSION)-$(REVISION)_amd64.deb
DEB_ARM64 = $(DIST_DIR)/$(DEB_NAME)_$(VERSION)-$(REVISION)_arm64.deb
GPIOCHIP_DEB_ARM64 = $(DIST_DIR)/$(GPIOCHIP_DEB_NAME)_$(VERSION)-$(REVISION)_arm64.deb
RPM_AMD64 = $(DIST_DIR)/$(DEB_NAME)-$(VERSION)-$(REVISION).x86_64.rpm
RPM_ARM64 = $(DIST_DIR)/$(DEB_NAME)-$(VERSION)-$(REVISION).aarch64.rpm

# Login user/password baked into `make image`'s SD card image (SSH + Samba).
# A known default, so a freshly flashed card is reachable without hunting for
# a build log. The image still forces a password change at first login
# (`chage -d 0`), which is what keeps it from staying valid in the field.
# Set VSTIMD_IMAGE_PASSWORD="" to auto-generate a random one per build
# instead — see packaging/image/build-sd-image.sh.
VSTIMD_IMAGE_USER     ?= vstimd-admin
VSTIMD_IMAGE_PASSWORD ?= vstimd

# Version string in the SD image filename. Defaults to the same git-derived
# version as the packages, so a downloaded .img.xz says which release it is.
IMAGE_VERSION ?= $(VERSION)

RUST_SRCS     := Cargo.toml Cargo.lock $(shell find server/src vtl/src proto -type f 2>/dev/null)
# 2>/dev/null to match RUST_SRCS: the Makefile is now also evaluated inside the
# packaging containers, and the rpm builder's first stage has no packaging/.
PKG_SRCS      := $(shell find packaging -type f 2>/dev/null)

WEB_DIR  := client/web
WEB_DIST := $(WEB_DIR)/dist/index.html
WEB_SRCS := $(shell find $(WEB_DIR)/src -type f 2>/dev/null) \
            $(WEB_DIR)/index.html $(WEB_DIR)/package.json $(WEB_DIR)/vite.config.ts

.PHONY: build build-server web install uninstall setup-user \
        docs docs-build \
        deb-amd64 deb-arm64 deb \
        rpm-amd64 rpm-arm64 rpm \
        packages image \
        deb-assemble print-version print-binary

# Build the React bundle that gets baked into the binary (requires Node/npm).
# File target so it only rebuilds when the web sources change.
$(WEB_DIST): $(WEB_SRCS)
	$(MAKE) -C $(WEB_DIR) build

web: $(WEB_DIST)

# Deployable binary WITH the browser UI embedded: serves the React app at
# http://<device>:8080 so any machine on the LAN can control vstimd. Requires
# Node/npm to build the frontend first. Use `build-server` for a UI-less build.
build: web
	VSTIMD_VERSION=$(VERSION) cargo build --release --features embed-ui $(CARGO_TARGET_ARG)

# Server-only binary (no embedded UI, no Node/npm needed). The web control
# surface still runs, but `/` serves a placeholder instead of the React app.
build-server:
	VSTIMD_VERSION=$(VERSION) cargo build --release $(CARGO_TARGET_ARG)

# Install a pre-built binary. Kept separate from `build` so the usual flow is
# `make build` (as your user, with cargo) then `sudo make install` (as root,
# which has no cargo/rustup in PATH). Fails clearly if the binary is missing.
install:
	@test -x $(BINARY) || { echo "error: $(BINARY) not found — run 'make build' first (as your user, not via sudo)"; exit 1; }
	@test -n "$(VSTIMD_ALLOW_NO_UI)" || grep -aq 'id="root"' $(BINARY) || { echo "error: $(BINARY) has no embedded web UI — a dev target (make dev/dev-null, cargo build/run) rebuilt it without the UI. Run 'make build' before installing, or set VSTIMD_ALLOW_NO_UI=1 to install a server-only binary."; exit 1; }
	install -D -m 0755 $(BINARY)          $(DESTDIR)$(PREFIX)/bin/vstimd
	install -D -m 0755 $(BOOT_SCRIPT)     $(DESTDIR)$(PREFIX)/sbin/vstimd-boot-entry
	install -D -m 0755 $(HOSTNAME_SCRIPT) $(DESTDIR)$(PREFIX)/sbin/vstimd-set-hostname
	install -D -m 0644 $(SERVICE)         $(DESTDIR)$(UNITDIR)/vstimd.service
	install -D -m 0644 $(TARGET_UNIT)     $(DESTDIR)$(UNITDIR)/vstimd.target
	install -D -m 0644 $(HOSTNAME_UNIT)   $(DESTDIR)$(UNITDIR)/vstimd-hostname.service
	install -D -m 0644 $(SYSUSERS)        $(DESTDIR)$(SYSUSERSDIR)/vstimd.conf
	install -d -m 0755 $(DESTDIR)$(CONFDIR)
	test -f $(DESTDIR)$(CONFDIR)/vstimd-rig-config.toml || \
	  install -m 0644 $(RIG_CONFIG) $(DESTDIR)$(CONFDIR)/vstimd-rig-config.toml
	install -d -m 0755 $(DESTDIR)$(SHAREDIR)
	for f in $(EXAMPLES); do install -m 0644 $$f $(DESTDIR)$(SHAREDIR)/; done
	install -D -m 0644 $(AVAHI_TEMPLATE)  $(DESTDIR)$(SHAREDIR)/vstimd.service.avahi.tmpl
	install -D -m 0644 $(SAMBA_SHARES)    $(DESTDIR)$(SHAREDIR)/vstimd-shares.conf

uninstall:
	systemctl disable --now vstimd 2>/dev/null || true
	systemctl disable vstimd-hostname 2>/dev/null || true
	vstimd-boot-entry --remove 2>/dev/null || true
	rm -f $(DESTDIR)$(PREFIX)/bin/vstimd
	rm -f $(DESTDIR)$(PREFIX)/sbin/vstimd-boot-entry
	rm -f $(DESTDIR)$(PREFIX)/sbin/vstimd-set-hostname
	rm -f $(DESTDIR)$(UNITDIR)/vstimd.service
	rm -f $(DESTDIR)$(UNITDIR)/vstimd.target
	rm -f $(DESTDIR)$(UNITDIR)/vstimd-hostname.service
	rm -f $(DESTDIR)$(SYSUSERSDIR)/vstimd.conf
	rm -f $(DESTDIR)$(SHAREDIR)/vstimd.service.avahi.tmpl
	rm -f $(DESTDIR)$(SHAREDIR)/vstimd-shares.conf
	rm -f /etc/avahi/services/vstimd.service
	for f in $(EXAMPLES); do rm -f $(DESTDIR)$(SHAREDIR)/$$(basename $$f); done
	rmdir --ignore-fail-on-non-empty $(DESTDIR)$(SHAREDIR) $(DESTDIR)$(CONFDIR) 2>/dev/null || true
	systemctl daemon-reload 2>/dev/null || true

setup-user:
	systemd-sysusers $(abspath $(SYSUSERS))

# ── Documentation (MkDocs + Material, via uv; see docs/pyproject.toml) ───────

# Live preview at http://127.0.0.1:8000 with auto-reload.
docs:
	uv run --project docs mkdocs serve

# Static site build to site/ (matches the Read the Docs build).
docs-build:
	uv run --project docs mkdocs build --strict

# ── Package targets (Docker-based, output to $(DIST_DIR)/) ───────────────────

deb-amd64:
	DOCKER_BUILDKIT=1 docker build \
	  -f packaging/docker/Dockerfile.deb-builder \
	  --build-arg REVISION=$(REVISION) \
	  --build-arg VSTIMD_VERSION=$(VERSION) \
	  -t $(DEB_BUILDER_IMAGE)-amd64 .
	mkdir -p $(DIST_DIR)
	docker run --rm -v $(abspath $(DIST_DIR)):/output $(DEB_BUILDER_IMAGE)-amd64

deb-arm64:
	DOCKER_BUILDKIT=1 docker build \
	  -f packaging/docker/Dockerfile.deb-builder \
	  --build-arg RUST_TARGET=aarch64-unknown-linux-gnu \
	  --build-arg DEB_HOST_ARCH=arm64 \
	  --build-arg REVISION=$(REVISION) \
	  --build-arg VSTIMD_VERSION=$(VERSION) \
	  -t $(DEB_BUILDER_IMAGE)-arm64 .
	mkdir -p $(DIST_DIR)
	docker run --rm -v $(abspath $(DIST_DIR)):/output $(DEB_BUILDER_IMAGE)-arm64

deb: deb-amd64 deb-arm64

# ── Targets run *inside* the packaging containers ────────────────────────────
# packaging/docker/Dockerfile.{deb,rpm}-builder call these instead of restating
# the build commands, so there is one definition of how vstimd is built. They
# pass RUST_TARGET (and REVISION) on the command line, which make forwards to
# the client/web sub-make automatically.
#
# Not for direct use on a dev box: they assume the container's toolchain and,
# unlike deb-amd64/deb-arm64 above, do no Docker work themselves.

# Both .debs, left in target/<triple>/debian/ for the caller to collect.
#
# --deb-version (rather than --deb-revision) because the manifests' 0.0.0 is a
# sentinel: cargo-deb would otherwise happily emit braemons-vstimd_0.0.0-1.deb.
#
# Sweep out .debs from earlier builds first. In the packaging containers the
# cargo target dir is a persistent cache mount, so a previous build's packages
# survive there — and the caller collects with `find target -name '*.deb'`.
# That was harmless while the version was a constant in Cargo.toml (each build
# overwrote the same filename); now that every commit produces a new version,
# stale packages would accumulate and ride along into a release.
deb-assemble: build
	rm -f target/debian/*.deb target/*/debian/*.deb
	cargo deb -p vstimd        --no-build $(CARGO_TARGET_ARG) --deb-version $(VERSION)-$(REVISION)
	cargo deb -p gpiochip-daqd --no-build $(CARGO_TARGET_ARG) --deb-version $(VERSION)-$(REVISION)

# Single source of truth for these two, so container scripts do not re-derive
# them. The rpm builder used to re-implement the version parse by grepping
# Cargo.toml — which now holds a 0.0.0 sentinel, so that shortcut would produce
# a package claiming to be version 0.0.0.
print-version:
	@echo $(VERSION)

print-binary:
	@echo $(BINARY)

rpm-amd64:
	DOCKER_BUILDKIT=1 docker build \
	  -f packaging/docker/Dockerfile.rpm-builder \
	  --build-arg VSTIMD_VERSION=$(VERSION) \
	  -t $(RPM_BUILDER_IMAGE)-amd64 .
	mkdir -p $(DIST_DIR)
	docker run --rm -v $(abspath $(DIST_DIR)):/output $(RPM_BUILDER_IMAGE)-amd64

rpm-arm64:
	DOCKER_BUILDKIT=1 docker build \
	  -f packaging/docker/Dockerfile.rpm-builder \
	  --build-arg RUST_TARGET=aarch64-unknown-linux-gnu \
	  --build-arg RPM_ARCH=aarch64 \
	  --build-arg VSTIMD_VERSION=$(VERSION) \
	  -t $(RPM_BUILDER_IMAGE)-arm64 .
	mkdir -p $(DIST_DIR)
	docker run --rm -v $(abspath $(DIST_DIR)):/output $(RPM_BUILDER_IMAGE)-arm64

rpm: rpm-amd64 rpm-arm64

packages: deb rpm

# ── SD card image (Raspberry Pi) ──────────────────────────────────────────
#
# Ready-to-flash Raspberry Pi OS Lite (arm64) image with vstimd and
# gpiochip-daqd preinstalled, sshd + an admin user, and a Samba share for
# /etc/braemons. Needs --privileged (loop devices, chroot) — see
# packaging/image/build-sd-image.sh and Dockerfile.image-builder.
image: deb-arm64
	DOCKER_BUILDKIT=1 docker build \
	  -f packaging/docker/Dockerfile.image-builder \
	  -t $(IMAGE_BUILDER_IMAGE) .
	mkdir -p $(DIST_DIR) $(IMAGE_CACHE_DIR)
	docker run --rm --privileged \
	  -v $(abspath $(DIST_DIR)):/src/$(DIST_DIR) \
	  -v $(abspath $(IMAGE_CACHE_DIR)):/src/$(IMAGE_CACHE_DIR) \
	  -e VSTIMD_IMAGE_USER=$(VSTIMD_IMAGE_USER) \
	  -e IMAGE_VERSION=$(IMAGE_VERSION) \
	  -e VSTIMD_IMAGE_PASSWORD=$(VSTIMD_IMAGE_PASSWORD) \
	  -e DIST_DIR=$(DIST_DIR) \
	  -e CACHE_DIR=$(IMAGE_CACHE_DIR) \
	  $(IMAGE_BUILDER_IMAGE) $(DEB_ARM64) $(GPIOCHIP_DEB_ARM64)
