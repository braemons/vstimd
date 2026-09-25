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

The hostname belongs to the rig, not to vstimd. A rig box can run all four
braemons daemons, so the name is set by the **`braemons-rig`** package
([braemons/rig](https://github.com/braemons/rig)), which every daemon's package
leaves alone. Its `braemons-hostname.service` runs at boot and sets the hostname
to:

```
braemons-XXXXXX
```

where `XXXXXX` is the **last 6 hex characters of the primary interface's MAC
address**, lower-cased. For example, a NIC at `d8:3a:dd:a1:b2:c3` gives
`braemons-a1b2c3`.

| Property | Consequence |
|---|---|
| Derived from the MAC | Stable across reboots *and* re-flashes, with no stored state |
| 15 characters | Fits Samba's 15-character NetBIOS limit, so the SMB name matches the DNS name |
| Follows the board, not the card | Moving an SD card to another Pi renames the rig; replacing a board or NIC gives it a new name |
| Collision-resistant, not collision-proof | Two rigs clash only if the low 24 bits of their MACs match |

The [Raspberry Pi image](raspberry-pi-image.md) installs `braemons-rig` from the
archive. On any other box, install it yourself:

```bash
sudo apt install braemons-rig
journalctl -u braemons-hostname     # what it picked, and from which interface
```

To name a rig by hand instead, disable the unit and set the name:

```bash
sudo systemctl disable braemons-hostname
sudo hostnamectl set-hostname my-rig-name          # keep it ≤ 15 chars for SMB
```

Nothing else needs to change. vstimd advertises whatever the hostname is.

---

## mDNS advertisement

vstimd advertises itself while it runs, from inside the server, the way every
braemons daemon does. The record appears when the server starts, lists the
ports it actually bound, and is withdrawn when it stops. Nothing on disk
describes it, so there is no file to keep in step with the rig config.

| Field | Value |
|---|---|
| Service type | `_vstimd._tcp` |
| Port | the web port (8080 by default): the panels a braemons console loads. With `--no-web`, the ZMQ command port |
| Instance name | the hostname |
| TXT records | `id=` — this server on this box, `sha256("vstimd:" + machine-id)`, 16 hex digits; `rig=` — the box, `sha256("braemons:" + machine-id)`, the same value every braemons daemon on it advertises; `version=`; `port=`; `elements=/elements/vstimd.js` (only with the web surface on); `zmq_port=` and `event_port=` — the command and event sockets, which `vstimctl discover` dials |

`--no-mdns` turns the advertisement off, for a test run or a second server on a
desk. Neither `avahi-daemon` nor any service file is needed for the record. Avahi
on the rig (a Recommends of `braemons-rig`) is still what answers for the
box's own `braemons-XXXXXX.local` name, for `ssh` and the browser.

!!! warning "Match on the TXT record, not the instance name"
    When two services advertise the same name, mDNS appends ` (2)` or ` #2` to
    the **instance name**. `id=` and `rig=` are derived from the machine-id,
    so they are unaffected. A script that needs to reach *one specific rig*
    must key off `id=`, and a console groups a rig's daemons by `rig=`.

---

## Finding rigs from a client

```console
$ vstimctl discover | jq -r '.[] | "\(.hostname)  \(.address)"'
braemons-a1b2c3.local  tcp://braemons-a1b2c3.local:5555
braemons-ffee00.local  tcp://braemons-ffee00.local:5555
```

`vstimctl` ships with the [Python client](../client/cli.md); `discover` is
also importable, so an experiment script can resolve a rig without shelling out.
Without any vstimd tooling installed, plain Avahi works too:

```bash
avahi-browse -r _vstimd._tcp
```

A hostname from that listing is what `--rig` takes, so
`vstimctl --rig braemons-a1b2c3.local state` connects straight away.

`.local` resolution is built in on macOS (Bonjour), on Windows 10 1703 and
later, and on Linux with `nss-mdns` or `systemd-resolved` installed. On a
network where `.local` is hijacked by a corporate search domain, use the IP
address from `discover` instead.

### On Windows, without the Python client

Once you know a rig's name you need nothing at all — Windows resolves `.local`
itself, so `ssh braemons-a1b2c3.local`, `http://braemons-a1b2c3.local:8080` and
`\\braemons-a1b2c3` all work on a stock machine. Discovery only matters when you
do not know the name yet.

For that, use `dns-sd.exe`, which comes with Apple's Bonjour (bundled with
iTunes, or standalone as *Bonjour Print Services for Windows*):

```powershell
dns-sd -B _vstimd._tcp                      # browse; the instance name is the hostname
dns-sd -L braemons-a1b2c3 _vstimd._tcp local  # → host:port and the id= TXT record
dns-sd -G v4 braemons-a1b2c3.local            # → IP address
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
| 5556 | TCP | ZMQ event stream (PUB) |
| 5353 | UDP | mDNS (the server's own responder, and Avahi if installed) |
| 22 | TCP | SSH, if `openssh-server` is installed |
| 139, 445 | TCP | Samba — **only** on a rig where you installed it (see below) |

!!! info "The packages do not install Samba"
    No braemons package pulls in Samba: on a rig installed from a
    `.deb`/`.rpm` or from source there is no SMB server at all. Nothing appears
    in Windows Explorer and `nbtstat -A <ip>` reports *Host not found* — that is
    expected, not a discovery failure.

    The share *definitions* do ship, in `braemons-rig`, at
    `/usr/share/braemons/rig/braemons-shares.conf`, so turning them on is
    `apt install samba` plus one `include =` line — see
    [Manual appliance setup → Admin access](appliance-setup.md#6-admin-access-ssh-optional-samba).
    The [Raspberry Pi image](raspberry-pi-image.md) has it done already.

If the rig runs a firewall, open at least 5555 and 8080:
`sudo ufw allow 5555/tcp && sudo ufw allow 8080/tcp`.

---

## Troubleshooting

The rig is still called `raspberrypi` / `ubuntu`
:   `braemons-rig` is not installed, or its unit could not find an interface.
    `systemctl status braemons-hostname` and `journalctl -u braemons-hostname`
    say which. A rig with no NIC at all (Wi-Fi-only, adapter unplugged at boot)
    fails after a 20 s wait.

`discover` finds nothing
:   Check that vstimd is running, and that its log says `mDNS: advertising`
    rather than `not advertising`. mDNS does not cross subnets:
    client and rig must share a broadcast domain. `vstimctl discover --wait 5`
    listens longer on a lossy network.

Two rigs show the same name with a ` #2` suffix
:   Their MACs collide in the low 24 bits. Both are still individually reachable
    through the `id=` TXT record. To fix it properly, disable
    `braemons-hostname` on one of them and assign a name by hand.

The name changed after moving the SD card
:   Expected — the name follows the board's MAC, not the card. See the table
    above.
