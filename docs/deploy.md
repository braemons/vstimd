# Deployment

vstimd is designed to run as a systemd service on bare-metal Linux, driving the
display directly via `VK_KHR_display` without a compositor.

## Supported platforms

| Platform | OS | Notes |
|---|---|---|
| Jetson Orin (Tegra) | Ubuntu (L4T) | Primary target; GPU and display controller are separate DRM nodes |
| Raspberry Pi 4 / 5 | Raspberry Pi OS | Full KMS overlay required; see below |
| x86 / desktop NVIDIA | Ubuntu | Extra kernel parameter required; see below |

---

## Common setup (all platforms)

### 1. Disable the display manager

The display manager must not be running — it holds the display and
`VK_KHR_display` will fail to acquire it.

```bash
# Ubuntu / L4T
sudo systemctl disable --now gdm

# Raspberry Pi OS
sudo systemctl disable --now lightdm
# or via raspi-config → System Options → Boot → Console (no desktop)
```

### 2. Add the service user to the right groups

The process needs access to `/dev/input/event*` (keyboard via libinput) and
`/dev/dri/*` (Vulkan / DRM).

```bash
# Ubuntu / L4T
sudo usermod -aG input,video $USER

# Raspberry Pi OS — 'render' is used instead of 'video' for GPU nodes
sudo usermod -aG input,video,render $USER
```

If running as a dedicated system user (rather than a login user), set
`SupplementaryGroups=input video` in the unit file (already done).

### 3. Install the service unit

```bash
sudo cp packaging/systemd/vstimd.service /usr/lib/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now vstimd
```

---

## Platform-specific notes

### Jetson Orin (Tegra / L4T)

The Orin has a split DRM architecture:

| DRM node | Driver | Role |
|---|---|---|
| `card0` / `renderD128` | `nvgpu` (`13e00000.host1x`) | GPU — Vulkan runs here |
| `card1` / `renderD129` | display controller (`13800000.display`) | KMS/scanout |

Because the Vulkan device and the display controller are different hardware nodes,
`VK_EXT_acquire_drm_display` does not work.  `VK_KHR_display` works: the Vulkan
driver enumerates displays directly without a DRM fd.

No special kernel parameters are required.  The display controller driver loads
from the device tree at boot.

### Raspberry Pi 4 / 5

The Pi display stack requires **full KMS** (not fake-KMS) for `VK_KHR_display`.
Add or confirm this line in `/boot/firmware/config.txt` (Pi OS Bookworm) or
`/boot/config.txt` (older):

```
dtoverlay=vc4-kms-v3d
```

The `vc4-fkms-v3d` overlay (fake KMS) is **not** sufficient.

After changing the overlay, reboot and verify:

```bash
# Should list a card with connected displays
ls /dev/dri/
cat /sys/class/drm/card*/status
```

The Vulkan driver (`v3d`) and display controller (`vc4`) are again separate DRM
nodes on Pi 4/5, similar to Jetson.

### Desktop / workstation NVIDIA (proprietary driver)

The `nvidia-drm` module must have KMS enabled.  Add to the kernel command line:

```
nvidia-drm.modeset=1
```

**Ubuntu with GRUB:**

```bash
# /etc/default/grub
GRUB_CMDLINE_LINUX_DEFAULT="quiet splash nvidia-drm.modeset=1"

sudo update-grub
sudo reboot
```

Verify after reboot:

```bash
cat /sys/module/nvidia_drm/parameters/modeset   # should print Y
```

Without `modeset=1`, `VK_KHR_display` will find no displays and fail at startup.
No other display manager interaction is needed once this parameter is set.

---

## Packaging

Package layout:

| Source path | Installed path |
|---|---|
| `target/release/vstimd` | `/usr/bin/vstimd` |
| `packaging/systemd/vstimd.service` | `/usr/lib/systemd/system/vstimd.service` |

### Build the .deb (Docker — recommended)

```bash
# 1. Build the binary.
cargo build --release

# 2. Build the .deb inside a container (no packaging tools needed on host).
docker build -f packaging/docker/Dockerfile.deb-builder -t vstimd-deb-builder .
docker run --rm -v $(pwd)/packaging:/output vstimd-deb-builder
# packaging/vstimd_0.1.0-1_amd64.deb is ready
```

### Build the .deb (native)

Requires `debhelper` >= 13 and `dpkg-dev` on the host.
`dpkg-buildpackage` expects `debian/` at the repo root, so symlink it first:

```bash
cargo build --release
ln -sf packaging/debian debian
dpkg-buildpackage -b --no-sign
rm debian
# ../vstimd_0.1.0-1_amd64.deb
```

### Cross-compile for arm64 (Jetson / Raspberry Pi)

```bash
sudo apt install gcc-aarch64-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu

# Docker build picks up the cross-compiled binary automatically via debian/rules.
docker build -f packaging/docker/Dockerfile.deb-builder -t vstimd-deb-builder .
docker run --rm -v $(pwd)/packaging:/output vstimd-deb-builder
```

### Install on target

```bash
sudo dpkg -i vstimd_0.1.0-1_arm64.deb
sudo systemctl enable --now vstimd
```

`postinst` will warn if a display manager (GDM, LightDM, etc.) is enabled.
Disable it first — it will hold the DRM master and block vstimd:

```bash
sudo systemctl disable --now gdm   # Ubuntu / L4T
sudo systemctl disable --now lightdm  # Raspberry Pi OS
```

---

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

The test script (`packaging/docker/test-service.sh`) exercises:
1. `dpkg -i` — package installs cleanly
2. `systemctl start vstimd` — `Type=notify` handshake succeeds within 20 s
3. ZMQ port 5555 is reachable
4. `systemctl stop vstimd` — clean SIGTERM shutdown
5. No zombie process after stop
