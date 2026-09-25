# Command-line client

`vstimctl` ships with the [Python client](python.md) and covers the
system-level commands from a shell: the server's state, scene-wide changes,
the scene-config store, the event stream and shutdown. It also finds the
servers on your network over mDNS.

It is named like the other braemons commands (`statemachinectl`,
`mousewheelctl`, `trialctl`), and it behaves like them. The rules are in
`contracts/DAEMON_LAYOUT.md`: the same `--rig`, JSON output, and the same exit
statuses.

## Install

```sh
pip install vstimd-client
```

Or without installing anything permanently, `uvx --from vstimd-client vstimctl state`.
On a rig, the `braemons-tools` package installs it alongside the other three.

Discovery works out of the box: [zeroconf](https://pypi.org/project/zeroconf/),
a pure-Python mDNS implementation, is a dependency of the client. If it is ever
missing, discovery falls back to shelling out to `avahi-browse`, which needs
`avahi-utils` and a running `avahi-daemon` (Linux only).

From a source checkout, `cd client/python && uv sync && make proto` puts
`vstimctl` in `.venv/bin/`.

## Choosing a rig

Every command except `discover` talks to one server:

1. `--rig HOST`, `--rig HOST:PORT` or `--rig tcp://HOST:PORT`
2. `$BRAEMONS_RIG`
3. `localhost`

A missing port becomes 5555, vstimd's command port. Every braemons command
fills in its own daemon's port, so a single `BRAEMONS_RIG=rig-a.local` reaches
all four daemons on one box. Put a host in it, never a port.

```sh
vstimctl --rig braemons-a1b2c3d4e5f6.local state
export BRAEMONS_RIG=braemons-a1b2c3d4e5f6.local     # or set it once per shell
```

`vstimctl` never browses the network to guess a rig. A command that went to
"whichever rig answered" is a bad surprise when the command is `shutdown`. Use
`discover` to list the rigs, and name one.

Requests time out after `--timeout` seconds (default 5), and `--timeout 0`
blocks forever. The `wait-*` commands and `capture` always block, bounded by
their own `--wait` deadline.

## Discovering rigs

Each rig advertises `_vstimd._tcp` over mDNS. See
[Discovery & hostnames](../operations/discovery.md) for what the record carries.

```console
$ vstimctl discover | jq -r '.[] | "\(.id)  \(.address)"'
3f9c0a7d1e2b4c56  tcp://braemons-a1b2c3d4e5f6.local:5555
8e1d44b0c9a27f13  tcp://braemons-ffee00112233.local:5555
```

`--wait N` listens longer on a lossy network, and `--backend {zeroconf,avahi}`
forces one implementation. The command exits 6 when nothing is found.

## Commands

| Command | Effect |
|---|---|
| `discover` | the servers on this network, as a JSON list (needs no connection) |
| `state` | the display, the server's version, and every stimulus |
| `watch` | the event stream, one JSON object per line (`--topic PREFIX`, repeatable; `--summary`) |
| `background R G B [A]` | set the clear colour, components in 0–1 |
| `clear-stimuli` | remove every unprotected stimulus |
| `clear-animations` | remove every animation |
| `clear-all` | remove every animation, then every unprotected stimulus |
| `enable-all` / `disable-all` | toggle every unprotected stimulus |
| `wait-frames [N]` | block until N more frames are rendered |
| `wait-ready` | block until the server answers and has drawn a frame |
| `capture PATH` | save the next presented frame as a PNG, overlay included |
| `shutdown` | ask the server to exit cleanly (asks first unless `-y`) |
| `scene-configs list` | the stored scene-configs (`-p PROJECT` scopes it) |
| `scene-configs get` | the current scene, as a scene-config file |
| `scene-configs put FILE` | store a file (`--name`, else the file's stem; `-` reads stdin and needs `--name`; `-f` overwrites; `--load` applies it) |
| `scene-configs load NAME` | load and apply a stored scene-config (`--additive` merges) |
| `scene-configs save NAME` | store the current scene (`-f` overwrites) |

Every `NAME` is `[<project>/]<name>`. A **project** is a directory on the device
holding everything one study needs. An unqualified name means the `default`
project, so the everyday case stays one word.

The `demos/*` entries in `scene-configs list` are the
[demo scenes](../getting-started/demos.md) the server installs on first start.
They are ordinary scene-configs, so `scene-configs load demos/drifting_grating`
puts one on the display, and `scene-configs list -p demos` shows only those.

`watch` reads the event port (5556, or `--event-port`). It needs no reply, so a
rig that is switched off gives a quiet stream rather than a failure:

```console
$ vstimctl watch --topic frame.dropped --topic vtl.edge
{"topic": "vtl.edge", "missed_before": 0, "sequence": "912", "frame": "4410", ...}
```

## Scripting

Everything prints JSON on stdout. A command prints one document, and `watch`
prints one object per line:

```sh
# Wait for a rig to come up before starting an experiment
vstimctl wait-ready --wait 60

# How many stimuli are enabled
vstimctl state | jq '[.stimuli[] | select(.enabled)] | length'

# Back up the running scene, restore it later
vstimctl scene-configs get > restored.json
vstimctl scene-configs put restored.json --overwrite --load
```

A failure is one JSON object on stderr, `{"error": …, "detail": …}`, and never
a traceback. `error` is the short kind a script switches on. The exit status
tells a rig that is switched off from one that refused the request. These are
the same numbers every braemons command uses:

| Status | Meaning |
|---|---|
| `0` | success |
| `1` | a failure none of the statuses below describes |
| `2` | bad command line |
| `3` | nothing answered within `--timeout`: a rig that is off, or a wrong address |
| `4` | the server answered, and a call did not finish in time |
| `5` | the server answered with an error |
| `6` | nothing found: no rigs discovered, no such scene-config |
| `130` | interrupted with Ctrl-C |

A traceback would be a bug in the client. `VSTIMCTL_TRACEBACK=1` brings back the
full one for a bug report.

Discovery can also be imported, so an experiment script can find a rig without
shelling out:

```python
from vstimd_client import VstimdClient
from vstimd_client.command_line_interface import discover

servers = discover(timeout_s=2.0)
with VstimdClient(servers[0].address) as conn:
    print(conn.system.query_server_info())
```
