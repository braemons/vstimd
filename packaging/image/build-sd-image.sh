#!/bin/bash
# build-sd-image.sh — turn a stock Raspberry Pi OS Lite (arm64) image into a
# ready-to-flash vstimd appliance image: vstimd + gpiochip-daqd installed
# from locally-built .debs, sshd + an admin user, and a Samba share for
# /etc/braemons (the vstimd/gpiochip-daqd config directory).
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
SAMBA_SHARE="${SAMBA_SHARE_NAME:-vstimd-config}"

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
    [ -n "$LOOP_DEV" ] && losetup -d "$LOOP_DEV" 2>/dev/null
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

# ── 2. Loop-mount boot + root partitions ─────────────────────────────────────

echo "==> attaching loop device"
LOOP_DEV=$(losetup -fP --show "$WORK_IMG")
BOOT_PART="${LOOP_DEV}p1"
ROOT_PART="${LOOP_DEV}p2"

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

cat > "$MNT/root/setup.sh" <<CHROOT_EOF
#!/bin/bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends openssh-server samba avahi-daemon

# evdi 1.15.0 (from the Synaptics repo bundled with Raspberry Pi OS) fails to
# build its DKMS module on current RPi kernels. Pin it to 1.14.16 which is
# known-good. The preference file also prevents apt from upgrading to 1.15.x
# on future `apt-get upgrade` runs on the appliance.
cat > /etc/apt/preferences.d/evdi-pin <<APT_PREF
Package: evdi
Pin: version 1.14.16*
Pin-Priority: 1001
APT_PREF
apt-get install -y --no-install-recommends evdi

# vstimd + gpiochip-daqd, from the locally-built .debs (postinst runs here:
# creates the vstimd system user, /etc/braemons, the hostname unit, etc.).
dpkg -i /root/debs/*.deb || true
apt-get install -y -f

# Admin login for SSH + Samba. Forced password change on first login: a
# publicly downloadable image shouldn't leave a known password valid
# indefinitely, generated or not.
useradd -m -s /bin/bash -G sudo "${IMAGE_USER}"
echo "${IMAGE_USER}:${IMAGE_PASSWORD}" | chpasswd
chage -d 0 "${IMAGE_USER}"
systemctl enable ssh

# Samba share for the vstimd/gpiochip-daqd config directory.
printf '%s\n%s\n' "${IMAGE_PASSWORD}" "${IMAGE_PASSWORD}" | smbpasswd -s -a "${IMAGE_USER}"
cat >> /etc/samba/smb.conf <<SMB_EOF

[${SAMBA_SHARE}]
   path = /etc/braemons
   browseable = yes
   read only = no
   guest ok = no
   valid users = ${IMAGE_USER}
   create mask = 0644
   directory mask = 0755
SMB_EOF
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
losetup -d "$LOOP_DEV"; LOOP_DEV=""

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
