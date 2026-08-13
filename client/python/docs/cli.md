# `vstimd-client` command-line tool

Installing the Python client puts a `vstimd-client` executable on your `PATH`.
It covers the system-level commands — server info, scene-wide mutations, config
management, shutdown — plus mDNS discovery of servers on the local network.

```bash
pip install vstimd-client
vstimd-client --help
```

It can also be run as a module: `python -m vstimd.cli`.

## Finding servers

vstimd advertises itself over mDNS/DNS-SD as `_vstimd._tcp` on port 5555, with
an `id=<hostname>` TXT record:

```console
$ vstimd-client discover
ID             HOSTNAME             ADDRESSES    ADDRESS
vstimd-a1b2c3  vstimd-a1b2c3.local  10.0.1.42    tcp://vstimd-a1b2c3.local:5555
vstimd-ffee00  vstimd-ffee00.local  10.0.1.51    tcp://vstimd-ffee00.local:5555
```

Key off the `ID` column, not the advertised service name: Avahi appends `#2`,
`#3` to the display name when two devices collide, while the TXT record stays
the literal hostname.

Two backends are supported, in this order of preference:

| Backend    | Requirement                                | Platforms |
| ---------- | ------------------------------------------ | --------- |
| `zeroconf` | ships with `vstimd-client`                 | any       |
| `avahi`    | `avahi-browse` + a running `avahi-daemon`  | Linux     |

Force one with `--backend`, and give a slow or lossy network more time with
`--wait`:

```bash
vstimd-client discover --backend avahi --wait 5
```

`discover` exits `6` if no servers are found, so it composes in scripts. With
`--json` it emits the full record, TXT properties included.

## Choosing a server

Every other command talks to one server, selected in this order:

1. `--address` — an endpoint, or the part of one you can be bothered to type
2. `--host NAME [--port N]` — a bare name gets `.local` appended, so the `ID`
   from `discover` works directly: `--host vstimd-a1b2c3`
3. `$VSTIMD_ADDRESS`
4. whatever mDNS finds (see below)

### Finding the server for you

Given none of the first three, the client browses for about a second and:

- **one rig found** — uses it, and says so on stderr;
- **several found** — lists them and asks which;
- **none found** — falls back to `tcp://localhost:5555`, as before.

```console
$ vstimd-client info
vstimd-client: using vstimd-a1b2c3 at tcp://vstimd-a1b2c3.local:5555
version     0.4.1
resolution  1920x1080
...

$ vstimd-client info
vstimd-client: 2 vstimd servers found, and no address given
  1  vstimd-a1b2c3  tcp://vstimd-a1b2c3.local:5555
  2  vstimd-ffee00  tcp://vstimd-ffee00.local:5555
Select a server [1-2, q to cancel]:
```

The prompt and the menu go to stderr, so `--json` output on stdout stays
parseable however the server was chosen.

Which rig ran a command is not something to be vague about, so the choice is
never made silently: with one candidate it is announced, with several you are
asked. `--non-interactive` refuses to ask — it lists the candidates and exits
`2` — and so does a non-terminal stdin, which is what a cron job or a CI step
has. Naming the server with `-a`, `-H`, or `$VSTIMD_ADDRESS` skips the browse
entirely, which is what a script should do anyway.

`--address` fills in what you leave out, so these are the same server:

```bash
vstimd-client -a 10.0.1.42 info
vstimd-client -a 10.0.1.42:5555 info
vstimd-client -a tcp://10.0.1.42:5555 info
```

The scheme defaults to `tcp://` and the port to `--port` (5555 unless you say
otherwise). IPv6 literals may be written bare (`::1`) or bracketed
(`[::1]:5555`); `ipc://` and `inproc://` endpoints are passed through as
written.

```bash
export VSTIMD_ADDRESS=tcp://vstimd-a1b2c3.local:5555
vstimd-client info
```

Requests give up after `--timeout` seconds (default 5) and exit `4`; pass
`--timeout 0` to block forever. The `wait-*` commands always block, using
their own `--wait` deadline instead.

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
| ------- | ------ |
| `discover` | browse the network for servers (no connection needed) |
| `info` | display properties and server version |
| `ls` (`list`) | list the stimuli in the scene |
| `background R G B [A]` | set the clear colour, components in 0–1 |
| `delete-all` | remove every unprotected stimulus |
| `enable-all` / `disable-all` | toggle every unprotected stimulus |
| `wait-frames [N]` | block until N more frames are rendered |
| `wait-ready` | block until the server answers and has drawn a frame |
| `shutdown` | ask the server to exit cleanly (prompts unless `-y`) |
| `config list` | list saved configs in the server's config directory |
| `config save NAME` | save the current scene (`-f` to overwrite) |
| `config load NAME` | load and apply a config (`--additive` to merge) |
| `config get` | print the current config JSON (`-o FILE` to write it) |
| `config upload NAME FILE` | upload a local config (`-` reads stdin) |

`wait-ready` is the one to use in start-up scripts — it retries the connection
until the server is up, so it doubles as "is this machine ready?":

```bash
vstimd-client --host vstimd-a1b2c3 wait-ready --wait 60
```

## Scripting

`--json` switches every command to machine-readable output, which pairs well
with `jq`:

```bash
# Address of the first server found
vstimd-client --json discover | jq -r '.[0].address'

# Back up the running scene, then restore it later
vstimd-client config get -o scene.json
vstimd-client config upload restored scene.json --overwrite --apply-now
```

### Exit codes

Failures are distinguished by exit status, so a script can tell "the rig is
off" from "the rig said no" without parsing stderr:

| Code | Meaning |
| ---- | ------- |
| `0` | success |
| `1` | a failure none of the codes below describes |
| `2` | bad command line, or no command given |
| `3` | the server could not be reached — bad address, nothing listening |
| `4` | the server did not reply within `--timeout` |
| `5` | the server replied with an error |
| `6` | nothing found: no rigs discovered, no such config |
| `7` | `discover` has no mDNS backend available |
| `130` | interrupted with Ctrl-C |

```bash
if ! vstimd-client -H rig1 info > /dev/null; then
    case $? in
        3|4) echo "rig1 is not answering — is it powered on?" ;;
        5)   echo "rig1 rejected the request" ;;
    esac
fi
```

Errors are reported as a single line on stderr, never a traceback. If you do
see one, it is a bug: set `VSTIMD_TRACEBACK=1` to get the full traceback and
[open an issue](https://github.com/braemons/vstimd/issues).

## Discovery from Python

The discovery machinery is importable, so experiment scripts can find a server
without shelling out:

```python
from vstimd import Connection
from vstimd.cli import discover

servers = discover(timeout_s=2.0)
with Connection(servers[0].address) as conn:
    print(conn.system.query_server_info())
```

```{eval-rst}
.. autofunction:: vstimd.cli.discover

.. autoclass:: vstimd.cli.DiscoveredServer
   :members:

.. autofunction:: vstimd.cli.available_backends
```
