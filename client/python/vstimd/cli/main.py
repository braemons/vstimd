"""``vstimd-client`` — command-line interface to a vstimd server."""
from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any, Callable, Sequence

from vstimd._version import __version__
from vstimd.connection import Connection
from vstimd.exceptions import ConfigNotFoundError, VstimdError

from . import discovery
from .address import DEFAULT_ADDRESS, DEFAULT_PORT, AddressError, normalize_address
from .discovery import DiscoveredServer, DiscoveryUnavailableError
from .exit_codes import ExitCode

ADDRESS_ENV = "VSTIMD_ADDRESS"
TRACEBACK_ENV = "VSTIMD_TRACEBACK"

# Commands that block on the server for an unbounded time — no recv timeout.
_BLOCKING_COMMANDS = {"wait-frames", "wait-until", "wait-ready"}

# A continuation row has an empty left column. Built rather than written out so
# the columns cannot drift when a constant below changes length.
_ADDRESS_ROWS: tuple[tuple[str, str], ...] = (
    ("-a, --address tcp://HOST:PORT", "a full endpoint; a bare HOST or HOST:PORT"),
    ("", f"is completed to tcp://HOST:{DEFAULT_PORT}"),
    ("-H, --host NAME [-p PORT]", "a bare NAME gets '.local' appended, so an"),
    ("", "ID from `discover` can be pasted straight in"),
    (f"${ADDRESS_ENV}", "set it once for a whole shell session"),
    (DEFAULT_ADDRESS, "the default"),
)


def _format_address_help() -> str:
    width = max(len(left) for left, _ in _ADDRESS_ROWS)
    lines = ["Choosing a server (first match wins):"]
    lines += [f"  {left:<{width}}  {right}" for left, right in _ADDRESS_ROWS]
    return "\n".join(lines)


_ADDRESS_HELP = _format_address_help()

_EXAMPLES = """\
Examples:
  vstimd-client discover                     find the rigs on this network
  vstimd-client -H vstimd-a1b2c3 info        display properties of one rig
  vstimd-client -a 10.0.1.42 ls              stimuli on the rig at an address
  vstimd-client background 0.5 0.5 0.5       grey out the local server
  vstimd-client -H rig1 wait-ready -w 60     block until a rig has booted
  vstimd-client --json discover | jq -r '.[0].address'\
"""

# Names must match the subparsers in build_parser(); a unit test enforces that.
_COMMAND_GROUPS: tuple[tuple[str, tuple[tuple[str, str], ...]], ...] = (
    (
        "Find a server",
        (("discover", "browse the network for vstimd servers over mDNS"),),
    ),
    (
        "Inspect",
        (
            ("info", "display properties and server version"),
            ("ls", "list the stimuli in the scene"),
        ),
    ),
    (
        "Change the scene",
        (
            ("background", "set the background clear colour (R G B [A], 0-1)"),
            ("delete-all", "remove every unprotected stimulus"),
            ("enable-all", "enable every unprotected stimulus"),
            ("disable-all", "disable every unprotected stimulus"),
        ),
    ),
    (
        "Synchronise",
        (
            ("wait-frames", "block until N more frames are rendered"),
            ("wait-ready", "block until the server answers and has drawn a frame"),
        ),
    ),
    (
        "Manage the server",
        (
            ("config", "list, save, load, get, or upload scene configs"),
            ("shutdown", "ask the server to exit cleanly"),
        ),
    ),
)


def format_overview() -> str:
    """The grouped command listing shown when no command is given.

    ``--help`` lists the same commands flat and adds every option; this is the
    shorter answer to "what can this thing do", which is the actual question
    behind running the bare command.
    """
    lines = [
        f"vstimd-client {__version__} — control a vstimd visual stimulus server.",
        "",
        "Usage:",
        "  vstimd-client [OPTIONS] COMMAND [ARGS...]",
        "",
    ]
    width = max(len(name) for _, commands in _COMMAND_GROUPS for name, _ in commands)
    for title, commands in _COMMAND_GROUPS:
        lines.append(f"{title}:")
        lines += [f"  {name:<{width}}  {help_text}" for name, help_text in commands]
        lines.append("")
    lines += [
        _ADDRESS_HELP,
        "",
        _EXAMPLES,
        "",
        "Run `vstimd-client COMMAND --help` for a command's options,",
        "or `vstimd-client --help` for the global ones.",
    ]
    return "\n".join(lines)


# ── Argument parsing ──────────────────────────────────────────────────────────


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vstimd-client",
        description="Control and inspect a vstimd visual stimulus server.",
        epilog=f"{_ADDRESS_HELP}\n\n{_EXAMPLES}",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    # The client's own version — `info` reports the server's.
    parser.add_argument(
        "-V", "--version", action="version", version=f"vstimd-client {__version__}",
    )
    parser.add_argument(
        "-a",
        "--address",
        help=f"server endpoint, HOST or HOST:PORT or tcp://HOST:PORT "
        f"(default: ${ADDRESS_ENV} or {DEFAULT_ADDRESS})",
    )
    parser.add_argument(
        "-H",
        "--host",
        help="hostname or discovered id (a bare name gets '.local' appended)",
    )
    parser.add_argument(
        "-p", "--port", type=int, default=discovery.DEFAULT_PORT,
        help="port to use when the address carries none (default: %(default)s)",
    )
    parser.add_argument(
        "-t", "--timeout", type=float, default=5.0,
        help="seconds to wait for a server reply, 0 to wait forever (default: %(default)s)",
    )
    parser.add_argument(
        "--json", action="store_true", dest="as_json",
        help="emit machine-readable JSON instead of a human-readable table",
    )

    # Not `required`: a bare `vstimd-client` should reach main() and print the
    # command overview rather than argparse's one-line "COMMAND is required".
    sub = parser.add_subparsers(dest="command", metavar="COMMAND")

    p = sub.add_parser("discover", help="find vstimd servers via mDNS/Avahi")
    p.add_argument(
        "-w", "--wait", type=float, default=2.0,
        help="seconds to listen for mDNS responses (default: %(default)s)",
    )
    p.add_argument(
        "-b", "--backend", choices=["zeroconf", "avahi"],
        help="force a discovery backend (default: first available)",
    )
    p.set_defaults(func=cmd_discover, needs_connection=False)

    p = sub.add_parser("info", help="show display properties and server version")
    p.set_defaults(func=cmd_info)

    p = sub.add_parser("ls", aliases=["list"], help="list the stimuli in the scene")
    p.set_defaults(func=cmd_ls)

    p = sub.add_parser("background", help="set the background clear colour")
    p.add_argument("r", type=float)
    p.add_argument("g", type=float)
    p.add_argument("b", type=float)
    p.add_argument("a", type=float, nargs="?", default=1.0)
    p.set_defaults(func=cmd_background)

    p = sub.add_parser("delete-all", help="remove every (unprotected) stimulus")
    p.set_defaults(func=cmd_delete_all)

    p = sub.add_parser("enable-all", help="enable every (unprotected) stimulus")
    p.set_defaults(func=lambda conn, args: _set_all_enabled(conn, args, True))

    p = sub.add_parser("disable-all", help="disable every (unprotected) stimulus")
    p.set_defaults(func=lambda conn, args: _set_all_enabled(conn, args, False))

    p = sub.add_parser("wait-frames", help="block until N more frames are rendered")
    p.add_argument("count", type=int, nargs="?", default=1)
    p.set_defaults(func=cmd_wait_frames)

    p = sub.add_parser("wait-ready", help="block until the server answers and has rendered a frame")
    p.add_argument(
        "-w", "--wait", type=float, default=30.0,
        help="seconds to keep retrying (default: %(default)s)",
    )
    p.set_defaults(func=cmd_wait_ready)

    p = sub.add_parser("shutdown", help="ask the server to exit cleanly")
    p.add_argument("-y", "--yes", action="store_true", help="skip the confirmation prompt")
    p.set_defaults(func=cmd_shutdown)

    _add_config_parsers(sub)
    return parser


def _add_config_parsers(sub: Any) -> None:
    config = sub.add_parser("config", help="manage saved scene configs on the server")
    csub = config.add_subparsers(dest="config_command", metavar="SUBCOMMAND")
    csub.required = True

    p = csub.add_parser("list", help="list configs in the server's config directory")
    p.set_defaults(func=cmd_config_list)

    p = csub.add_parser("save", help="save the current scene under a name")
    p.add_argument("name")
    p.add_argument("-f", "--overwrite", action="store_true", help="replace an existing config")
    p.set_defaults(func=cmd_config_save)

    p = csub.add_parser("load", help="load and apply a named config")
    p.add_argument("name")
    p.add_argument(
        "--additive", action="store_true",
        help="merge into the current scene instead of clearing it first",
    )
    p.set_defaults(func=cmd_config_load)

    p = csub.add_parser("get", help="print the current scene config as JSON")
    p.add_argument("-o", "--output", help="write to a file instead of stdout")
    p.set_defaults(func=cmd_config_get)

    p = csub.add_parser("upload", help="upload a local config JSON file to the server")
    p.add_argument("name")
    p.add_argument("file", help="path to a config JSON file, or '-' for stdin")
    p.add_argument("-f", "--overwrite", action="store_true", help="replace an existing config")
    p.add_argument("--apply-now", action="store_true", help="apply the config after saving")
    p.add_argument(
        "--additive", action="store_true",
        help="with --apply-now, merge instead of clearing the scene",
    )
    p.set_defaults(func=cmd_config_upload)


# ── Commands ──────────────────────────────────────────────────────────────────


def cmd_discover(args: argparse.Namespace) -> int:
    servers = discovery.discover(args.wait, backend=args.backend)
    if args.as_json:
        _print_json([_server_to_dict(s) for s in servers])
        return ExitCode.OK if servers else ExitCode.NOT_FOUND
    if not servers:
        return _fail(
            f"no vstimd servers found (listened {args.wait:g}s)",
            ExitCode.NOT_FOUND,
            hint="mDNS does not cross subnets — try `--wait 5`, or give the "
            "address directly with -a",
        )
    _print_table(
        ["ID", "HOSTNAME", "ADDRESSES", "ADDRESS"],
        [
            [s.id or "-", s.hostname or "-", ", ".join(s.addresses) or "-", s.address]
            for s in servers
        ],
    )
    return 0


def cmd_info(conn: Connection, args: argparse.Namespace) -> int:
    info = conn.system.query_server_info()
    bg = info.background_color
    if args.as_json:
        _print_json(
            {
                "width": info.width,
                "height": info.height,
                "frame_rate": info.frame_rate,
                "version": str(info.version),
                "background_color": [bg.r, bg.g, bg.b, bg.a],
            }
        )
        return 0
    _print_pairs(
        [
            ("version", str(info.version)),
            ("resolution", f"{info.width}x{info.height}"),
            ("frame rate", f"{info.frame_rate:.2f} Hz"),
            ("background", f"{bg.r:.3f} {bg.g:.3f} {bg.b:.3f} {bg.a:.3f}"),
        ]
    )
    return 0


def cmd_ls(conn: Connection, args: argparse.Namespace) -> int:
    entries = conn.system.list_stimuli()
    if args.as_json:
        _print_json(
            [
                {
                    "handle": int(e.handle),
                    "enabled": e.enabled,
                    "id": e.id,
                    "name": e.name,
                }
                for e in entries
            ]
        )
        return 0
    if not entries:
        print("no stimuli")
        return 0
    _print_table(
        ["HANDLE", "ENABLED", "NAME", "ID"],
        [
            [str(int(e.handle)), "yes" if e.enabled else "no", e.name or "-", e.id or "-"]
            for e in entries
        ],
    )
    return 0


def cmd_background(conn: Connection, args: argparse.Namespace) -> int:
    conn.system.set_background(args.r, args.g, args.b, args.a)
    return _ok(args, f"background set to {args.r} {args.g} {args.b} {args.a}")


def cmd_delete_all(conn: Connection, args: argparse.Namespace) -> int:
    conn.system.delete_all()
    return _ok(args, "all stimuli deleted")


def _set_all_enabled(conn: Connection, args: argparse.Namespace, enabled: bool) -> int:
    conn.system.set_all_enabled(enabled)
    return _ok(args, f"all stimuli {'enabled' if enabled else 'disabled'}")


def cmd_wait_frames(conn: Connection, args: argparse.Namespace) -> int:
    resp = conn.system.wait_for_frames(args.count)
    if args.as_json:
        _print_json({"frame_count": resp.frame_count, "server_time_ns": resp.server_time_ns})
        return 0
    print(f"frame_count={resp.frame_count} server_time_ns={resp.server_time_ns}")
    return 0


def cmd_wait_ready(conn: Connection, args: argparse.Namespace) -> int:
    conn.wait_until_ready(timeout_s=args.wait)
    return _ok(args, f"server ready at {conn.address}")


def cmd_shutdown(conn: Connection, args: argparse.Namespace) -> int:
    if not args.yes:
        if not sys.stdin.isatty():
            return _fail(
                "refusing to prompt on non-interactive stdin; use --yes",
                ExitCode.USAGE,
            )
        answer = input(f"Shut down the vstimd server at {conn.address}? [y/N] ")
        if answer.strip().lower() not in ("y", "yes"):
            return _fail("aborted", ExitCode.FAILURE)
    conn.system.shutdown()
    return _ok(args, "shutdown requested")


def cmd_config_list(conn: Connection, args: argparse.Namespace) -> int:
    names = conn.config.list_configs()
    if args.as_json:
        _print_json(names)
        return 0
    if not names:
        print("no configs")
        return 0
    for name in names:
        print(name)
    return 0


def cmd_config_save(conn: Connection, args: argparse.Namespace) -> int:
    conn.config.save(args.name, overwrite=args.overwrite)
    return _ok(args, f"saved config {args.name!r}")


def cmd_config_load(conn: Connection, args: argparse.Namespace) -> int:
    conn.config.load(args.name, additive=args.additive)
    return _ok(args, f"loaded config {args.name!r}")


def cmd_config_get(conn: Connection, args: argparse.Namespace) -> int:
    text = conn.config.retrieve()
    if args.output:
        with open(args.output, "w", encoding="utf-8") as fh:
            fh.write(text)
        return _ok(args, f"wrote config to {args.output}")
    print(text)
    return 0


def cmd_config_upload(conn: Connection, args: argparse.Namespace) -> int:
    if args.file == "-":
        text = sys.stdin.read()
    else:
        with open(args.file, encoding="utf-8") as fh:
            text = fh.read()
    conn.config.upload(
        args.name,
        text,
        overwrite=args.overwrite,
        apply_now=args.apply_now,
        additive=args.additive,
    )
    return _ok(args, f"uploaded config {args.name!r}")


# ── Output helpers ────────────────────────────────────────────────────────────


def _ok(args: argparse.Namespace, message: str) -> int:
    if args.as_json:
        _print_json({"ok": True, "message": message})
    else:
        print(message)
    return ExitCode.OK


def _fail(message: object, code: ExitCode, *, hint: str | None = None) -> int:
    """Report a failure the way a command-line tool should: one line, no stack.

    Tracebacks are for bugs in the client. Everything a user can cause — a rig
    that is off, a typo in an address, a config that does not exist — is a
    sentence on stderr and a distinct exit code.
    """
    print(f"vstimd-client: {message}", file=sys.stderr)
    if hint:
        print(f"  hint: {hint}", file=sys.stderr)
    return code


def _print_json(payload: object) -> None:
    print(json.dumps(payload, indent=2))


def _print_pairs(pairs: Sequence[tuple[str, str]]) -> None:
    width = max(len(key) for key, _ in pairs)
    for key, value in pairs:
        print(f"{key:<{width}}  {value}")


def _print_table(headers: Sequence[str], rows: Sequence[Sequence[str]]) -> None:
    widths = [
        max(len(headers[i]), *(len(row[i]) for row in rows)) if rows else len(headers[i])
        for i in range(len(headers))
    ]
    line = "  ".join(h.ljust(w) for h, w in zip(headers, widths))
    print(line.rstrip())
    for row in rows:
        print("  ".join(cell.ljust(w) for cell, w in zip(row, widths)).rstrip())


def _server_to_dict(server: DiscoveredServer) -> dict[str, Any]:
    return {
        "id": server.id,
        "name": server.name,
        "hostname": server.hostname,
        "addresses": list(server.addresses),
        "port": server.port,
        "address": server.address,
        "properties": server.properties,
    }


# ── Entry point ───────────────────────────────────────────────────────────────


def resolve_address(args: argparse.Namespace) -> str:
    """Work out which ZMQ endpoint to talk to, from flags then environment.

    Raises :class:`~vstimd.cli.address.AddressError` if what was given cannot
    be completed into an endpoint.
    """
    if args.address:
        return normalize_address(args.address, default_port=args.port)
    if args.host:
        host = args.host
        # A bare name is an id from `discover`, which is an mDNS name.
        if "." not in host and ":" not in host:
            host = f"{host}.local"
        return normalize_address(host, default_port=args.port)
    return normalize_address(
        os.environ.get(ADDRESS_ENV) or DEFAULT_ADDRESS, default_port=args.port
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command is None:
        print(format_overview(), file=sys.stderr)
        return ExitCode.USAGE

    if args.address and args.host:
        parser.error("--address and --host are mutually exclusive")

    func: Callable[..., int] = args.func
    if not getattr(args, "needs_connection", True):
        try:
            return func(args)
        except DiscoveryUnavailableError as exc:
            return _fail(exc, ExitCode.NO_BACKEND)

    try:
        address = resolve_address(args)
    except AddressError as exc:
        return _fail(exc, ExitCode.USAGE)

    timeout_s = args.timeout if args.timeout > 0 else None
    if args.command in _BLOCKING_COMMANDS:
        timeout_s = None

    import zmq  # type: ignore[import]  # imported here so `discover` works without a server

    try:
        conn = Connection(address, recv_timeout_s=timeout_s)
    except zmq.ZMQError as exc:
        return _fail(
            f"cannot open a connection to {address}: {exc}",
            ExitCode.UNAVAILABLE,
            hint="an address looks like tcp://HOST:PORT; run `vstimd-client "
            "discover` to list the rigs on this network",
        )

    try:
        return func(conn, args)
    except zmq.Again:
        # REQ sockets queue silently when nothing is listening, so a dead rig
        # and a wrong address both surface here rather than at connect time.
        return _fail(
            f"no reply from {address} within {args.timeout:g}s",
            ExitCode.TIMEOUT,
            hint="is vstimd running there? `vstimd-client discover` lists the "
            "rigs it can see, and -t raises the timeout",
        )
    except zmq.ZMQError as exc:
        return _fail(f"connection to {address} failed: {exc}", ExitCode.UNAVAILABLE)
    except ConfigNotFoundError as exc:
        return _fail(exc, ExitCode.NOT_FOUND)
    except VstimdError as exc:
        return _fail(exc, ExitCode.SERVER_ERROR)
    except TimeoutError as exc:
        return _fail(exc, ExitCode.TIMEOUT)
    except FileNotFoundError as exc:
        return _fail(f"{exc.filename}: no such file", ExitCode.NOT_FOUND)
    except OSError as exc:
        return _fail(exc, ExitCode.FAILURE)
    finally:
        conn.close()


def run() -> None:
    """Console-script wrapper: run :func:`main` and exit with its status.

    The last line of defence against a traceback reaching the terminal: any
    exception :func:`main` did not expect is a bug in the client, and is
    reported as one, with the traceback available behind an environment
    variable for whoever has to fix it.
    """
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        sys.exit(ExitCode.INTERRUPTED)
    except BrokenPipeError:
        # `vstimd-client ls | head` closes the pipe under us. Redirect stdout to
        # the void so the interpreter's own flush at exit cannot fail as well.
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        sys.exit(ExitCode.FAILURE)
    except Exception as exc:
        if os.environ.get(TRACEBACK_ENV):
            raise
        print(
            f"vstimd-client: unexpected error: {type(exc).__name__}: {exc}",
            file=sys.stderr,
        )
        print(
            f"  hint: this is a bug — set {TRACEBACK_ENV}=1 for the traceback, "
            "then report it at https://github.com/braemons/vstimd/issues",
            file=sys.stderr,
        )
        sys.exit(ExitCode.FAILURE)
