# Bare-metal Linux rendering

Run vstimd without a compositor on Linux, using KMS/DRM for display ownership and raw
Vulkan for rendering — no X11, no Wayland, no display server. This page is the
operator's setup guide for a rig. For the internal backend design (crates, module
layout, vblank sources) see [Rendering & DRM internals](../developer/rendering.md).

---

Every platform uses **`VK_KHR_display`**: the DRM backend releases DRM master and
lets Vulkan acquire the display itself. `VK_EXT_acquire_drm_display` is not used —
on the Jetsons it cannot work (the Vulkan device and the display node are
different hardware, see below), and there is no code path for it elsewhere.

For which platforms are supported at all, see
[Deployment → Supported platforms](deployment.md#supported-platforms).

---

## Tested hardware

| GPU / SoC | CPU | OS | Display backend(s) | Date tested |
|---|---|---|---|---|
| NVIDIA GeForce GTX 1650 | AMD Ryzen 7 5700G | Ubuntu 24.04 | DRM/KMS, X11 | 2026-08-04 |
| NVIDIA Jetson Orin Nano | Orin (Cortex-A78AE) | JetPack 7 (Ubuntu 24.04) | DRM/KMS | 2026-08-04 |
| Broadcom VideoCore VII (Raspberry Pi 5 Model B Rev 1.1, 8GB) | Cortex-A76 | Raspberry Pi OS (Debian 13 trixie) | DRM/KMS | 2026-08-04 |

Tested displays:

| Display | Resolution @ refresh |
|---|---|
| ASUS MB168B Portable USB | 1366×768 @ 60 Hz |
| LG 27GR95QE-B | 2560×1440 @ 100 Hz |

---

## NVIDIA Jetson Orin Nano (L4T R36.x / JetPack 6.x)

### Hardware architecture

The Orin Nano has a **split DRM node architecture** — GPU and display controller are
separate hardware blocks:

| DRM node | Hardware | Role |
|---|---|---|
| `card0` / `renderD128` | nvgpu (`13e00000.host1x`) | GPU — Vulkan rendering |
| `card1` / `renderD129` | nvdisplay (`13800000.display`) | Display controller — scanout, KMS connectors |

`VK_EXT_acquire_drm_display` does **not** work here because that extension requires
the Vulkan physical device and the DRM display node to be the same hardware node —
they are not. Use `VK_KHR_display`, where the Vulkan driver drives the display
controller directly without a DRM fd.

### One-time kernel configuration

`nvidia-drm` must load with `modeset=1` so the display controller registers as
`card1`. Without it, `card1` does not exist and
`vkGetPhysicalDeviceDisplayPropertiesKHR` returns `VK_ERROR_UNKNOWN`.

```bash
echo 'options nvidia-drm modeset=1 fbdev=1' | sudo tee /etc/modprobe.d/nvidia-drm.conf
sudo reboot
```

`fbdev=1` additionally creates `/dev/fb0`, enabling a framebuffer console on the
physical display when no Vulkan app is running.

### Running without a display manager

GDM (or any compositor) must not be running — it holds the display and
`VK_KHR_display` will fail with `VK_ERROR_UNKNOWN`. Stopping GDM alone is not enough;
logind also holds the seat's DRM master reference and must be released:

```bash
sudo systemctl stop gdm
sudo loginctl terminate-seat seat0
```

Then run the server:

```bash
cd ~/src/vstimd
cargo run --release
```

Restore the desktop afterwards with `sudo systemctl start gdm`.

### Persistent headless setup (no desktop)

If the machine is dedicated to vstimd:

```bash
sudo systemctl disable gdm
sudo systemctl set-default multi-user.target
```

The physical display shows a framebuffer console at boot (needs `fbdev=1` above), and
the app takes over the display when launched. For a full boot-into-vstimd service,
see [Deployment](deployment.md).

### Known driver issue: VRR causes GPU device loss (JetPack 6.x)

**Symptom:** vstimd renders 3–4 frames then crashes with `ERROR_DEVICE_LOST`. The
kernel log shows:

```
nvidia-modeset: ERROR: GPU:0: nvRmApiAlloc(memory) failed for vrr 0x22
nvidia-modeset: ERROR: GPU:0: Failed to setup Rgline active session for vrr
nvgpu: ga10b_pbdma_handle_intr_0_acquire: semaphore acquire timeout!
```

**Root cause:** When `VK_KHR_display` acquires a VRR-capable (G-Sync / Adaptive Sync)
display, `nvidia-modeset` tries to allocate a VRR "Rgline active session". On
JetPack 6.x (driver 540.x) this fails with error `0x22` and corrupts the GPU
presentation semaphore pathway; the PBDMA engine then times out, causing
`ERROR_DEVICE_LOST`.

**Fix:** Disable HDMI FRL (Fixed Rate Link), the HDMI 2.1 transport that enables VRR.
Disabling it forces HDMI 2.0 TMDS — ample bandwidth for 4K@60Hz, but no VRR exposed
to `nvidia-modeset`.

```bash
# Apply immediately (no reboot):
sudo sh -c 'echo 1 > /sys/module/nvidia_modeset/parameters/disable_hdmi_frl'

# Make it permanent:
echo 'options nvidia-modeset disable_hdmi_frl=1' | sudo tee /etc/modprobe.d/nvidia-modeset-nofrl.conf
```

**Remaining benign log noise after the fix:**

| Message | Source | Meaning |
|---|---|---|
| `Failed to set variable refresh rate with invalid minimum frame time` | `nvidia-modeset` | VRR range inconsistent; driver skips VRR setup cleanly. Harmless. |
| `nvdisplayr: secure read … EMEM address decode error` | `tegra-mc` | Display controller briefly reads a stale framebuffer address during CRTC restore on exit. Harmless. |

---

## Raspberry Pi 5

Runs on the standard `vc4`/`v3d` KMS drivers via the Mesa v3dv ICD. Unlike the
Jetsons, the GPU and the display controller are the same DRM node here, so setup
is limited to the full-KMS overlay (`dtoverlay=vc4-kms-v3d`, already the Raspberry
Pi OS default) and disabling the display manager — see
[Deployment → Raspberry Pi 4 / 5](deployment.md#raspberry-pi-4-5).

For a rig rather than a development board, skip all of it and flash the
[Raspberry Pi 5 appliance image](raspberry-pi-image.md), which arrives configured.
See [Tested hardware](#tested-hardware) for the validated configuration.

---

## Permissions

Running vstimd **as your own login user** (development) needs group membership for
the devices it opens:

```bash
sudo usermod -aG input,video,render $USER
# log out and back in for group changes to take effect
```

The packaged service does not use this path — see
[Deployment → Service account and device access](deployment.md#2-service-account-and-device-access)
for what the unit actually runs as and which groups cover what.

---

## Keyboard controls

While the server is running:

| Key | Action |
|---|---|
| **D** | Spawn demo stimuli |
| **F1–F7** | Show an overlay panel (Stimuli / Log / Virtual Trigger / Animations / System / Config / Benchmarks) |
| **Shift+F1–F7** | Hide that overlay panel |
| **Esc** | Clean exit |
| **Alt+Enter** | Toggle fullscreen (desktop mode only) |
| **Ctrl+Alt+F1–F12** | Switch VT (forwarded to the kernel, so you can return to a desktop) |

---

## Running

```bash
# Auto-detected mode:
cargo run --release
#   Linux without DISPLAY/WAYLAND_DISPLAY → DRM mode
#   Linux with a display server           → desktop mode
#   Windows                               → desktop mode

# Force headless (ZMQ only, no display) for protocol testing:
cargo run --release -- --null

# Then drive it from a client:
cd client/python && uv run examples/flash_rects.py
```
