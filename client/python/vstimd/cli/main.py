"""``vstimd-client`` — command-line interface to a vstimd server."""
from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any, Callable, Sequence

from vstimd.connection import Connection
from vstimd.exceptions import VstimdError

from . import discovery
from .discovery import DiscoveredServer, DiscoveryUnavailableError

DEFAULT_ADDRESS = "tcp://localhost:5555"
ADDRESS_ENV = "VSTIMD_ADDRESS"

# Commands that block on the server for an unbounded time — no recv timeout.
_BLOCKING_COMMANDS = {"wait-frames", "wait-until", "wait-ready"}


# ── Argument parsing ──────────────────────────────────────────────────────────


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vstimd-client",
        description="Control and inspect a vstimd visual stimulus server.",
        epilog=(
            f"The server address defaults to ${ADDRESS_ENV} if set, "
            f"otherwise {DEFAULT_ADDRESS}. Use `vstimd-client discover` to find "
            "servers on the local network."
        ),
    )
    parser.add_argument(
        "-a",
        "--address",
        help=f"ZMQ endpoint of the server (default: ${ADDRESS_ENV} or {DEFAULT_ADDRESS})",
    )
    parser.add_argument(
        "-H",
        "--host",
        help="hostname or discovered id (a bare name gets '.local' appended)",
    )
    parser.add_argument(
        "-p", "--port", type=int, default=discovery.DEFAULT_PORT,
        help="port to use with --host (default: %(default)s)",
    )
    parser.add_argument(
        "-t", "--timeout", type=float, default=5.0,
        help="seconds to wait for a server reply, 0 to wait forever (default: %(default)s)",
    )
    parser.add_argument(
        "--json", action="store_true", dest="as_json",
        help="emit machine-readable JSON instead of a human-readable table",
    )

    sub = parser.add_subparsers(dest="command", metavar="COMMAND")
    sub.required = True

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
        return 0 if servers else 1
    if not servers:
        print(
            f"No vstimd servers found (listened {args.wait:g}s).",
            file=sys.stderr,
        )
        return 1
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
    if not args.yes and sys.stdin.isatty():
        answer = input(f"Shut down the vstimd server at {conn.address}? [y/N] ")
        if answer.strip().lower() not in ("y", "yes"):
            print("aborted", file=sys.stderr)
            return 1
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
    return 0


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
    """Work out which ZMQ endpoint to talk to, from flags then environment."""
    if args.address:
        return args.address
    if args.host:
        host = args.host
        if "." not in host and ":" not in host:
            host = f"{host}.local"
        return f"tcp://{host}:{args.port}"
    return os.environ.get(ADDRESS_ENV) or DEFAULT_ADDRESS


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.address and args.host:
        parser.error("--address and --host are mutually exclusive")

    func: Callable[..., int] = args.func
    if not getattr(args, "needs_connection", True):
        try:
            return func(args)
        except DiscoveryUnavailableError as exc:
            print(f"vstimd-client: {exc}", file=sys.stderr)
            return 2

    address = resolve_address(args)
    timeout_s = args.timeout if args.timeout > 0 else None
    if args.command in _BLOCKING_COMMANDS:
        timeout_s = None

    import zmq  # type: ignore[import]  # imported here so `discover` works without a server

    conn = Connection(address, recv_timeout_s=timeout_s)
    try:
        return func(conn, args)
    except zmq.Again:
        print(
            f"vstimd-client: no reply from {address} within {args.timeout:g}s "
            "— is the server running?",
            file=sys.stderr,
        )
        return 1
    except VstimdError as exc:
        print(f"vstimd-client: {exc}", file=sys.stderr)
        return 1
    except TimeoutError as exc:
        print(f"vstimd-client: {exc}", file=sys.stderr)
        return 1
    except OSError as exc:
        print(f"vstimd-client: {exc}", file=sys.stderr)
        return 1
    finally:
        conn.close()


def run() -> None:
    """Console-script wrapper: run :func:`main` and exit with its status."""
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
