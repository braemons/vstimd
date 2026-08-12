# Command-line client

`vstimd-client` ships with the [Python client](python.md) and covers the
system-level commands from a shell — server info, scene-wide mutations, config
management, shutdown — plus mDNS discovery of the servers on your network.

## Install

```sh
pip install 'vstimd[discover]'
```

The `[discover]` extra pulls in [zeroconf](https://pypi.org/project/zeroconf/),
a pure-Python mDNS implementation. Without it, discovery falls back to shelling
out to `avahi-browse`, which needs `avahi-utils` and a running `avahi-daemon`
(Linux only). Everything else works with the base install.

From a source checkout, `cd client/python && uv sync` puts `vstimd-client` in
`.venv/bin/`.

## Discovering rigs

Each rig advertises `_vstimd._tcp` on port 5555 over mDNS with an
`id=<hostname>` TXT record — see
[Deployment](../operations/deployment.md) for how the advertisement is
published.

```console
$ vstimd-client discover
ID             HOSTNAME             ADDRESSES   ADDRESS
vstimd-a1b2c3  vstimd-a1b2c3.local  10.0.1.42   tcp://vstimd-a1b2c3.local:5555
vstimd-ffee00  vstimd-ffee00.local  10.0.1.51   tcp://vstimd-ffee00.local:5555
```

!!! tip "Match on the ID, not the name"

    When two rigs advertise the same name, Avahi suffixes the *display name*
    with ` #2`, ` #3`. The `id=` TXT record is a literal written at boot by
    `vstimd-set-hostname` and is never rewritten, so it is the identity to key
    off in scripts.

`--wait N` listens longer on a lossy network, `--backend {zeroconf,avahi}`
forces one implementation, and the command exits 1 when nothing is found:

```sh
vstimd-client discover --wait 5 --backend avahi
```

## Choosing a server

Every other command talks to a single server, selected in this order:

1. `--address tcp://host:port` — a full ZMQ endpoint
2. `--host NAME [--port N]` — a bare name gets `.local` appended, so an `ID`
   from `discover` can be pasted straight in
3. `$VSTIMD_ADDRESS`
4. `tcp://localhost:5555`

```sh
vstimd-client --host vstimd-a1b2c3 info
export VSTIMD_ADDRESS=tcp://vstimd-a1b2c3.local:5555   # or set it once
```

Requests time out after `--timeout` seconds (default 5) and exit 1;
`--timeout 0` blocks forever. The `wait-*` commands always block, bounded by
their own `--wait` deadline.

## Commands

```console
$ vstimd-client info
version     0.4.1
resolution  1920x1080
frame rate  60.00 Hz
background  0.000 0.000 0.000 1.000

$ vstimd-client ls
HANDLE  ENABLED  NAME       ID
1       yes      fixation   0d0f2d0e-…
2       no       target     6b1a7c44-…
```

| Command | Effect |
|---|---|
| `discover` | browse the network for servers (needs no connection) |
| `info` | display properties and server version |
| `ls` (`list`) | list the stimuli in the scene |
| `background R G B [A]` | set the clear colour, components in 0–1 |
| `delete-all` | remove every unprotected stimulus |
| `enable-all` / `disable-all` | toggle every unprotected stimulus |
| `wait-frames [N]` | block until N more frames are rendered |
| `wait-ready` | block until the server answers and has drawn a frame |
| `shutdown` | ask the server to exit cleanly (prompts unless `-y`) |
| `config list` | list configs in the server's config directory |
| `config save NAME` | save the current scene (`-f` overwrites) |
| `config load NAME` | load and apply a config (`--additive` merges) |
| `config get` | print the current config JSON (`-o FILE` writes it) |
| `config upload NAME FILE` | upload a local config (`-` reads stdin) |

## Scripting

`--json` switches any command to machine-readable output:

```sh
# Address of the first rig found
addr=$(vstimd-client --json discover | jq -r '.[0].address')

# Wait for a rig to come up before starting an experiment
vstimd-client --host vstimd-a1b2c3 wait-ready --wait 60

# Back up the running scene, restore it later
vstimd-client config get -o scene.json
vstimd-client config upload restored scene.json --overwrite --apply-now
```

Exit codes: `0` success, `1` server error / timeout / nothing discovered,
`2` no mDNS backend available, `130` interrupted.

Discovery is also importable, so an experiment script can find a rig without
shelling out:

```python
from vstimd import Connection
from vstimd.cli import discover

servers = discover(timeout_s=2.0)
with Connection(servers[0].address) as conn:
    print(conn.system.query_server_info())
```
