# Discovery & hostnames

Rigs are appliances: they get an address from DHCP, nobody logs into them day to
day, and a lab may have several sitting on the same switch. So a rig has to
**name itself** — uniquely, identically on every boot, with no per-device
configuration step — and then **announce itself** so clients can find it without
being told an IP address.

This page is the policy both halves follow. The client-side commands are
documented in full under [Command-line client](../client/cli.md#discovering-rigs).

---

## Hostname policy

Every packaged install ships `vstimd-hostname.service`, which runs
`/usr/sbin/vstimd-set-hostname` at boot and sets the hostname to:

```
vstimd-XXXXXX
```

where `XXXXXX` is the **last 6 hex characters of the primary interface's MAC
address**, lower-cased — e.g. a NIC at `d8:3a:dd:a1:b2:c3` gives
`vstimd-a1b2c3`.

| Property | Consequence |
|---|---|
| Derived from the MAC | Stable across reboots *and* re-flashes — no state is stored |
| 13 characters | Fits Samba's 15-character NetBIOS limit, so the SMB name matches the DNS name |
| Follows the board, not the card | Moving an SD card to another Pi renames the rig; replacing a board/NIC gives it a new name |
| Collision-resistant, not collision-proof | Two rigs clash only if the low 24 bits of their MACs match |

The interface is picked in order: `eth0`, `end0`, `eno1`, then the first
non-loopback interface that has a MAC at all. The unit runs deliberately early
(`Before=sysinit.target`) and waits up to 20 s for that interface to appear —
the Pi 5's onboard NIC hangs off the RP1 southbridge over PCIe and can enumerate
a few seconds after udev is otherwise settled.

Along the way the script also:

- writes `/etc/hostname` and calls `sethostname(2)` directly rather than going
  through `hostnamectl` — that would need `systemd-hostnamed` over D-Bus, which
  is not guaranteed to be up this early in boot;
- keeps the Debian-style `127.0.1.1 <hostname>` line in `/etc/hosts` in sync.
  Nothing else does this, and without it every local hostname lookup — including
  the ones `sudo` performs — breaks immediately after a rename;
- `try-restart`s `avahi-daemon`, `smbd`, and `nmbd` if they happen to be running
  already (a manual re-run, or a NIC swap on a live system). On a normal boot
  they have not started yet, so this is a no-op.

It is idempotent: a rig already named `vstimd-XXXXXX` is left alone.

!!! info "Avahi and Samba need no configuration"
    Both inherit the system hostname by default. The unit completes before
    `sysinit.target` — long before anything in `multi-user.target` starts — so
    they come up already seeing the generated name. There is no `host-name=` in
    `avahi-daemon.conf` and no `netbios name` in `smb.conf` to keep in step.

### Enabling it on a source install

Packages enable this for you. After `make install`:

```bash
sudo systemctl enable --now vstimd-hostname
journalctl -u vstimd-hostname       # what it picked, and from which interface
```

### Opting out

Nothing forces the policy on you — but the two halves are linked, so opt out of
both together:

```bash
sudo systemctl disable vstimd-hostname
sudo hostnamectl set-hostname my-rig-name          # keep it ≤ 15 chars for SMB
sudo rm -f /etc/avahi/services/vstimd.service      # stale: no longer re-rendered
```

The Avahi service file is only (re)written by `vstimd-set-hostname`. With the
unit disabled, an existing one keeps advertising the *old* `id=` forever — so
either delete it, or maintain it by hand from the template described below.

---

## mDNS advertisement

Packages install an Avahi service template at

```
/usr/share/braemons/vstimd/vstimd.service.avahi.tmpl
```

which `vstimd-set-hostname` renders to `/etc/avahi/services/vstimd.service` at
boot. It advertises:

| Field | Value |
|---|---|
| Service type | `_vstimd._tcp` |
| Port | 5555 (the ZMQ command API) |
| Display name | `%h` — the hostname, substituted by Avahi |
| TXT record | `id=vstimd-XXXXXX` — a literal, written by the script |

`avahi-daemon` is a **Recommends** of the `.deb`, not a hard dependency: install
it (it is preinstalled on the Raspberry Pi image) or the rig simply does not
advertise, and clients address it by IP or DNS name instead.

!!! warning "Match on the TXT record, not the display name"
    When two services advertise the same name, Avahi appends ` #2`, ` #3`, … to
    the **display name**. The `id=` TXT record is a plain literal by the time
    Avahi loads the file, so it survives that unchanged. Scripts that need to
    reach *one specific rig* must key off `id=`.

    The `id=` value cannot be produced by Avahi's own `%h` substitution:
    `avahi-service.dtd` only allows `replace-wildcards` on `<name>`, not on
    `<txt-record>`. That is why the script writes it in rather than shipping a
    static file.

`/etc/avahi/services/vstimd.service` is a **generated file** — it is overwritten
on the next boot or hostname change. Edit the template, not the output.

---

## Finding rigs from a client

```console
$ vstimd-client discover
ID             HOSTNAME             ADDRESSES   ADDRESS
vstimd-a1b2c3  vstimd-a1b2c3.local  10.0.1.42   tcp://vstimd-a1b2c3.local:5555
vstimd-ffee00  vstimd-ffee00.local  10.0.1.51   tcp://vstimd-ffee00.local:5555
```

`vstimd-client` ships with the [Python client](../client/cli.md); `discover` is
also importable, so an experiment script can resolve a rig without shelling out.
Without any vstimd tooling installed, plain Avahi works too:

```bash
avahi-browse -r _vstimd._tcp
```

An `ID` from that listing is a valid host name: a bare name gets `.local`
appended, so `vstimd-client --host vstimd-a1b2c3 info` connects straight away.

`.local` resolution is built in on macOS (Bonjour), on Windows 10 1703 and
later, and on Linux with `nss-mdns` or `systemd-resolved` installed. On a
network where `.local` is hijacked by a corporate search domain, use the IP
address from `discover` instead.

### On Windows, without the Python client

Once you know a rig's name you need nothing at all — Windows resolves `.local`
itself, so `ssh vstimd-a1b2c3.local`, `http://vstimd-a1b2c3.local:8080` and
`\\vstimd-a1b2c3` all work on a stock machine. Discovery only matters when you
do not know the name yet.

For that, use `dns-sd.exe`, which comes with Apple's Bonjour (bundled with
iTunes, or standalone as *Bonjour Print Services for Windows*):

```powershell
dns-sd -B _vstimd._tcp                      # browse; the instance name is the hostname
dns-sd -L vstimd-a1b2c3 _vstimd._tcp local  # → host:port and the id= TXT record
dns-sd -G v4 vstimd-a1b2c3.local            # → IP address
```

Both `-B` and `-L` run until ++ctrl+c++; there is no timeout flag.

!!! warning "`Resolve-DnsName` cannot do this"
    Windows' built-in mDNS support covers **A/AAAA name resolution only** — it
    has no service enumeration, so `Resolve-DnsName _vstimd._tcp.local -Type PTR`
    (or `_services._dns-sd._udp.local`) returns *DNS name does not exist*. There
    is no stock-PowerShell equivalent of `avahi-browse`; use `dns-sd`, the Python
    client, or address the rig by name.

    `avahi-browse` under WSL only reaches the LAN when WSL is in mirrored
    networking mode (`networkingMode=mirrored` in `.wslconfig`); the default NAT
    does not forward multicast.

---

## Ports

| Port | Protocol | What |
|---|---|---|
| 5555 | TCP | ZMQ command API (`tcp://0.0.0.0:5555`) |
| 8080 | TCP | [Web control UI](../client/web.md) + `/events` WebSocket |
| 5353 | UDP | mDNS (avahi) |
| 22 | TCP | SSH, if `openssh-server` is installed |
| 139, 445 | TCP | Samba — **only** on a rig where you installed it (see below) |

!!! info "The packages do not install Samba"
    File sharing is set up by the [Raspberry Pi image](raspberry-pi-image.md)
    build, not by `braemons-vstimd`. On a rig installed from a `.deb`/`.rpm` or
    from source there is no SMB server at all, so nothing appears in Windows
    Explorer and `nbtstat -A <ip>` reports *Host not found* — that is expected,
    not a discovery failure. Add it by hand with
    [Manual appliance setup → Admin access](appliance-setup.md#6-admin-access-ssh-optional-samba)
    if you want the shares.

If the rig runs a firewall, open at least 5555 and 8080:
`sudo ufw allow 5555/tcp && sudo ufw allow 8080/tcp`.

---

## Troubleshooting

The rig is still called `raspberrypi` / `ubuntu`
:   The unit did not run or could not find an interface. `systemctl status
    vstimd-hostname` and `journalctl -u vstimd-hostname` say which. A rig with no
    NIC at all (Wi-Fi-only, adapter unplugged at boot) fails after the 20 s wait.

`discover` finds nothing
:   Check `systemctl is-active avahi-daemon` on the rig and that
    `/etc/avahi/services/vstimd.service` exists. mDNS does not cross subnets —
    client and rig must share a broadcast domain. `vstimd-client discover --wait 5`
    listens longer on a lossy network.

Two rigs show the same name with a ` #2` suffix
:   Their MACs collide in the low 24 bits. Both are still individually reachable
    via the `id=` TXT record; to fix it properly, [opt out](#opting-out) on one of
    them and assign a name by hand.

The name changed after moving the SD card
:   Expected — the name follows the board's MAC, not the card. See the table
    above.
