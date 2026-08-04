# Deployment

vstimd is designed to run as a systemd service on bare-metal Linux, driving the
display directly via `VK_KHR_display` without a compositor. This page covers
installing and running it on a rig. To *build* the binaries and packages referenced
here, see [Building & packaging](../developer/building.md).

## Supported platforms

| Platform | OS | Notes |
|---|---|---|
| Jetson Orin (Tegra) | Ubuntu (L4T) | Primary target; GPU and display controller are separate DRM nodes |
| Raspberry Pi 4 / 5 | Raspberry Pi OS | Full KMS overlay required; see below |
| x86 / desktop NVIDIA | Ubuntu | Extra kernel parameter required; see below |

See [Bare-metal Linux](bare-metal.md) for the per-board display setup.

---

## Install

### From a package

Install a `.deb` (or `.rpm`) built as described in
[Building & packaging](../developer/building.md), then enable the service:

```bash
sudo dpkg -i braemons-vstimd_<version>-1_arm64.deb
sudo systemctl enable --now vstimd
```

`postinst` calls `systemd-sysusers` to create the `vstimd` system user (in the
`input`, `video`, and `render` groups) and warns if a display manager is enabled on
the same VT.

### From source

The `Makefile` install target is the same one the packages use:

```bash
# Build as your user (embeds the browser UI — needs Node/npm + cargo).
# Run WITHOUT sudo: root has no cargo/rustup in PATH.
make build

# Install files and provision the vstimd system user:
sudo make install              # → /usr/bin/vstimd, unit + sysusers files
sudo make setup-user           # runs systemd-sysusers

sudo systemctl daemon-reload
sudo systemctl enable --now vstimd
```

Use `make build-server` for a UI-less binary that needs no Node. See
[Building & packaging](../developer/building.md) for all targets and the installed
file layout.

### Web control surface

The server runs an HTTP + WebSocket control surface, enabled by default on
`0.0.0.0:8080`. Browse to `http://<device-ip>:8080` from any machine on the network
to control it — no client install needed.

- The full React UI is served **only when the binary was built with the embedded UI**
  (`make build`, or the `.deb`/`.rpm` packages). A plain `cargo build --release` /
  `make build-server` binary still runs and serves the WebSocket API, but shows a
  placeholder page at `/`.
- Configure via the rig-config (`[web] enabled`, `[web] port`) or CLI flags
  (`--no-web`, `--web-port <N>`).
- If the device runs a firewall, open the port: e.g. `sudo ufw allow 8080/tcp`.

### Network discovery

Multiple rigs on the same LAN need distinct, collision-free names. Packages install
`vstimd-hostname.service`, which runs `vstimd-set-hostname` at boot to derive a stable
hostname from the primary interface's MAC address: `vstimd-XXXXXX`, where `XXXXXX` is
the MAC's last 6 hex characters (13 chars total, within Samba's 15-char NetBIOS
limit). The unit orders itself before `avahi-daemon`, `smbd`, and `nmbd`, so both
inherit the generated hostname automatically — neither needs a `host-name=` override
in `avahi-daemon.conf` nor a `netbios name` override in `smb.conf`. The script also
keeps the `127.0.1.1` line in `/etc/hosts` in sync (`hostnamectl`/`hostname -F` never
touch it, so without this, local hostname lookups — including the ones `sudo` does —
break right after the rename) and, belt-and-braces for a rename on an already-running
system, `try-restart`s `avahi-daemon`/`smbd`/`nmbd` if they're active.

If Avahi is installed, the package also ships an mDNS service template
(`/usr/share/braemons/vstimd/vstimd.service.avahi.tmpl`) that `vstimd-set-hostname`
renders to `/etc/avahi/services/vstimd.service` at boot, advertising `_vstimd._tcp` on
port 5555 — discoverable with `avahi-browse -r _vstimd._tcp`. The rendered file's
`id=vstimd-XXXXXX` TXT record is a literal written by the script, not an Avahi `%h`
substitution — Avahi's `avahi-service.dtd` only allows `replace-wildcards` on `<name>`,
not `<txt-record>`, so `%h` can't be used inside a TXT value. The TXT record carries
the raw hostname even if Avahi appends `#2`, `#3`, etc. to the advertised `<name>` on
collision — key off the TXT record, not the display name, when matching a specific
device programmatically.

From source (`make install`), enable the unit alongside `vstimd`:

```bash
sudo systemctl enable --now vstimd-hostname
```

---

## Common setup (all platforms)

### 1. Display manager

vstimd acquires the display via `VK_KHR_display`, which requires DRM master on the VT
it uses (`TTYPath=/dev/tty1` in the unit file).

**Dedicated / headless hardware (recommended):** disable the display manager so
nothing contends for the display.

```bash
# Ubuntu / L4T
sudo systemctl disable --now gdm

# Raspberry Pi OS
sudo systemctl disable --now lightdm
# or via raspi-config → System Options → Boot → Console (no desktop)
```

**Desktop / development machine:** VT switching allows coexistence. vstimd runs on
VT3 by default (`TTYPath=/dev/tty3`). Ctrl+Alt+F1–F12 is intercepted and forwarded so
you can switch back to your desktop; the input grab is released while vstimd is in the
background. The unit file strips `DISPLAY`, `WAYLAND_DISPLAY`, and `XDG_RUNTIME_DIR`
(`UnsetEnvironment`) so Vulkan does not fall back to WSI.

### 2. Groups

The `vstimd` user needs:

| Group | Device | Notes |
|---|---|---|
| `input` | `/dev/input/event*` | libinput keyboard/mouse |
| `video` | `/dev/dri/card*` | DRM master / Vulkan |
| `render` | `/dev/dri/renderD*` | GPU nodes on Raspberry Pi OS |

These are added automatically by `make setup-user` / package post-install. For an
existing login user running vstimd directly (development only):

```bash
sudo usermod -aG input,video,render $USER
# log out and back in for group changes to take effect
```

---

## Platform-specific notes

### Jetson Orin (Tegra / L4T)

The Orin has a split DRM architecture (`card0` = nvgpu GPU, `card1` = display
controller). Because the Vulkan device and display controller are different hardware
nodes, `VK_EXT_acquire_drm_display` does not work but `VK_KHR_display` does. No
special kernel parameters are required — the display controller driver loads from the
device tree at boot. See [Bare-metal Linux](bare-metal.md) for the full Orin setup.

### Raspberry Pi 4 / 5

The Pi display stack requires **full KMS** (not fake-KMS). Add or confirm this in
`/boot/firmware/config.txt` (Pi OS Bookworm) or `/boot/config.txt` (older):

```
dtoverlay=vc4-kms-v3d
```

The `vc4-fkms-v3d` overlay (fake KMS) is **not** sufficient. After changing it, reboot
and verify:

```bash
ls /dev/dri/
cat /sys/class/drm/card*/status
```

### Desktop / workstation NVIDIA (proprietary driver)

The `nvidia-drm` module must have KMS enabled. Add to the kernel command line:

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

Verify after reboot: `cat /sys/module/nvidia_drm/parameters/modeset` should print
`Y`. Without `modeset=1`, `VK_KHR_display` finds no displays and fails at startup.

---

## Boot to vstimd

vstimd ships a custom `vstimd.target` between `multi-user.target` and
`graphical.target`. Booting into it starts vstimd (plus networking, logging, etc.)
without a display manager. A normal boot into `graphical.target` leaves vstimd alone.

**Setup (once):**

```bash
# Enable vstimd to start when vstimd.target is reached.
# This does NOT make it start on normal graphical boots.
sudo systemctl enable vstimd
```

### GRUB (x86 — Fedora, Ubuntu, Debian)

**Fedora — use `grubby`** (easiest, version-independent):

```bash
sudo grubby --copy-default \
  --add-kernel=$(grubby --default-kernel) \
  --title="Boot to vstimd" \
  --args="systemd.unit=vstimd.target"

sudo grubby --info=ALL | grep -A4 "vstimd"
```

**Ubuntu / Debian — add a custom entry** to `/etc/grub.d/40_custom`, copying the
`linux`/`initrd` lines from your default entry and appending
`systemd.unit=vstimd.target` to the `linux` line:

```
menuentry "Boot to vstimd" {
    load_video
    set gfxpayload=keep
    linux   /boot/vmlinuz-6.8.0-51-generic root=UUID=<your-root-uuid> ro quiet systemd.unit=vstimd.target
    initrd  /boot/initrd.img-6.8.0-51-generic
}
```

Then `sudo update-grub` (Debian/Ubuntu) or `sudo grub2-mkconfig -o …` (Fedora).

### extlinux (Jetson, Raspberry Pi, embedded)

Edit `/boot/extlinux/extlinux.conf`. Copy your primary entry and add
`systemd.unit=vstimd.target` to the `APPEND` line:

```
LABEL vstimd
    MENU LABEL Boot to vstimd
    LINUX /boot/Image
    INITRD /boot/initrd
    APPEND ${cbootargs} quiet root=PARTUUID=<your-partuuid> rw systemd.unit=vstimd.target
```

No rebuild step is needed — extlinux reads the file directly at boot. On Jetson the
file is typically `/boot/extlinux/extlinux.conf`; on Raspberry Pi it may be under
`/boot/firmware/extlinux/`.

### Switching back to the desktop

Select your normal boot entry from the GRUB/extlinux menu. Or, without rebooting:

```bash
sudo systemctl stop vstimd
sudo systemctl isolate graphical.target
```

> **Note:** Runtime VT switching (Ctrl+Alt+Fn while vstimd is running) currently works
> only when vstimd was started from a TTY session, not from an X11/Wayland desktop.
> When using the "Boot to vstimd" entry, switching works correctly.
