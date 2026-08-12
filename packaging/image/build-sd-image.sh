#!/bin/bash
# build-sd-image.sh — turn a stock Raspberry Pi OS Lite (arm64) image into a
# ready-to-flash vstimd appliance image: vstimd + gpiochip-daqd installed
# from locally-built .debs, sshd + an admin user, and Samba shares for
# /etc/braemons (settings) and /var/lib/braemons (saved stim-configs). Also bakes in
# the DisplayLink/evdi driver (auxiliary screens only), an Energy-Efficient
# Ethernet workaround, and a few interactive tools.
#
# Runs a stock RPi OS image through a loop-mounted chroot rather than a
# from-scratch build (pi-gen) — cheaper to build/iterate since it reuses the
# .deb postinst logic (sysusers, hostname unit, avahi template) unchanged.
# Needs root + qemu-user-static (arm64 binfmt) since the base image is
# arm64 and this is expected to run on an amd64 CI/dev host — see
# packaging/docker/Dockerfile.image-builder / `make image`, which provides
# both. Not meant to be run outside that container.
#
# Usage: build-sd-image.sh <vstimd.deb> <gpiochip-daqd.deb>
set -euo pipefail

[ "$(id -u)" -eq 0 ] || { echo "error: must run as root (needs loop devices + chroot)" >&2; exit 1; }
[ $# -eq 2 ] || { echo "usage: $0 <vstimd.deb> <gpiochip-daqd.deb>" >&2; exit 1; }
VSTIMD_DEB=$(readlink -f "$1")
GPIOCHIP_DEB=$(readlink -f "$2")
[ -f "$VSTIMD_DEB" ]   || { echo "error: $VSTIMD_DEB not found" >&2; exit 1; }
[ -f "$GPIOCHIP_DEB" ] || { echo "error: $GPIOCHIP_DEB not found" >&2; exit 1; }

BASE_IMAGE_URL="${BASE_IMAGE_URL:-https://downloads.raspberrypi.com/raspios_lite_arm64_latest}"
CACHE_DIR="${CACHE_DIR:-packaging/image/.cache}"
DIST_DIR="${DIST_DIR:-dist}"
IMAGE_VERSION="${IMAGE_VERSION:-$(date +%Y%m%d)}"
OUT_BASENAME="${OUT_BASENAME:-vstimd-${IMAGE_VERSION}-raspios-lite-arm64}"

# Login user baked into the image (SSH + Samba). Deliberately NOT named
# "vstimd" — that's the unprivileged system account the vstimd.service unit
# runs as (see packaging/sysusers/vstimd.conf); reusing it here would clash.
IMAGE_USER="${VSTIMD_IMAGE_USER:-vstimd-admin}"
[ "$IMAGE_USER" != "vstimd" ] || { echo "error: VSTIMD_IMAGE_USER must not be 'vstimd' — that's the service account" >&2; exit 1; }
IMAGE_PASSWORD="${VSTIMD_IMAGE_PASSWORD:-}"

# The two shares — vstimd-config (/etc/braemons) and vstimd-data
# (/var/lib/braemons, where saved stim-configs and the save-on-quit slot live)
# — are defined once in packaging/samba/vstimd-shares.conf, which the .deb
# installs. Their names are therefore fixed by that file, not overridable here.

mkdir -p "$CACHE_DIR" "$DIST_DIR"

# A fixed default password baked into every download of a public image is a
# real credential-stuffing/Mirai-style risk, so unless the caller supplies
# one, generate a fresh random one per build and hand it back out-of-band
# (never printed into a place that ends up inside the image itself).
GENERATED_PASSWORD=0
if [ -z "$IMAGE_PASSWORD" ]; then
    IMAGE_PASSWORD=$(openssl rand -base64 18)
    GENERATED_PASSWORD=1
fi

MNT=""
LOOP_DEV=""
BOOT_PART=""
ROOT_PART=""
WORK_IMG=""
cleanup() {
    set +e
    [ -n "$MNT" ] && mountpoint -q "$MNT/proc" && umount "$MNT/proc"
    [ -n "$MNT" ] && mountpoint -q "$MNT/sys"  && umount "$MNT/sys"
    [ -n "$MNT" ] && mountpoint -q "$MNT/dev"  && umount -R "$MNT/dev"
    [ -n "$MNT" ] && mountpoint -q "$MNT/boot/firmware" && umount "$MNT/boot/firmware"
    [ -n "$MNT" ] && mountpoint -q "$MNT/boot" && umount "$MNT/boot"
    [ -n "$MNT" ] && mountpoint -q "$MNT" && umount "$MNT"
    [ -n "$MNT" ] && rmdir "$MNT" 2>/dev/null
    for dev in "$BOOT_PART" "$ROOT_PART" "$LOOP_DEV"; do
        [ -n "$dev" ] && losetup -d "$dev" 2>/dev/null
    done
    [ -n "$WORK_IMG" ] && rm -f "$WORK_IMG"
}
trap cleanup EXIT

# ── 1. Base image (cached compressed download, never mutated) ───────────────

BASE_XZ="$CACHE_DIR/raspios-lite-arm64.img.xz"
if [ ! -f "$BASE_XZ" ] || [ -n "${FORCE_DOWNLOAD:-}" ]; then
    echo "==> downloading base image from $BASE_IMAGE_URL"
    curl -fL "$BASE_IMAGE_URL" -o "$BASE_XZ.tmp"
    mv "$BASE_XZ.tmp" "$BASE_XZ"
else
    echo "==> using cached base image $BASE_XZ"
fi

WORK_IMG="$CACHE_DIR/work.img"
echo "==> decompressing to working copy"
xz -dc "$BASE_XZ" > "$WORK_IMG"

# ── 2. Grow the rootfs, then loop-mount boot + root partitions ──────────────
#
# Stock RPi OS Lite leaves only a few hundred MB free in the root partition —
# nowhere near enough for build-essential + kernel headers + the DKMS evdi
# build below. Grow the image file and the root partition before chrooting.
# (The Pi expands the rootfs to fill the card on first boot anyway, so this
# only affects the shipped .img size, which xz squeezes back down.)

# --privileged gives the container a *snapshot* of the host's /dev taken at
# container start, not a live view of devtmpfs. Loop device nodes the kernel
# creates afterwards therefore never show up in here, and losetup fails with
# ENOENT on the very device it just allocated. Pre-create the whole pool of
# nodes (major 7) so whichever index losetup picks already exists.
echo "==> pre-creating loop device nodes"
for i in $(seq 0 "${LOOP_POOL:-63}"); do
    [ -e "/dev/loop$i" ] || mknod "/dev/loop$i" b 7 "$i"
done

echo "==> growing image by ${GROW_SIZE:-4G}"
truncate -s "+${GROW_SIZE:-4G}" "$WORK_IMG"

# Whole-disk loop, only to rewrite the partition table.
LOOP_DEV=$(losetup -f --show "$WORK_IMG")
parted -s "$LOOP_DEV" resizepart 2 100%

# `losetup -P` partition devices (/dev/loopNp2) are created in the host's
# devtmpfs, which a container's private /dev never sees — so map each
# partition with its own offset/sizelimit loop device instead. partx reads
# the table straight off the whole-disk loop, no partition nodes needed.
read -r BOOT_START BOOT_SECTORS <<<"$(partx -g -o START,SECTORS -n 1 "$LOOP_DEV")"
read -r ROOT_START ROOT_SECTORS <<<"$(partx -g -o START,SECTORS -n 2 "$LOOP_DEV")"

# Deliberately keep the whole-disk loop attached until teardown: detaching it
# here and immediately calling `losetup -f` races the kernel's asynchronous
# teardown, which hands back the device still being torn down and then fails
# to set it up.
BOOT_PART=$(losetup -f --show --offset $((BOOT_START * 512)) --sizelimit $((BOOT_SECTORS * 512)) "$WORK_IMG")
ROOT_PART=$(losetup -f --show --offset $((ROOT_START * 512)) --sizelimit $((ROOT_SECTORS * 512)) "$WORK_IMG")

echo "==> resizing root filesystem"
e2fsck -fy "$ROOT_PART" || true   # e2fsck exits 1 when it fixed something
resize2fs "$ROOT_PART"

MNT=$(mktemp -d)
mount "$ROOT_PART" "$MNT"

# Bookworm mounts the FAT boot partition at /boot/firmware; older Bullseye
# images mount it directly at /boot.
if grep -q '/boot/firmware' "$MNT/etc/fstab" 2>/dev/null; then
    BOOT_MOUNT="$MNT/boot/firmware"
else
    BOOT_MOUNT="$MNT/boot"
fi
mount "$BOOT_PART" "$BOOT_MOUNT"

# ── 3. Chroot setup: qemu binfmt + bind mounts ───────────────────────────────

QEMU_STATIC=$(command -v qemu-aarch64-static)
cp "$QEMU_STATIC" "$MNT/usr/bin/"

mount -t proc proc "$MNT/proc"
mount -t sysfs sysfs "$MNT/sys"
mount --rbind /dev "$MNT/dev"

cp /etc/resolv.conf "$MNT/etc/resolv.conf"

# ── 4. Install packages + configure services inside the chroot ──────────────

mkdir -p "$MNT/root/debs"
cp "$VSTIMD_DEB" "$GPIOCHIP_DEB" "$MNT/root/debs/"

# Archive signing key for in-place updates. Optional: until a key has been
# generated (packaging/apt/README.md), the image simply ships without an update
# source and rigs are updated by re-flashing.
APT_KEYRING="${APT_KEYRING:-packaging/apt/braemons-archive-keyring.asc}"
APT_REPO_URL="${APT_REPO_URL:-https://braemons.github.io/packages}"
# Pre-release images track the 'testing' suite so a rig flashed from an alpha
# keeps getting alphas, and a 'stable' rig is never upgraded onto one.
case "$IMAGE_VERSION" in
    *~*|*alpha*|*beta*|*rc*) APT_SUITE="${APT_SUITE:-testing}" ;;
    *)                       APT_SUITE="${APT_SUITE:-stable}"  ;;
esac

if [ -f "$APT_KEYRING" ]; then
    cp "$APT_KEYRING" "$MNT/root/braemons-archive-keyring.asc"
    echo "apt updates: enabled ($APT_REPO_URL, suite $APT_SUITE)"
else
    echo "apt updates: DISABLED — $APT_KEYRING not found."
    echo "             The image will have no update source; see packaging/apt/README.md."
fi

cat > "$MNT/root/setup.sh" <<CHROOT_EOF
#!/bin/bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends openssh-server samba avahi-daemon ethtool

# smb.conf's 'unix password sync' + 'passwd program' below already sync
# Samba -> Unix (running smbpasswd, or changing the password from a Windows
# client, also updates the Unix password). This closes the loop in the other
# direction: without it, only the SSH/login password gets rotated at first
# login (the forced prompt from 'chage -d 0' below) and the factory Samba
# password lingers indefinitely unless someone remembers to run smbpasswd
# separately.
#
# Ubuntu ships a pam_smbpass module (libpam-smbpass) for exactly this, but
# Debian — which this image is based on — does not package it at all, so
# 'apt-get install libpam-smbpass' fails outright here. pam_exec(8) is a
# Debian-provided equivalent: 'expose_authtok' hands the new plaintext
# password to a script on stdin during the PAM password-change stack, and
# 'seteuid' runs that script with effective root (needed for smbpasswd's -s
# flag, which is root-only) even though passwd's real caller is the
# unprivileged user going through the forced first-login change.
cat > /usr/local/sbin/sync-smb-password <<'SYNC_EOF'
#!/bin/sh
# Only touch users who already have a Samba account (e.g. IMAGE_USER, seeded
# by smbpasswd -a below) — skip silently for any other account's password
# change, such as root's.
pdbedit -L 2>/dev/null | cut -d: -f1 | grep -qx "\$PAM_USER" || exit 0
IFS= read -r password
printf '%s\n%s\n' "\$password" "\$password" | smbpasswd -s "\$PAM_USER" >/dev/null
SYNC_EOF
chmod 755 /usr/local/sbin/sync-smb-password
echo "password optional pam_exec.so expose_authtok seteuid /usr/local/sbin/sync-smb-password" >> /etc/pam.d/common-password

# Convenience tools for anyone who SSHes into the appliance to poke at it.
apt-get install -y --no-install-recommends btop vim tmux

# git + the system-level build requirements for compiling vstimd on the
# appliance itself (cargo build --release / cargo test / cargo clippy per
# CLAUDE.md), mirroring packaging/docker/Dockerfile.deb-builder's apt list
# (minus its cross-compile-only and pre-built-runtime-lib entries, which
# don't apply to a native on-device build). The Rust toolchain itself isn't
# an apt package (Debian's rustc trails the edition vstimd needs) — installed
# via rustup for IMAGE_USER below, after that user exists.
apt-get install -y --no-install-recommends \
    git build-essential pkg-config \
    libinput-dev libudev-dev libdrm-dev libwayland-dev \
    protobuf-compiler

# Energy-Efficient Ethernet makes the Pi 5's NIC drop connections (see
# docs/developer/platform-notes.md). A udev rule rather than a oneshot unit so
# it applies whenever a wired NIC appears, including USB adapters and
# re-enumeration, with no ordering against network.target to get wrong.
# ethtool exits non-zero on NICs that don't implement EEE at all; that's
# expected on some adapters, so don't let it fail the rule.
#
# ACTION=="add" alone is not enough: it fires the instant the netdev is
# created, which on the Pi 5's onboard NIC is well before the PHY has linked
# up and autonegotiated a mode — ethtool --set-eee errors out at that point
# (silently, because of the trailing "|| true"), so the setting never
# actually takes. Also matching ACTION=="change" (fired on carrier/link-state
# transitions, i.e. when the cable is plugged in and negotiation completes)
# re-applies it once the link is actually in a state where it can be set.
cat > /etc/udev/rules.d/80-disable-eee.rules <<'UDEV_EOF'
ACTION=="add|change", SUBSYSTEM=="net", KERNEL=="eth*", \
  RUN+="/bin/sh -c '/usr/sbin/ethtool --set-eee %k eee off || true'"
UDEV_EOF

# ── DisplayLink / evdi ──────────────────────────────────────────────────────
# Drives a USB screen via vstimd's --evdi backend: auxiliary/status displays,
# and stimulus output for behavioral-training setups. Not for recording
# sessions — DisplayLink has no GPU vsync, so stimulus onset cannot be trusted
# to the frame (see docs/developer/platform-notes.md).
#
# Synaptics' displaylink-driver package builds evdi through DKMS, and DKMS
# targets \$(uname -r) — which inside this chroot is the amd64 build host's
# kernel, not the image's. Left alone, evdi's postinst fails ("kernel headers
# for kernel <host> cannot be found"), displaylink-driver is left unconfigured
# on its dependency, and any later dpkg --configure -a fails the same way.
# So shim uname -r to the image's kernel for the duration of the install and
# let the package's own machinery build against the right target.
apt-get install -y --no-install-recommends \
    dkms build-essential libdrm-dev libusb-1.0-0-dev pkg-config unzip

# Kernel headers for the image's kernels: Pi 5 (2712) and Pi 4/older (v8).
apt-get install -y linux-headers-rpi-2712 linux-headers-rpi-v8

# Prefer the Pi 5 kernel; the loop further down covers the rest.
TARGET_KVER=\$(ls /lib/modules | grep -- '2712' | sort -V | tail -1)
[ -n "\$TARGET_KVER" ] || TARGET_KVER=\$(ls /lib/modules | sort -V | tail -1)
echo "==> shimming uname -r to \$TARGET_KVER for the DisplayLink install"

cat > /usr/sbin/uname <<UNAME_EOF
#!/bin/sh
# Build-time shim (removed before the image is packed): report the image's
# kernel so DKMS builds for it instead of the build host's kernel.
for a in "\\\$@"; do
    case "\\\$a" in -r|--kernel-release) echo "\$TARGET_KVER"; exit 0;; esac
done
exec /bin/uname "\\\$@"
UNAME_EOF
chmod +x /usr/sbin/uname

curl -fL -o /root/synaptics-repository-keyring.deb \
    https://www.synaptics.com/sites/default/files/Ubuntu/pool/stable/main/all/synaptics-repository-keyring.deb
dpkg -i /root/synaptics-repository-keyring.deb
rm -f /root/synaptics-repository-keyring.deb
apt-get update

# evdi 1.15.0 (what the Synaptics repo serves) fails to build its DKMS module
# on current RPi kernels; 1.14.16 is known-good. This MUST be pinned before
# displaylink-driver is installed: apt pulls evdi in to satisfy that
# dependency, and an unpinned 1.15.0 fails in its own postinst (exit 10),
# which aborts this script under set -e long before any later pin applies.
# Priority 1001 also keeps a later 'apt-get upgrade' on the appliance from
# pulling 1.15.x back in.
cat > /etc/apt/preferences.d/evdi-pin <<APT_PREF
Package: evdi
Pin: version 1.14.16*
Pin-Priority: 1001
APT_PREF

apt-get install -y displaylink-driver

# Cover the image's other kernels (Pi 4/v8) too — the postinst above only
# built for TARGET_KVER.
EVDI_SRC=\$(ls -d /usr/src/evdi-* 2>/dev/null | sort -V | tail -1)
if [ -n "\$EVDI_SRC" ]; then
    EVDI_VER=\${EVDI_SRC##*/evdi-}
    BUILT=0
    for KVER in \$(ls /lib/modules); do
        [ -d "/lib/modules/\$KVER/build" ] || continue
        echo "==> building evdi \$EVDI_VER for kernel \$KVER"
        if dkms install "evdi/\$EVDI_VER" -k "\$KVER" --force; then
            BUILT=\$((BUILT + 1))
        else
            echo "warning: evdi build failed for \$KVER" >&2
        fi
    done
    [ "\$BUILT" -gt 0 ] || { echo "error: evdi built for no kernel at all" >&2; exit 1; }
else
    echo "error: displaylink-driver did not provide evdi DKMS sources" >&2
    exit 1
fi

rm -f /usr/sbin/uname

# Deliberately NOT 'systemctl enable displaylink-driver': the unit ships with
# no [Install] section, so enabling it is a silent no-op (systemctl warns and
# exits 0). It is started on demand by /lib/udev/rules.d/99-displaylink.rules
# -> /opt/displaylink/udev.sh -> 'systemctl start --no-block displaylink-driver'
# when a DisplayLink device (vendor 17e9) appears, including at boot coldplug.

# vstimd + gpiochip-daqd, from the locally-built .debs (postinst runs here:
# creates the vstimd system user, /etc/braemons, the hostname unit, etc.).
dpkg -i /root/debs/*.deb || true
apt-get install -y -f

# gpiochip-daqd's postinst only installs the empty default-config.toml to
# /etc/braemons/gpiochip-daqd-config.toml (it ships board-specific configs as
# read-only examples under /usr/share/braemons/gpiochip-daqd/ instead, since
# the .deb is board-agnostic). This image IS a specific board, so overwrite
# with the matching example — a freshly flashed card must have GPIO already
# wired up, not need someone to SSH in and cp it by hand.
install -m 0644 \
    /usr/share/braemons/gpiochip-daqd/raspberry-pi-5_in16_out4.toml \
    /etc/braemons/gpiochip-daqd-config.toml

# Same deal for vstimd's own rig-config: 'make install' (the .deb's postinst
# path) only ever installs the generic, everything-commented-out
# server/config/default-rig-config.toml to
# /etc/braemons/vstimd-rig-config.toml (see the Makefile's RIG_CONFIG/EXAMPLES
# split) because the .deb itself is board-agnostic too. Overwrite with the
# Pi 5 example so a freshly flashed card boots with correct VTL/GPIO settings
# instead of a config with everything commented out.
install -m 0644 \
    /usr/share/braemons/vstimd/raspberry-pi-5.toml \
    /etc/braemons/vstimd-rig-config.toml

# ── In-place updates ─────────────────────────────────────────────────────────
# Point apt at the vstimd archive so a deployed rig upgrades with
# 'apt update && apt upgrade' instead of being re-flashed. Re-flashing discards
# /etc/braemons (the rig config and any saved stimulus configs) and
# /var/lib/braemons, which is exactly what an upgrade must preserve:
# vstimd-rig-config.toml is a dpkg conffile, so local edits survive.
if [ -f /root/braemons-archive-keyring.asc ]; then
    # Kept ASCII-armored as .asc, which apt reads directly — dearmoring would
    # need gpg(1), which is not guaranteed present on a Lite image.
    install -D -m 0644 /root/braemons-archive-keyring.asc \
        /etc/apt/keyrings/braemons.asc
    rm -f /root/braemons-archive-keyring.asc

    # deb822 format: trust is scoped to this one archive via Signed-By rather
    # than added to the global keyring.
    cat > /etc/apt/sources.list.d/braemons.sources <<APT_SOURCES
Types: deb
URIs: ${APT_REPO_URL}
Suites: ${APT_SUITE}
Components: main
Architectures: arm64
Signed-By: /etc/apt/keyrings/braemons.asc
APT_SOURCES

    # A conffile prompt on a headless rig hangs the upgrade indefinitely —
    # there is no one at a terminal to answer it. Keep the installed file and
    # drop the package's version alongside as .dpkg-dist to diff later.
    # Deliberately global rather than scoped to vstimd: any package can prompt,
    # and the rig is equally unattended for all of them.
    cat > /etc/apt/apt.conf.d/90braemons-unattended <<'APT_CONF'
Dpkg::Options {
   "--force-confdef";
   "--force-confold";
};
APT_CONF
fi

# Admin login for SSH + Samba. The password is expired at the very end of this
# script rather than here -- see the 'chage -d 0' below for why the ordering
# matters.
useradd -m -s /bin/bash -G sudo "${IMAGE_USER}"
echo "${IMAGE_USER}:${IMAGE_PASSWORD}" | chpasswd
systemctl enable ssh

# Stock Raspberry Pi OS's first-boot user wizard (raspberrypi-sys-mods /
# userconf-pi, WantedBy=multi-user.target -- which vstimd.target requires, so
# it runs regardless of vstimd.target being the default target): unless
# /boot/firmware/userconf.txt was preseeded (only Raspberry Pi Imager's own
# OS-customisation writes that -- balenaEtcher and any other raw-dd flash do
# not), it blocks the very first boot on an interactive username/password
# prompt on /dev/tty8. IMAGE_USER already exists at this point (created
# above), so the wizard is redundant -- disable it outright rather than ship
# an appliance that hangs on first boot waiting for a keyboard nobody has
# plugged in.
systemctl disable userconfig.service 2>/dev/null || true

# Rust toolchain for IMAGE_USER, so 'cargo build --release'/'cargo test'/
# 'cargo clippy' (see CLAUDE.md) work out of the box for anyone who git
# clones vstimd onto the appliance to build from source. Debian's packaged
# rustc trails the edition vstimd needs, so install via rustup rather than
# apt (which is why this needs its own step -- apt-installed git and the
# system -dev libs above aren't sufficient on their own). Installed as
# IMAGE_USER, not root: cargo/rustup are meant to be per-user tooling, and
# running the installer as that user avoids ending up with a root-owned
# toolchain a non-root SSH login can't update.
su - "${IMAGE_USER}" -c \
    'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default'

# Force a password change at first login: a publicly downloadable image must
# not leave a known password valid indefinitely, generated or not.
#
# MUST come after every 'su - IMAGE_USER' above. chage -d 0 marks the password
# as expired, and su runs the full PAM account stack -- which then refuses to
# hand over a shell until the password is changed, prompting for it on a stdin
# that is not a terminal inside this chroot:
#
#     You are required to change your password immediately (administrator enforced)
#     Current password: su: Authentication token manipulation error
#
# That is a non-zero exit under 'set -e', so expiring the password before the
# rustup install above fails the whole image build. Anything added later that
# needs to become IMAGE_USER must go above this line, or use runuser (whose
# PAM config has no account stage) instead of su.
chage -d 0 "${IMAGE_USER}"

# Samba share for the vstimd/gpiochip-daqd config directory.
#
# Samba keeps its own credential in passdb.tdb, which the 'chage -d 0' above
# does NOT govern — that only gates Unix/SSH logins. useradd/chpasswd (used
# to bootstrap the account above) don't go through PAM either, so this
# initial smbpasswd call is still needed to seed the Samba side to the same
# factory password. From here on, though, the pam_exec hook installed above
# keeps the two in sync automatically: the forced first-login SSH password
# change runs through PAM, which pushes the new password into Samba's passdb
# too — no separate manual smbpasswd run needed after today.
#
# Seeding here is also what arms that hook: sync-smb-password deliberately
# no-ops for accounts pdbedit does not already know, so the Samba account has
# to exist before the first password change comes through PAM.
#
# NB: this heredoc is unquoted (it interpolates IMAGE_USER etc.), so backticks
# and $ in here run on the BUILD HOST. Keep both out of comments.
printf '%s\n%s\n' "${IMAGE_PASSWORD}" "${IMAGE_PASSWORD}" | smbpasswd -s -a "${IMAGE_USER}"

# systemd creates /var/lib/braemons/vstimd on first start via StateDirectory=,
# but Samba refuses to export a path that does not exist yet — so the share
# would be dead on a freshly flashed card until vstimd had run once.
install -d -m 0755 /var/lib/braemons

# The share stanzas themselves ship in the .deb (installed just above) as
# /usr/share/braemons/vstimd/vstimd-shares.conf, so the image and a hand-built
# rig get byte-identical shares from one definition. Include it rather than
# copying its contents in, so a later 'apt upgrade' that revises the stanzas
# reaches this rig too.
#
# A repeated [global] is legal and merges into the first one, which keeps this
# an append rather than an edit of the stock smb.conf. The included file opens
# with its own [global] (for the password-sync settings and 'map to guest')
# before declaring the two shares; nothing follows this in smb.conf, so there
# is no section for the include to leak into.
#
# IMAGE_USER gets write access through the shipped file's 'write list = @sudo
# @wheel' — it was created with -G sudo above.
cat >> /etc/samba/smb.conf <<SMB_EOF

[global]
   include = /usr/share/braemons/vstimd/vstimd-shares.conf
SMB_EOF
# Fails the build loudly if the shipped stanzas or the include are malformed,
# rather than shipping a card whose shares silently never appear.
testparm -s >/dev/null

# No backticks/\$ here — see the note above.
cat >> /etc/motd <<'MOTD_EOF'

vstimd appliance
  Samba shares: vstimd-config -> /etc/braemons (settings, read-only to guests)
                vstimd-data   -> /var/lib/braemons (saved stim-configs, read-only to guests)
  Anyone on the LAN can browse them read-only with no credentials. Writing
  requires this account's login — changing your SSH password at first login
  updates the Samba one too (a PAM hook keeps them in sync from here on).

MOTD_EOF

systemctl enable smbd nmbd avahi-daemon

systemctl enable vstimd vstimd-hostname gpiochip-daqd
# Appliance behaviour: boot straight into vstimd.target instead of the
# normal multi-user console. vstimd.target still Requires=multi-user.target,
# so networking/ssh/samba come up first — see packaging/systemd/vstimd.target.
systemctl set-default vstimd.target

apt-get clean
rm -f /root/debs/*.deb
CHROOT_EOF
chmod +x "$MNT/root/setup.sh"
chroot "$MNT" /root/setup.sh
rm -f "$MNT/root/setup.sh" "$MNT/usr/bin/$(basename "$QEMU_STATIC")"

# ── 5. Unmount, detach, repack ────────────────────────────────────────────

umount "$MNT/proc" "$MNT/sys"
umount -R "$MNT/dev"
umount "$BOOT_MOUNT"
umount "$MNT"
rmdir "$MNT"; MNT=""
losetup -d "$ROOT_PART"; ROOT_PART=""
losetup -d "$BOOT_PART"; BOOT_PART=""
losetup -d "$LOOP_DEV";  LOOP_DEV=""

echo "==> compressing image"
OUT_IMG="$DIST_DIR/${OUT_BASENAME}.img.xz"
xz -T0 -c "$WORK_IMG" > "$OUT_IMG"
rm -f "$WORK_IMG"; WORK_IMG=""
sha256sum "$OUT_IMG" | sed "s|$DIST_DIR/||" > "$OUT_IMG.sha256"

if [ "$GENERATED_PASSWORD" -eq 1 ]; then
    CRED_FILE="$DIST_DIR/${OUT_BASENAME}-credentials.txt"
    {
        echo "user: $IMAGE_USER"
        echo "password: $IMAGE_PASSWORD (must be changed at first login)"
    } > "$CRED_FILE"
    chmod 600 "$CRED_FILE"
    echo "==> generated login: $IMAGE_USER / $IMAGE_PASSWORD (also written to $CRED_FILE — keep this out of git/logs)"
fi

echo "==> done: $OUT_IMG"
