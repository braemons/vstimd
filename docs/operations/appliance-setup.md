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

!!! tip "Raspberry Pi 5: don't do this by hand"
    Flash the published [appliance image](raspberry-pi-image.md) instead — it is
    the output of exactly these steps, already done.

---

## Checklist

1. [Base OS](#1-base-os)
2. [Display backend](#2-display-backend)
3. [Install vstimd + gpiochip-daqd](#3-install-vstimd-gpiochip-daqd)
4. [Configure gpiochip-daqd](#4-configure-gpiochip-daqd)
5. [Network identity](#5-network-identity)
6. [Admin access (SSH + optional Samba)](#6-admin-access-ssh-optional-samba)
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
later — [Installation → apt archive](../getting-started/installation.md#apt-archive-debian-ubuntu)
has the setup. Then:

```bash
sudo apt install braemons-vstimd braemons-gpiochip-daqd
```

Or, without the archive, install `.deb`s from a
[release](https://github.com/braemons/vstimd/releases) directly:

```bash
# Both packages are published for amd64 and arm64 — pick the one matching
# `dpkg --print-architecture` on the rig.
sudo apt install ./braemons-vstimd_<version>-1_arm64.deb \
                 ./braemons-gpiochip-daqd_<version>-1_arm64.deb
```

Either way, `postinst` creates the `vstimd` system user in the `input`,
`video`, and `render` groups automatically (see
[Deployment → Groups](deployment.md#2-service-account-and-device-access)) — no manual `usermod` needed
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
automatically — see [Discovery & hostnames](discovery.md) for the policy, the
mDNS advertisement, and how to opt out:

```bash
sudo apt install -y avahi-daemon        # for the mDNS advertisement
sudo systemctl enable --now vstimd-hostname
hostname                                # → vstimd-XXXXXX
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
`gpiochip-daqd-config.toml`) and reading `/var/lib/braemons` (the projects
holding saved scene-configs) from a lab Windows/macOS machine without SSHing in
each time. Both
shares are browsable read-only to anyone on the LAN with no credentials;
writing requires an account in `sudo` (or `wheel` on RHEL-family), which
`vstimd-admin` is.

Samba is a **Suggests** of `braemons-vstimd`, so it is not installed with the
package — but the share definitions are, at
`/usr/share/braemons/vstimd/vstimd-shares.conf`. You do not need to write any
`smb.conf` stanzas by hand; point Samba at the shipped file instead:

```bash
sudo apt install -y samba

# Activate the shipped share definitions. Append at the END of the file, so
# the include cannot land inside another share's section:
printf '\n[global]\n   include = /usr/share/braemons/vstimd/vstimd-shares.conf\n' \
    | sudo tee -a /etc/samba/smb.conf

sudo smbpasswd -a vstimd-admin       # seed the Samba credential
sudo testparm -s                     # must parse cleanly
sudo systemctl enable --now smbd nmbd
```

Including rather than copying means a later `apt upgrade` that revises the
stanzas reaches this rig. This is exactly what the Raspberry Pi image build
does, so a hand-built rig and a flashed one export byte-identical shares.

### Keeping the Samba and Unix passwords in sync

Samba keeps its own credential in `passdb.tdb`, which `chage -d 0` does not
govern — that only gates Unix/SSH logins. The shipped file's `unix password
sync` covers one direction (changing the Samba password, or changing it from a
Windows client, updates the Unix one). For the reverse — pushing a Unix
password change, including the forced first-login prompt, into Samba's passdb
— add a `pam_exec` hook.

Ubuntu packages `pam_smbpass` as `libpam-smbpass` for exactly this, but Debian
(and therefore Raspberry Pi OS) does not ship it at all, so
`apt install libpam-smbpass` simply fails there. `pam_exec(8)` is the
Debian-native equivalent: `expose_authtok` hands the new plaintext password to
a script on stdin during the password-change stack, and `seteuid` runs that
script with effective root — needed for `smbpasswd -s`, which is root-only —
even though the real caller is the unprivileged user changing their own
password.

```bash
sudo tee /usr/local/sbin/sync-smb-password >/dev/null <<'EOF'
#!/bin/sh
# Only touch users who already have a Samba account — skip silently for any
# other account's password change, such as root's.
pdbedit -L 2>/dev/null | cut -d: -f1 | grep -qx "$PAM_USER" || exit 0
IFS= read -r password
printf '%s\n%s\n' "$password" "$password" | smbpasswd -s "$PAM_USER" >/dev/null
EOF
sudo chmod 755 /usr/local/sbin/sync-smb-password
echo "password optional pam_exec.so expose_authtok seteuid /usr/local/sbin/sync-smb-password" \
    | sudo tee -a /etc/pam.d/common-password >/dev/null
```

Run the `smbpasswd -a` above **before** the first password change comes through
PAM: the hook deliberately no-ops for accounts `pdbedit` does not already know,
so an account with no Samba entry yet is skipped rather than created.

Without this, the Samba password is a second credential that never gets rotated
by `chage -d 0` and has to be changed separately.

!!! note "One server-wide setting comes with it"
    The shipped file sets `map to guest = bad user`, which is what lets
    credential-free read-only browsing work — but it applies to the whole
    server, not just these two shares. That is the right default for an
    appliance; do not include this file on a general-purpose file server.

`/var/lib/braemons/vstimd` is created by systemd's `StateDirectory=` the first
time `vstimd.service` runs, and Samba won't export a path that doesn't exist
yet — create it up front if you're setting this up before first boot:

```bash
sudo install -d -m 0755 /var/lib/braemons
```

Only put Samba on a network you trust — the read-only guest share means
anyone who can reach the box on the LAN can browse rig-config and saved
scene-configs with no credentials at all.

From Windows, reach the shares by typing `\\vstimd-XXXXXX\vstimd-config` into
Explorer. The rig will not appear on its own in Explorer's *Network* list:
Samba announces over NetBIOS, and modern Windows builds that list from
WS-Discovery. `sudo apt install wsdd2 && sudo systemctl enable --now wsdd2` on
the rig if you want the icon — `wsdd` itself is gone from Debian as of trixie.

## 7. Boot straight into vstimd

The package post-install already registered a "Boot to vstimd" bootloader entry
(GRUB on x86, extlinux on Jetson/Pi) — check it landed, then enable the unit:

```bash
vstimd-boot-entry --dry-run     # "entry already present" = nothing to do
sudo systemctl enable vstimd
```

To make vstimd the *default* target instead of a menu choice — what the Pi image
does — `sudo systemctl set-default vstimd.target`. See
[Deployment → Boot to vstimd](deployment.md#boot-to-vstimd) for the details and
the manual fallback.

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
