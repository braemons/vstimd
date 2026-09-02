# Command-line client

`vstimd-client` ships with the [Python client](python.md) and covers the
system-level commands from a shell — server info, scene-wide mutations, config
management, shutdown — plus mDNS discovery of the servers on your network.

## Install

```sh
pip install vstimd-client
```

Or without installing anything permanently, `uvx vstimd-client info`.

Discovery works out of the box: [zeroconf](https://pypi.org/project/zeroconf/),
a pure-Python mDNS implementation, is a dependency of the client. If it is ever
missing, discovery falls back to shelling out to `avahi-browse`, which needs
`avahi-utils` and a running `avahi-daemon` (Linux only).

From a source checkout, `cd client/python && uv sync && make proto` puts
`vstimd-client` in `.venv/bin/`.

## Discovering rigs

Each rig advertises `_vstimd._tcp` on port 5555 over mDNS with an
`id=<hostname>` TXT record — see
[Discovery & hostnames](../operations/discovery.md) for how rigs name themselves
and how the advertisement is published.

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
forces one implementation, and the command exits 6 when nothing is found:

```sh
vstimd-client discover --wait 5 --backend avahi
```

## Choosing a server

Every other command talks to a single server, selected in this order:

1. `--address` — an endpoint, or as much of one as you feel like typing
2. `--host NAME [--port N]` — a bare name gets `.local` appended, so an `ID`
   from `discover` can be pasted straight in
3. `$VSTIMD_ADDRESS`
4. whatever mDNS finds, falling back to `tcp://localhost:5555`

```sh
vstimd-client --host vstimd-a1b2c3 info
export VSTIMD_ADDRESS=tcp://vstimd-a1b2c3.local:5555   # or set it once
```

Given none of the first three, the client browses the network for about a
second. One rig found is used and announced; several are listed for you to pick
from; none falls back to `tcp://localhost:5555`. On a bench with a single rig
that removes the address from the command line altogether:

```console
$ vstimd-client info
vstimd-client: using vstimd-a1b2c3 at tcp://vstimd-a1b2c3.local:5555
version     0.4.1
```

!!! warning "Scripts should name their rig"

    The choice is never silent — one candidate is announced, several are
    prompted for — but a script that relies on there being exactly one rig will
    start asking questions the day a second appears. Pass `-a`/`-H` or set
    `$VSTIMD_ADDRESS`, which also skips the browse. `--non-interactive` refuses
    to prompt and exits `2` instead; a non-terminal stdin does the same.

`--address` completes what it is given — a missing scheme becomes `tcp://` and
a missing port becomes `--port` (5555 by default) — so `-a 10.0.1.42`,
`-a 10.0.1.42:5555` and `-a tcp://10.0.1.42:5555` all name the same rig. An
address it cannot complete is rejected with an explanation and exit code 2.

Requests time out after `--timeout` seconds (default 5) and exit 4;
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
| `clear-stimuli` | remove every unprotected stimulus |
| `clear-animations` | remove every animation |
| `clear-all` | remove every animation, then every unprotected stimulus |
| `enable-all` / `disable-all` | toggle every unprotected stimulus |
| `wait-frames [N]` | block until N more frames are rendered |
| `wait-ready` | block until the server answers and has drawn a frame |
| `shutdown` | ask the server to exit cleanly (prompts unless `-y`) |
| `scene-config list` | list the scene-configs on the server (`-p PROJECT` scopes it) |
| `scene-config save NAME` | save the current scene (`-f` overwrites) |
| `scene-config load NAME` | load and apply a scene-config (`--additive` merges) |
| `scene-config get` | print the current scene-config JSON (`-o FILE` writes it) |
| `scene-config upload NAME FILE` | upload a local scene-config (`-` reads stdin) |

Every `NAME` is `[<project>/]<name>`. A **project** is a directory on the device
holding everything one study needs; an unqualified name means the `default`
project, so the everyday case stays one word.

The `demos/*` entries in `scene-config list` are the
[demo scenes](../getting-started/demos.md) the server installs on first start —
ordinary scene-configs, so `scene-config load demos/drifting_grating` is all it
takes to put one on the display. `scene-config list -p demos` shows just those.

## Scripting

`--json` switches any command to machine-readable output:

```sh
# Address of the first rig found
addr=$(vstimd-client --json discover | jq -r '.[0].address')

# Wait for a rig to come up before starting an experiment
vstimd-client --host vstimd-a1b2c3 wait-ready --wait 60

# Back up the running scene, restore it later
vstimd-client scene-config get -o scene.json
vstimd-client scene-config upload restored scene.json --overwrite --apply-now
```

Failures are told apart by exit status, so a start-up script can distinguish a
rig that is switched off from one that refused the request:

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | a failure none of the codes below describes |
| `2` | bad command line, or no command given |
| `3` | the server could not be reached — bad address, nothing listening |
| `4` | the server did not reply within `--timeout` |
| `5` | the server replied with an error |
| `6` | nothing found: no rigs discovered, no such config |
| `7` | `discover` has no mDNS backend available |
| `130` | interrupted with Ctrl-C |

Every failure prints one line on stderr, never a traceback. A traceback would
be a bug — `VSTIMD_TRACEBACK=1` brings back the full one for a bug report.

Running `vstimd-client` with no command lists everything it can do, grouped,
with examples.

Discovery is also importable, so an experiment script can find a rig without
shelling out:

```python
from vstimd import Connection
from vstimd.cli import discover

servers = discover(timeout_s=2.0)
with Connection(servers[0].address) as conn:
    print(conn.system.query_server_info())
```
