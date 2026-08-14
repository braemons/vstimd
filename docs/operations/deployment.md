# Deployment

vstimd is designed to run as a systemd service on bare-metal Linux, driving the
display directly via `VK_KHR_display` without a compositor. This page covers
installing and running it on a rig. To *build* the binaries and packages referenced
here, see [Building & packaging](../developer/building.md).

## Supported platforms

| Platform | OS | Notes |
|---|---|---|
| Jetson Orin (Tegra) | Ubuntu (L4T) | Primary target; GPU and display controller are separate DRM nodes |
| Raspberry Pi 5 | Raspberry Pi OS | A [ready-to-flash image](raspberry-pi-image.md) is published with every release |
| Raspberry Pi 4 | Raspberry Pi OS | Full KMS overlay required; see below |
| x86 / desktop NVIDIA | Ubuntu | Extra kernel parameter required; see below |

See [Bare-metal Linux](bare-metal.md) for the per-board display setup.

---

## Install

### From a package

The published `.deb`/`.rpm`, the braemons apt archive, and the Raspberry Pi 5
image are all covered in [Installation](../getting-started/installation.md) —
that page is the single place install commands live. In short:

```bash
sudo apt install braemons-vstimd        # from the archive
sudo systemctl enable --now vstimd
```

On install, `postinst` (or the rpm `%post`) calls `systemd-sysusers` to create the
`vstimd` system user, creates `/etc/braemons` and `/var/log/vstimd`, and registers
a ["Boot to vstimd" bootloader entry](#boot-to-vstimd).

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

Packages install `vstimd-hostname.service`, which names the rig `vstimd-XXXXXX`
after its MAC address at boot and renders the Avahi service file that advertises
`_vstimd._tcp` on port 5555. Both Samba and Avahi inherit that name. The policy,
the mDNS TXT record clients should match on, and how to opt out are documented in
**[Discovery & hostnames](discovery.md)**.

From source (`make install`), enable the unit alongside `vstimd`:

```bash
sudo systemctl enable --now vstimd-hostname
```

### Logs

vstimd logs to the journal (`journalctl -u vstimd -f`). The packages additionally
drop in an rsyslog rule and a logrotate policy, so on a rig with rsyslog installed
the same messages land in a dedicated file:

| Path | |
|---|---|
| `/var/log/vstimd/vstimd.log` | Everything from `programname == 'vstimd'` |
| `/etc/rsyslog.d/10-vstimd.conf` | The routing rule |
| `/etc/logrotate.d/vstimd` | Daily, 14 rotations, compressed |

Raise the level with a drop-in: `systemctl edit vstimd` and set
`Environment=RUST_LOG=debug`.

---

## Updating a deployed rig

**Upgrade in place — do not re-flash.** Re-flashing wipes `/etc/braemons` (the
rig config and every saved stimulus config) and `/var/lib/braemons`.

```bash
sudo apt update && sudo apt upgrade
```

Images ship pointing at the vstimd archive, so this works out of the box. The
rig config `/etc/braemons/vstimd-rig-config.toml` is a dpkg **conffile**: your
local edits survive the upgrade, and if a release changes the shipped default,
the new version is written alongside as `.dpkg-dist` for you to diff. Configs
saved through the API aren't owned by the package at all, so upgrades never
touch them.

Images built from a pre-release track the `testing` suite and keep receiving
pre-releases; images from a plain release track `stable` and are never upgraded
onto an alpha. To move a rig between them, edit `Suites:` in
`/etc/apt/sources.list.d/braemons.sources`.

To check what a rig is running and what it would move to:

```bash
vstimd --version
apt policy braemons-vstimd
```

A rig that predates the archive, or one that was installed from a downloaded
`.deb`, needs the source added once —
see [Installation → apt archive](../getting-started/installation.md#apt-archive-debian-ubuntu).

Reserve re-flashing for provisioning a new rig or recovering a failed card.

---

## Common setup (all platforms)

### 1. Display manager

vstimd acquires the display via `VK_KHR_display`, which requires DRM master on the VT
it uses — `TTYPath=/dev/tty3` in the unit file. The unit also declares
`Conflicts=getty@tty3.service`, so logind's own getty on that VT is stopped first
rather than holding the terminal open while vstimd blocks trying to acquire it.

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

**Power button:** vstimd grabs input devices exclusively (`EVIOCGRAB`) so keystrokes
cannot leak to the console behind the stimulus. Power and sleep switches are excluded
from that grab — on the Raspberry Pi 5 the power button is an ordinary `gpio-keys`
input device, and grabbing it would swallow `KEY_POWER` before `systemd-logind` saw
it, leaving no way to shut the machine down while vstimd is running. Left ungrabbed,
the button behaves normally: logind powers off, systemd stops `vstimd.service` with
SIGTERM, and the usual graceful path runs (scene saved, display released, VT
restored). The exclusion is by capability, not device name — a device counts as a
power switch when it advertises a power/sleep key and no ordinary typing keys, so a
keyboard that carries `KEY_POWER` on its main node is still grabbed.

### 2. Service account and device access

`vstimd.service` currently runs as **`User=root`**, with
`SupplementaryGroups=input video tty`. Root is what supplies `CAP_SYS_NICE`, which
the render thread needs to promote itself to `SCHED_FIFO` (`[scheduling]
render_rt_prio`, on by default). Without it vstimd still runs, but frame delivery
loses its priority and the overlay's System panel reports the promotion as FAILED.

`make setup-user` and the package post-install scripts still provision an
unprivileged `vstimd` system user (in `input`, `video`, `render`, `tty`) via
`systemd-sysusers`, ready for the unit to move onto it. If you switch the unit to
that user yourself, add `AmbientCapabilities=CAP_SYS_NICE` in the same drop-in.

The device access those groups cover:

| Group | Device | Notes |
|---|---|---|
| `input` | `/dev/input/event*` | libinput keyboard/mouse |
| `video` | `/dev/dri/card*` | DRM master / Vulkan |
| `render` | `/dev/dri/renderD*` | GPU nodes on Raspberry Pi OS |
| `tty` | `/dev/tty3` | The unit's `TTYPath`, for a non-root user |

For an existing login user running vstimd directly (development only):

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

!!! tip "Pi 5: use the published image"
    The [Raspberry Pi 5 appliance image](raspberry-pi-image.md) has all of this
    done already. This section is for a Pi you are setting up by hand.

The Pi display stack requires **full KMS** (not fake-KMS). Current Raspberry Pi OS
sets this by default; add or confirm it in `/boot/firmware/config.txt` (Pi OS
Bookworm and later) or `/boot/config.txt` (older):

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

### The bootloader entry

The packages register a **"Boot to vstimd"** entry automatically in their
post-install script, so on a packaged rig there is nothing to do. The same script
is installed for you to run by hand after `make install`:

```bash
sudo vstimd-boot-entry            # add the entry
vstimd-boot-entry --dry-run       # show what it would do, no root needed
sudo vstimd-boot-entry --remove   # take it away again
```

It detects `grubby` (Fedora/RHEL), GRUB2 (Ubuntu/Debian/Fedora/Arch), or extlinux
(Jetson, Raspberry Pi), clones the default entry, and appends
`systemd.unit=vstimd.target` to its kernel command line. It is safe to re-run —
an entry that already exists is left alone — and failure is non-fatal at install
time, which is why it is worth checking the output on an unusual bootloader.

The Raspberry Pi image goes further and makes `vstimd.target` the *default* target
outright (`systemctl set-default`), so no menu selection is involved at all.

??? note "Adding the entry by hand"
    Only needed if `vstimd-boot-entry` cannot handle your bootloader.

    **Fedora — `grubby`:**

    ```bash
    sudo grubby --copy-default \
      --add-kernel=$(grubby --default-kernel) \
      --title="Boot to vstimd" \
      --args="systemd.unit=vstimd.target"

    sudo grubby --info=ALL | grep -A4 "vstimd"
    ```

    **Ubuntu / Debian — a custom entry** in `/etc/grub.d/40_custom`, copying the
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

    **extlinux (Jetson, embedded)** — copy your primary entry in
    `/boot/extlinux/extlinux.conf` (or `/boot/firmware/extlinux/extlinux.conf`)
    and add `systemd.unit=vstimd.target` to the `APPEND` line:

    ```
    LABEL vstimd
        MENU LABEL Boot to vstimd
        LINUX /boot/Image
        INITRD /boot/initrd
        APPEND ${cbootargs} quiet root=PARTUUID=<your-partuuid> rw systemd.unit=vstimd.target
    ```

    No rebuild step is needed — extlinux reads the file directly at boot.

### Switching back to the desktop

Select your normal boot entry from the GRUB/extlinux menu. Or, without rebooting:

```bash
sudo systemctl stop vstimd
sudo systemctl isolate graphical.target
```

> **Note:** Runtime VT switching (Ctrl+Alt+Fn while vstimd is running) currently works
> only when vstimd was started from a TTY session, not from an X11/Wayland desktop.
> When using the "Boot to vstimd" entry, switching works correctly.
