# Raspberry Pi 5 appliance image

Every [release](https://github.com/braemons/vstimd/releases) ships a
ready-to-flash Raspberry Pi OS Lite (arm64) image with vstimd already installed
and configured. Write it to an SD card, plug the Pi in, and it comes up as a
stimulus rig on the network — no install steps, no keyboard, no monitor needed
for setup.

This page is the flash-to-first-experiment walkthrough. To do the same thing by
hand on other hardware (Jetson, desktop x86, a different Pi model), follow
[Manual appliance setup](appliance-setup.md) instead. To build the image
yourself, see [`make image`](../developer/building.md#raspberry-pi-sd-image).

---

## What you need

| | |
|---|---|
| Board | Raspberry Pi 5 (validated on the 8 GB Model B Rev 1.1) |
| Power | The official 27 W USB-C supply. A weaker one boots but throttles — a red status LED after firmware init usually means an underpowered PSU |
| Storage | microSD card, **16 GB or larger** (the image expands to ~6.5 GB before the rootfs grows to fill the card) |
| Network | Wired Ethernet to the same LAN/subnet as your experiment PC. Discovery is mDNS, which does not cross subnets |
| Display | HDMI cable into the Pi 5's **micro-HDMI** port — use `HDMI0`, the one nearest the USB-C connector |
| Optional | GPIO wiring for hardware triggers; a DisplayLink USB screen for auxiliary output |

---

## 1. Download and verify

From the [releases page](https://github.com/braemons/vstimd/releases), grab both:

- `vstimd-<version>-raspios-lite-arm64.img.xz`
- `vstimd-<version>-raspios-lite-arm64.img.xz.sha256`

Then check the download before spending ten minutes writing a corrupt card. The
`.sha256` file names the image by its bare filename, so run the check from the
directory holding both:

=== "Linux / macOS"

    ```bash
    sha256sum -c vstimd-*-raspios-lite-arm64.img.xz.sha256
    # → vstimd-….img.xz: OK
    ```

    On macOS use `shasum -a 256 -c` instead.

=== "Windows (PowerShell)"

    ```powershell
    (Get-FileHash .\vstimd-0.1.0-raspios-lite-arm64.img.xz -Algorithm SHA256).Hash.ToLower()
    Get-Content .\vstimd-0.1.0-raspios-lite-arm64.img.xz.sha256
    # the two hashes must match
    ```

!!! note "Pre-releases"
    A tag like `v0.1.0-alpha6` is published as a GitHub **pre-release**, so it is
    not badged "Latest" — tick *Show pre-releases* on the releases page if you are
    tracking alphas. Images built from a pre-release also track the archive's
    `testing` suite, so they keep receiving pre-releases; see
    [Updating](#5-updating-never-re-flash).

## 2. Flash with balenaEtcher

[balenaEtcher](https://etcher.balena.io/) reads `.xz` directly — **do not
decompress the image first**.

1. Insert the microSD card (a USB reader is fine).
2. Open Etcher → **Flash from file** → select the `.img.xz` you just verified.
3. **Select target** → pick the card. Check the size and drive letter; Etcher
   hides system drives, but confirm you are not about to overwrite a backup disk.
4. **Flash!** — then let it finish its verification pass.
5. Windows may pop up *"You need to format the disk before you can use it"* when
   Etcher is done. **Cancel it.** Windows is seeing the Linux root partition it
   cannot read; formatting would destroy the card you just wrote.

Eject the card and move on to first boot.

??? tip "Alternatives to Etcher"
    **Raspberry Pi Imager** — choose *Use custom* and select the `.img.xz`. When
    it offers OS customisation, choose **No**: the image already has its own
    login user, and Imager's customisation writes a `userconf.txt` for a
    first-boot wizard this image deliberately disables.

    **`dd` (Linux/macOS)** — no verification pass, so check the hash first:

    ```bash
    xz -dc vstimd-<version>-raspios-lite-arm64.img.xz | sudo dd of=/dev/sdX bs=4M conv=fsync status=progress
    ```

    Get `/dev/sdX` wrong and you overwrite the wrong disk. `lsblk` before, always.

## 3. First boot

Insert the card, connect Ethernet and the display, then power up.

The first boot expands the root filesystem to fill the card and reboots once by
itself; allow a couple of minutes before the rig answers. It then boots straight
into `vstimd.target` — no desktop, no login prompt on the primary console —
and the attached display shows vstimd's output (a black screen with the
configured background, not a terminal).

There is **no interactive setup wizard**: the stock Raspberry Pi OS first-boot
user prompt is disabled in the image precisely so a raw-flashed card
(Etcher, `dd`) does not sit waiting on a keyboard nobody plugged in.

### Find it on the network

The rig names itself `vstimd-XXXXXX` from its MAC address and advertises
`_vstimd._tcp` over mDNS — see [Discovery & hostnames](discovery.md) for the
full policy.

```console
$ vstimd-client discover
ID             HOSTNAME             ADDRESSES   ADDRESS
vstimd-a1b2c3  vstimd-a1b2c3.local  10.0.1.42   tcp://vstimd-a1b2c3.local:5555
```

Without the Python client installed, `avahi-browse -r _vstimd._tcp` (Linux),
`dns-sd -B _vstimd._tcp` (macOS, and Windows with Bonjour installed — see
[Discovery on Windows](discovery.md#on-windows-without-the-python-client)), or
your router's DHCP lease table will do.

Quickest confirmation that it is alive: browse to
**`http://vstimd-XXXXXX.local:8080`** — the [web control UI](../client/web.md)
is served from the rig itself and needs nothing installed locally.

## 4. Get in

### SSH

```bash
ssh vstimd-admin@vstimd-a1b2c3.local
```

| | |
|---|---|
| User | `vstimd-admin` — a normal `sudo` account, **not** the `vstimd` service account (which has no login shell) |
| Password | `vstimd`, unless the release notes for your download say otherwise |
| First login | You are **forced to change the password** (`chage -d 0`) before you get a shell |

Changing it also changes the Samba password: a `pam_exec` hook sits in the
password-change stack and pushes the new value into Samba's passdb, so the two
never drift apart.

!!! danger "The default login is public"
    Anyone can download the image and read the password out of it. Change it at
    first login (you are made to), and treat these rigs as **lab-network
    devices** — the network is the security boundary. Do not expose one to the
    internet.

Windows has a built-in `ssh` client in PowerShell; PuTTY works too (host
`vstimd-a1b2c3.local`, port 22).

### Files over SMB/CIFS

Two shares are exported, so you can edit configs and pull saved scenes from a
lab Windows or macOS machine without an SSH session:

| Share | Path on the rig | Contents |
|---|---|---|
| `vstimd-config` | `/etc/braemons` | `vstimd-rig-config.toml`, `gpiochip-daqd-config.toml` |
| `vstimd-data` | `/var/lib/braemons` | Saved stim-configs and the save-on-quit slot, under `vstimd/` |

Both are **browsable read-only by anyone on the LAN with no credentials**;
writing requires the `vstimd-admin` login.

=== "Windows"

    In Explorer's address bar:

    ```
    \\vstimd-a1b2c3\vstimd-config
    ```

    Read-only browsing needs no credentials. To write, map it as a drive with
    *Connect using different credentials* and log in as `vstimd-admin`.

=== "macOS"

    Finder → **Go → Connect to Server** (++cmd+k++):

    ```
    smb://vstimd-a1b2c3.local/vstimd-data
    ```

    Choose *Guest* to read, or *Registered User* → `vstimd-admin` to write.

=== "Linux"

    ```bash
    # Read-only, no credentials:
    sudo mount -t cifs //vstimd-a1b2c3.local/vstimd-data /mnt -o guest,vers=3.0

    # Read-write:
    sudo mount -t cifs //vstimd-a1b2c3.local/vstimd-config /mnt \
        -o username=vstimd-admin,vers=3.0
    ```

    Requires `cifs-utils`. A desktop file manager can also open
    `smb://vstimd-a1b2c3.local/` directly via gvfs.

!!! info "The rig shows up in Explorer's Network list"
    Samba announces itself over NetBIOS, but modern Windows builds that list
    from WS-Discovery instead — so the image also ships `wsdd2`, enabled by
    default, to cover that. On a hand-built rig without it, `\\vstimd-a1b2c3`
    still connects fine typed directly; only the icon is missing.

After editing the rig config, restart the service so it takes effect:

```bash
sudo systemctl restart vstimd
```

### Drive it from an experiment script

```python
from vstimd import Connection

with Connection("tcp://vstimd-a1b2c3.local:5555") as conn:
    print(conn.system.query_server_info())
```

or from a shell: `vstimd-client --host vstimd-a1b2c3 info`.

## 5. Updating — never re-flash

Re-flashing wipes `/etc/braemons` and `/var/lib/braemons`: the rig config and
every saved stimulus config. The image ships with the braemons apt archive
already configured and its signing key installed, so upgrades are in place:

```bash
sudo apt update && sudo apt upgrade
```

See [Deployment → Updating a deployed rig](deployment.md#updating-a-deployed-rig)
for how the `stable`/`testing` suites and conffile handling work. Reserve
re-flashing for a new rig or a failed card.

---

## What is baked into the image

| | |
|---|---|
| Base | Raspberry Pi OS Lite (arm64), rootfs grown by 4 GB for the DKMS builds |
| Packages | `braemons-vstimd` + `braemons-gpiochip-daqd` from the release's own `.deb`s |
| Boot | Default target set to `vstimd.target`; `vstimd`, `vstimd-hostname`, and `gpiochip-daqd` enabled |
| Rig config | `/usr/share/braemons/vstimd/raspberry-pi-5.toml` installed as `/etc/braemons/vstimd-rig-config.toml` (not the generic all-commented-out default) |
| GPIO config | `raspberry-pi-5_in16_out4.toml` installed as `/etc/braemons/gpiochip-daqd-config.toml` |
| Services | `sshd`, `smbd`/`nmbd` with both shares, `avahi-daemon`, `wsdd2` |
| Login | `vstimd-admin` in `sudo`, password change forced at first login, Samba password kept in sync by a `pam_exec` hook |
| Updates | `/etc/apt/sources.list.d/braemons.sources` + `/etc/apt/keyrings/braemons.asc`, plus an unattended-upgrade conffile policy so a headless rig never hangs on a dpkg prompt |
| DisplayLink | `displaylink-driver` with `evdi` pinned to 1.14.16 and DKMS-built for both shipped kernels (see [caveats](../developer/platform-notes.md)) |
| Workaround | udev rule disabling Energy-Efficient Ethernet, which otherwise drops the Pi 5's link |
| Dev tooling | `git`, `build-essential`, the vstimd build dependencies, a rustup toolchain for `vstimd-admin`, plus `btop`, `vim`, `tmux` |

The stock `dtoverlay=vc4-kms-v3d` (full KMS) that vstimd's DRM backend needs is
already the Raspberry Pi OS default — no `config.txt` edit is required.

---

## Troubleshooting

Nothing on the display
:   Check the cable is in the **`HDMI0`** micro-HDMI port (nearest USB-C), and
    that it was connected at boot. Then SSH in and run
    `systemctl status vstimd` and `journalctl -u vstimd -b`.

Red status LED, board does not come up
:   Almost always the power supply. Use the official 27 W USB-C unit.

The Ethernet link keeps dropping
:   Energy-Efficient Ethernet; the image already ships the udev rule that
    disables it. Confirm with `ethtool --show-eee eth0`. On a rig set up by hand,
    see [Platform notes](../developer/platform-notes.md).

Still called `raspberrypi`, or `discover` finds nothing
:   See [Discovery & hostnames → Troubleshooting](discovery.md#troubleshooting).

A DisplayLink screen flickers on and off
:   Do not power it through a USB-C power switch. And note DisplayLink output is
    only appropriate for behavioural training or auxiliary displays — it has no
    GPU vsync, so stimulus onset cannot be trusted to the frame. See
    [Platform notes](../developer/platform-notes.md).
