# Manual appliance setup

A step-by-step walkthrough for turning a bare Linux install into a dedicated,
headless vstimd rig — on any board the [supported platforms](deployment.md#supported-platforms)
table covers, not just Raspberry Pi. This is the manual, per-device equivalent
of what `packaging/image/build-sd-image.sh` automates for Raspberry Pi 5 SD
images: same end state, done by hand over SSH instead of baked into a
downloadable `.img`.

Use this guide when:

- provisioning a Jetson (no automated image build exists yet — see
  [Approaching a Jetson image](#jetson-note) below), or
- setting up a one-off rig on hardware the automated builder doesn't cover
  (a desktop NVIDIA box, a different Pi model), or
- you want to understand what the SD-image build actually does, step by step.

---

## Checklist

1. [Base OS](#1-base-os)
2. [Display backend](#2-display-backend)
3. [Install vstimd + gpiochip-daqd](#3-install-vstimd--gpiochip-daqd)
4. [Configure gpiochip-daqd](#4-configure-gpiochip-daqd)
5. [Network identity](#5-network-identity)
6. [Admin access (SSH + optional Samba)](#6-admin-access-ssh--optional-samba)
7. [Boot straight into vstimd](#7-boot-straight-into-vstimd)
8. [Verify](#8-verify)
9. [Optional: clone into a golden image](#9-optional-clone-into-a-golden-image)

---

## 1. Base OS

| Platform | How to get a base OS on the device |
|---|---|
| Raspberry Pi | [Raspberry Pi Imager](https://www.raspberrypi.com/software/) — under the gear icon, enable SSH and set a password before writing, so the card boots straight into a reachable system. |
| Jetson Orin / Nano | NVIDIA SDK Manager flashes JetPack/L4T over USB with the board in recovery mode. This step is host-tethered and can't be scripted into a downloadable image the way the Pi one can — see [Jetson note](#jetson-note). |
| Desktop x86 | A normal Ubuntu Server/Desktop install (or minimal netinstall) is fine. |

Once you have SSH access, update the base system:

```bash
sudo apt update && sudo apt full-upgrade
```

## 2. Display backend

Follow the platform-specific setup in [Bare-metal Linux](bare-metal.md) and
[Deployment → Platform-specific notes](deployment.md#platform-specific-notes)
first — this is the part that's genuinely different per board (KMS overlays,
`nvidia-drm.modeset=1`, split DRM nodes on Jetson, etc.) and is already
documented there in full; don't duplicate it here.

Then disable the display manager so nothing else contends for the display
(see [Deployment → Display manager](deployment.md#1-display-manager)):

```bash
sudo systemctl disable --now gdm     # Ubuntu / L4T
sudo systemctl disable --now lightdm # Raspberry Pi OS
```

## 3. Install vstimd + gpiochip-daqd

Preferred: point at the braemons apt archive so the rig can upgrade in place
later (see the [archive README](https://github.com/braemons/packages#using-it)
for adding the source and key). Then:

```bash
sudo apt install braemons-vstimd braemons-gpiochip-daqd
```

Or, without the archive, install locally-built `.deb`s directly:

```bash
sudo dpkg -i braemons-vstimd_<version>_arm64.deb braemons-gpiochip-daqd_<version>_arm64.deb
sudo apt install -y -f   # pull in any missing dependencies
```

Either way, `postinst` creates the `vstimd` system user in the `input`,
`video`, and `render` groups automatically (see
[Deployment → Groups](deployment.md#2-groups)) — no manual `usermod` needed
for a packaged install.

## 4. Configure gpiochip-daqd

The package ships an empty `default-config.toml` at
`/etc/braemons/gpiochip-daqd-config.toml` (the live path the daemon reads) and
installs board-specific examples read-only under
`/usr/share/braemons/gpiochip-daqd/`. Copy the one matching your board over
the live path:

```bash
ls /usr/share/braemons/gpiochip-daqd/
sudo install -m 0644 \
    /usr/share/braemons/gpiochip-daqd/<board>.toml \
    /etc/braemons/gpiochip-daqd-config.toml
```

Before trusting the `chip =` line in that file, confirm the GPIO chip name on
*your* kernel — it can differ by kernel version even on the same board model
(RP1 on a Pi 5 enumerates as `gpiochip0` on some kernels, `gpiochip4` on
others):

```bash
gpiodetect
gpioinfo /dev/gpiochip0   # or whichever gpiodetect reported
```

Edit `gpio.chip` in the config if it doesn't match, then start the daemon:

```bash
sudo systemctl enable --now gpiochip-daqd
systemctl status gpiochip-daqd
```

See the [gpiochip-daqd README](https://github.com/braemons/vstimd/blob/main/gpiochip-daqd/README.md)
for the full config format if you need a custom pin mapping instead of one of
the shipped examples.

## 5. Network identity

Enable the hostname service so the rig gets a stable, collision-free name
derived from its MAC address, and both Avahi and Samba pick it up
automatically (see
[Deployment → Network discovery](deployment.md#network-discovery) for how
this works):

```bash
sudo systemctl enable --now vstimd-hostname
```

## 6. Admin access (SSH + optional Samba)

Use a **separate login user for SSH/admin access** — not the `vstimd` system
account the service runs as (that account has no login shell). The automated
Pi image build calls this account `vstimd-admin`; match that or pick your own
name, just don't reuse `vstimd`.

```bash
sudo useradd -m -s /bin/bash -G sudo vstimd-admin
sudo passwd vstimd-admin
sudo chage -d 0 vstimd-admin   # force a password change at first login
```

**Optional — Samba shares** for editing `/etc/braemons` (rig config,
`gpiochip-daqd-config.toml`) and reading `/var/lib/braemons` (saved stim
configs) from a lab Windows/macOS machine without SSHing in each time:

```bash
sudo apt install -y samba
sudo smbpasswd -a vstimd-admin
```

Append to `/etc/samba/smb.conf`:

```ini
[global]
   unix password sync = yes

[vstimd-config]
   path = /etc/braemons
   browseable = yes
   read only = no
   guest ok = no
   valid users = vstimd-admin
   create mask = 0644
   directory mask = 0755

[vstimd-data]
   path = /var/lib/braemons
   browseable = yes
   read only = no
   guest ok = no
   valid users = vstimd-admin
   create mask = 0644
   directory mask = 0755
   force user = root
```

```bash
sudo testparm -s
sudo systemctl enable --now smbd nmbd
```

`/var/lib/braemons/vstimd` is created by systemd's `StateDirectory=` the first
time `vstimd.service` runs, and Samba won't export a path that doesn't exist
yet — create it up front if you're setting this up before first boot:

```bash
sudo install -d -m 0755 /var/lib/braemons
```

Only put Samba on a network you trust — `smbpasswd` credentials are separate
from the login password and don't get the forced-rotation treatment
`chage -d 0` gives SSH.

## 7. Boot straight into vstimd

Follow [Deployment → Boot to vstimd](deployment.md#boot-to-vstimd) to enable
the unit and add a boot-loader entry (GRUB on x86, extlinux on Jetson/Pi):

```bash
sudo systemctl enable vstimd
```

## 8. Verify

```bash
systemctl status vstimd gpiochip-daqd vstimd-hostname
```

Browse to `http://<device-ip>:8080` from another machine on the network — you
should see the vstimd web UI (or the placeholder page, if built without it).
If GPIO is wired, exercise a line through the API or, for a hardware
loopback, follow the [gpiochip-daqd README's test instructions](https://github.com/braemons/vstimd/blob/main/gpiochip-daqd/README.md#hardware-loopback-tests).

Reboot once and confirm everything comes back up on its own before calling
the rig done.

## 9. Optional: clone into a golden image

Once one rig is fully set up, `dd`-ing its SD card/disk to reuse as a starting
point for identical boards is a reasonable shortcut for boards without an
automated image build (e.g. Jetson today, per the [note below](#jetson-note)).
Two things a raw clone does *not* handle that the automated Pi build gets for
free from `raspberrypi-sys-mods` regenerating them on first boot:

- **SSH host keys** — every board cloned from the same image shares host
  keys, so the first SSH connection to each new board can trigger a "remote
  host identification has changed" warning on clients that talked to a
  *different* board with the same key before. Regenerate per board after
  cloning:

  ```bash
  sudo rm /etc/ssh/ssh_host_*
  sudo dpkg-reconfigure openssh-server
  ```

- **`/etc/machine-id`** — shared across clones otherwise, which confuses
  anything keying off it (systemd-journald's persistent log linking, DHCP
  client IDs on some stacks). Regenerate:

  ```bash
  sudo rm /etc/machine-id
  sudo systemd-machine-id-setup
  ```

If every board stays on a closed, trusted lab network and nothing relies on
host-key or machine-id uniqueness, skipping this is a defensible tradeoff —
just know it's a tradeoff, not a non-issue.

---

## <a name="jetson-note"></a>Jetson note

There's no `build-sd-image.sh` equivalent for Jetson yet, because NVIDIA's
flashing tools (SDK Manager / `l4t_initrd_flash`) are host-tethered — they
push JetPack to a board sitting in USB recovery mode rather than writing a
generic downloadable `.img` you can loop-mount and modify offline the way the
Pi build does. Until that gap is closed, the supported path is: flash one
board with SDK Manager, walk it through this guide once, then either repeat
this guide per board or clone that board's disk per
[§9](#9-optional-clone-into-a-golden-image).

---

## Updating a deployed rig

See [Deployment → Updating a deployed rig](deployment.md#updating-a-deployed-rig) —
`apt update && apt upgrade` in place, never re-flash (that wipes
`/etc/braemons` and `/var/lib/braemons`).
