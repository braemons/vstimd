# Raspberry Pi 5 rig image

The ready-to-flash Raspberry Pi image has moved to
**[braemons/rig](https://github.com/braemons/rig)**, because it is a whole rig
rather than a vstimd artifact: vstimd, `gpiochip-daqd`, statemachined,
mousewheeld, triald, the console and the command-line tools, all from the
braemons apt archive, plus SSH, the Samba shares and the Pi 5 defaults.

- **Download:** `braemons-<version>-raspios-lite-arm64.img.xz` from
  [braemons/rig's releases](https://github.com/braemons/rig/releases).
  Releases of vstimd up to `v0.3.0-alpha2` attached a `vstimd-…` image of
  their own; later ones do not.
- **Flashing, first boot, SSH and SMB access, building it yourself:**
  [docs/raspberry-pi-image.md](https://github.com/braemons/rig/blob/main/docs/raspberry-pi-image.md)
  in braemons/rig.

What changed for vstimd on the image:

| | before | now |
|---|---|---|
| Login | `vstimd-admin` / `vstimd` | `braemons-admin` / `braemons` (change forced at first login) |
| Samba shares | `vstimd-config`, `vstimd-data` | `braemons-config`, `braemons-data` (same paths), from `braemons-rig` |
| vstimd | the release's own `.deb` | `braemons-vstimd` from the archive |

The rest of vstimd's part is unchanged: the rig boots into `vstimd.target`, and
`raspberry-pi-5.toml` is installed as `/etc/braemons/vstimd-rig-config.toml`.
To set up a rig by hand instead, follow [Manual appliance setup](appliance-setup.md).
