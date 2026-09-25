"""``vstimctl`` — one rig's stimulus server, from a terminal.

Named without the ``d``: the server is ``vstimd``, and it installs a program of
that name on every rig this would also be installed on.

It follows the family's rules for a ``<name>ctl`` (``contracts/DAEMON_LAYOUT.md``):
``--rig``, then ``$BRAEMONS_RIG``, then localhost; JSON on stdout, one compact
object per line for a stream; a failure as one JSON object on stderr, with an
exit status a script can switch on. It never browses the network to guess a
rig — ``vstimctl discover`` lists them, and a person picks one.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Any

from vstimd_client._version import __version__
from vstimd_client.exceptions import SceneConfigNotFoundError, VstimdError
from vstimd_client.vstimd_client import VstimdClient

from . import discovery
from .address import DEFAULT_PORT, AddressError, normalize_address
from .discovery import DiscoveredServer, DiscoveryUnavailableError
from .exit_status import ExitStatus

RIG_ENVIRONMENT_VARIABLE = "BRAEMONS_RIG"
TRACEBACK_ENVIRONMENT_VARIABLE = "VSTIMCTL_TRACEBACK"

# Commands that block on the server for an unbounded time — no recv timeout.
_BLOCKING_COMMANDS = {"wait-frames", "wait-ready", "capture"}

_EXAMPLES = f"""\
Examples:
  vstimctl discover                          find the rigs on this network
  vstimctl --rig rig-a.local state           what one rig is showing
  export {RIG_ENVIRONMENT_VARIABLE}=rig-a.local            ...or name it once per shell
  vstimctl wait-ready --wait 60              block until the rig has booted
  vstimctl watch --topic frame.dropped       follow dropped frames, one per line
  vstimctl scene-configs get > scene.json    the current scene, as a file"""


# ── Argument parsing ──────────────────────────────────────────────────────────


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vstimctl",
        description="Control and inspect a vstimd visual stimulus server. "
        "Everything prints JSON, so it pipes into jq.",
        epilog=_EXAMPLES,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    # The client's own version — `state` reports the server's.
    parser.add_argument("-V", "--version", action="version", version=f"vstimctl {__version__}")
    parser.add_argument(
        "--rig",
        default=os.environ.get(RIG_ENVIRONMENT_VARIABLE) or "localhost",
        help=f"HOST, HOST:PORT or tcp://HOST:PORT (default ${RIG_ENVIRONMENT_VARIABLE}, "
        f"then localhost; port {DEFAULT_PORT})",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=5.0,
        metavar="SECONDS",
        help="how long to wait for the server to answer, 0 to wait forever "
        "(default %(default)s)",
    )

    sub = parser.add_subparsers(dest="command", metavar="COMMAND", required=True)

    p = sub.add_parser("discover", help="find vstimd servers on this network over mDNS")
    p.add_argument(
        "-w", "--wait", type=float, default=2.0,
        help="seconds to listen for mDNS responses (default %(default)s)",
    )
    p.add_argument(
        "-b", "--backend", choices=["zeroconf", "avahi"],
        help="force a discovery backend (default: first available)",
    )
    p.set_defaults(func=cmd_discover, needs_connection=False)

    p = sub.add_parser("state", help="the display, the server's version, and every stimulus")
    p.set_defaults(func=cmd_state)

    p = sub.add_parser("watch", help="follow the event stream, one object per line")
    p.add_argument(
        "--topic", action="append", default=[],
        help="a topic prefix such as frame.dropped or vtl.edge; repeatable "
        "(default: everything)",
    )
    p.add_argument(
        "--event-port", type=int, default=DEFAULT_PORT + 1,
        help="the server's event port (default %(default)s)",
    )
    p.add_argument("--summary", action="store_true", help="one line of text per event")
    p.set_defaults(func=cmd_watch, needs_connection=False)

    p = sub.add_parser("background", help="set the background clear colour (R G B [A], 0-1)")
    p.add_argument("r", type=float)
    p.add_argument("g", type=float)
    p.add_argument("b", type=float)
    p.add_argument("a", type=float, nargs="?", default=1.0)
    p.set_defaults(func=cmd_background)

    p = sub.add_parser("clear-stimuli", help="remove every unprotected stimulus")
    p.set_defaults(func=cmd_clear_stimuli)

    p = sub.add_parser("clear-animations", help="remove every animation")
    p.set_defaults(func=cmd_clear_animations)

    p = sub.add_parser("clear-all", help="remove every animation and unprotected stimulus")
    p.set_defaults(func=cmd_clear_all)

    p = sub.add_parser("enable-all", help="enable every unprotected stimulus")
    p.set_defaults(func=lambda conn, args: _set_all_enabled(conn, args, True))

    p = sub.add_parser("disable-all", help="disable every unprotected stimulus")
    p.set_defaults(func=lambda conn, args: _set_all_enabled(conn, args, False))

    p = sub.add_parser("wait-frames", help="block until N more frames are rendered")
    p.add_argument("count", type=int, nargs="?", default=1)
    p.set_defaults(func=cmd_wait_frames)

    p = sub.add_parser("wait-ready", help="block until the server answers and has drawn a frame")
    p.add_argument(
        "-w", "--wait", type=float, default=30.0,
        help="seconds to keep retrying (default %(default)s)",
    )
    p.set_defaults(func=cmd_wait_ready)

    p = sub.add_parser("capture", help="save the next presented frame as a PNG")
    p.add_argument("path", help="where to write the PNG")
    p.set_defaults(func=cmd_capture)

    p = sub.add_parser("shutdown", help="ask the server to exit cleanly")
    p.add_argument("-y", "--yes", action="store_true", help="skip the confirmation prompt")
    p.set_defaults(func=cmd_shutdown)

    _add_scene_config_parsers(sub)
    return parser


# Every name argument below is `[<project>/]<name>`: an unqualified name means
# the `default` project, so the everyday case stays one word.
_NAME_HELP = "[<project>/]<name>; unqualified means the 'default' project"


def _add_scene_config_parsers(sub: Any) -> None:
    scene_configs = sub.add_parser("scene-configs", help="the scene-config store")
    csub = scene_configs.add_subparsers(dest="action", metavar="ACTION", required=True)

    p = csub.add_parser("list", help="the scene-configs on the server")
    p.add_argument(
        "-p", "--project", default="",
        help="only this project's scene-configs (default: every project)",
    )
    p.set_defaults(func=cmd_scene_configs_list)

    p = csub.add_parser("get", help="the current scene, as a scene-config file")
    p.set_defaults(func=cmd_scene_configs_get)

    p = csub.add_parser("put", help="store a scene-config file")
    p.add_argument("file", help="a scene-config JSON file, or - for stdin")
    p.add_argument("--name", help=f"default: the file's stem. {_NAME_HELP}")
    p.add_argument("-f", "--overwrite", action="store_true", help="replace one of that name")
    p.add_argument("--load", action="store_true", help="load it once it is stored")
    p.add_argument(
        "--additive", action="store_true",
        help="with --load, merge into the current scene instead of clearing it",
    )
    p.set_defaults(func=cmd_scene_configs_put)

    p = csub.add_parser("load", help="load a stored scene-config and apply it")
    p.add_argument("name", help=_NAME_HELP)
    p.add_argument(
        "--additive", action="store_true",
        help="merge into the current scene instead of clearing it first",
    )
    p.set_defaults(func=cmd_scene_configs_load)

    p = csub.add_parser("save", help="store the current scene under a name")
    p.add_argument("name", help=_NAME_HELP)
    p.add_argument("-f", "--overwrite", action="store_true", help="replace one of that name")
    p.set_defaults(func=cmd_scene_configs_save)


# ── Commands ──────────────────────────────────────────────────────────────────


def cmd_discover(args: argparse.Namespace) -> int:
    servers = discovery.discover(args.wait, backend=args.backend)
    if not servers:
        return _fail(
            "not_found",
            f"no vstimd servers found in {args.wait:g} s; mDNS does not cross subnets, "
            "so try --wait 5, or name the rig with --rig",
            ExitStatus.NOT_FOUND,
        )
    _print([_server_to_dict(server) for server in servers])
    return ExitStatus.OK


def cmd_state(conn: VstimdClient, args: argparse.Namespace) -> int:
    info = conn.system.query_server_info()
    background = info.background_color
    _print(
        {
            "version": str(info.version),
            "width_px": info.width_px,
            "height_px": info.height_px,
            "frame_rate_hz": info.frame_rate_hz,
            "background_color": [background.r, background.g, background.b, background.a],
            "stimuli": [
                {"handle": int(e.handle), "enabled": e.enabled, "id": e.id, "name": e.name}
                for e in conn.system.list_stimuli()
            ],
        }
    )
    return ExitStatus.OK


def cmd_watch(args: argparse.Namespace) -> int:
    """Follow the PUB socket until Ctrl-C.

    No connection first: a SUB socket has no reply to wait for, so a rig that
    is off is a stream that stays quiet rather than a failure.
    """
    from google.protobuf.json_format import MessageToDict

    from vstimd_client.events import EventSubscriber, Topic

    host = _host_of(normalize_address(args.rig))
    topics = tuple(args.topic) or Topic.ALL
    with EventSubscriber(host, args.event_port, topic=topics) as events:
        for event in events:
            if args.summary:
                gap = f"  ({event.missed_before} missed before)" if event.missed_before else ""
                print(f"frame {event.frame:>8}  {event.topic}{gap}", flush=True)
                continue
            _print_line(
                {
                    "topic": event.topic,
                    "missed_before": event.missed_before,
                    **MessageToDict(event.message, preserving_proto_field_name=True),
                }
            )
    return ExitStatus.OK


def cmd_background(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.system.set_background(args.r, args.g, args.b, args.a)
    return _ok(f"background set to {args.r} {args.g} {args.b} {args.a}")


def cmd_clear_stimuli(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.system.clear_stimuli()
    return _ok("all stimuli cleared")


def cmd_clear_animations(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.system.clear_animations()
    return _ok("all animations cleared")


def cmd_clear_all(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.system.clear_all()
    return _ok("scene cleared")


def _set_all_enabled(conn: VstimdClient, args: argparse.Namespace, enabled: bool) -> int:
    conn.system.set_all_enabled(enabled)
    return _ok(f"all stimuli {'enabled' if enabled else 'disabled'}")


def cmd_wait_frames(conn: VstimdClient, args: argparse.Namespace) -> int:
    response = conn.system.wait_for_frames(args.count)
    _print({"frame_count": response.frame_count, "server_time_ns": response.server_time_ns})
    return ExitStatus.OK


def cmd_wait_ready(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.wait_until_ready(timeout_s=args.wait)
    return _ok(f"server ready at {conn.address}")


def cmd_capture(conn: VstimdClient, args: argparse.Namespace) -> int:
    frame = conn.system.capture_frame()
    frame.save(args.path)
    _print(
        {
            "path": args.path,
            "frame": frame.frame,
            "width_px": frame.width_px,
            "height_px": frame.height_px,
        }
    )
    return ExitStatus.OK


def cmd_shutdown(conn: VstimdClient, args: argparse.Namespace) -> int:
    if not args.yes:
        if not sys.stdin.isatty():
            return _fail(
                "usage", "refusing to ask on a non-interactive stdin; pass --yes", ExitStatus.USAGE
            )
        answer = input(f"Shut down the vstimd server at {conn.address}? [y/N] ")
        if answer.strip().lower() not in ("y", "yes"):
            return _fail("aborted", "not shut down", ExitStatus.FAILURE)
    conn.system.shutdown()
    return _ok("shutdown requested")


def cmd_scene_configs_list(conn: VstimdClient, args: argparse.Namespace) -> int:
    _print(conn.scene_config.list_scene_configs(project=args.project))
    return ExitStatus.OK


def cmd_scene_configs_get(conn: VstimdClient, args: argparse.Namespace) -> int:
    # The text as the server wrote it, not a re-encoding: `get > file` and
    # `put file` have to round-trip.
    text = conn.scene_config.retrieve()
    print(text, end="" if text.endswith("\n") else "\n")
    return ExitStatus.OK


def cmd_scene_configs_put(conn: VstimdClient, args: argparse.Namespace) -> int:
    if args.file == "-":
        name, text = args.name, sys.stdin.read()
    else:
        path = Path(args.file)
        # `scene.config.json` is the store's own spelling, and its stem is `scene`.
        name = args.name or path.name.removesuffix(".json").removesuffix(".config")
        text = path.read_text(encoding="utf-8")
    conn.scene_config.upload(
        name, text, overwrite=args.overwrite, apply_now=args.load, additive=args.additive
    )
    return _ok(f"stored scene-config {name!r}")


def cmd_scene_configs_load(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.scene_config.load(args.name, additive=args.additive)
    return _ok(f"loaded scene-config {args.name!r}")


def cmd_scene_configs_save(conn: VstimdClient, args: argparse.Namespace) -> int:
    conn.scene_config.save(args.name, overwrite=args.overwrite)
    return _ok(f"saved the current scene as {args.name!r}")


# ── Output ────────────────────────────────────────────────────────────────────


def _print(payload: object) -> None:
    print(json.dumps(payload, indent=2))


def _print_line(payload: object) -> None:
    """One compact object per line, flushed, so a pipe into `jq` prints as it
    goes rather than when the stream ends."""
    print(json.dumps(payload), flush=True)


def _ok(message: str) -> int:
    _print({"ok": True, "detail": message})
    return ExitStatus.OK


def _fail(error: str, detail: object, status: ExitStatus, **more: object) -> int:
    """A failure the way every `<name>ctl` reports one: one JSON object on
    stderr and a distinct exit status, never a stack.

    Tracebacks are for bugs in the client. Everything a user can cause — a rig
    that is off, a typo in an address, a scene-config that does not exist — is
    a sentence and a number.
    """
    print(json.dumps({"error": error, "detail": str(detail), **more}), file=sys.stderr)
    return status


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


def _host_of(endpoint: str) -> str:
    """`tcp://HOST:PORT` → `HOST`, brackets and all for an IPv6 literal."""
    return endpoint.removeprefix("tcp://").rpartition(":")[0]


# ── Entry point ───────────────────────────────────────────────────────────────


def main(argv: Sequence[str] | None = None) -> int:
    """Run one command, and return its exit status.

    The last line of defence against a traceback reaching the terminal: any
    exception the command did not expect is a bug in the client, and is
    reported as one, with the traceback behind an environment variable for
    whoever has to fix it.
    """
    parser = build_parser()
    args = parser.parse_args(argv)
    if getattr(args, "file", None) == "-" and not args.name:
        parser.error("put - reads stdin and needs --name")
    try:
        return _run(args)
    except KeyboardInterrupt:
        return ExitStatus.INTERRUPTED
    except BrokenPipeError:
        # `vstimctl state | head` closes the pipe under us. Redirect stdout to
        # the void so the interpreter's own flush at exit cannot fail as well.
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        return ExitStatus.FAILURE
    except Exception as problem:
        if os.environ.get(TRACEBACK_ENVIRONMENT_VARIABLE):
            raise
        return _fail(
            "bug",
            f"{type(problem).__name__}: {problem} — set {TRACEBACK_ENVIRONMENT_VARIABLE}=1 "
            "for the traceback, and report it at https://github.com/braemons/vstimd/issues",
            ExitStatus.FAILURE,
        )


def _run(args: argparse.Namespace) -> int:
    func: Callable[..., int] = args.func
    if not getattr(args, "needs_connection", True):
        try:
            return func(args)
        except DiscoveryUnavailableError as problem:
            return _fail("unavailable", problem, ExitStatus.FAILURE)
        except AddressError as problem:
            return _fail("usage", problem, ExitStatus.USAGE)

    try:
        address = normalize_address(args.rig)
    except AddressError as problem:
        return _fail("usage", problem, ExitStatus.USAGE)

    timeout_s = args.timeout if args.timeout > 0 else None
    if args.command in _BLOCKING_COMMANDS:
        timeout_s = None

    import zmq  # type: ignore[import]  # here, so that `discover` works without a server

    try:
        conn = VstimdClient(address, recv_timeout_s=timeout_s)
    except zmq.ZMQError as problem:
        return _fail("unavailable", f"cannot open {address}: {problem}", ExitStatus.UNAVAILABLE)

    try:
        return func(conn, args)
    except zmq.Again:
        # REQ sockets queue silently when nothing is listening, so a dead rig
        # and a wrong address both surface here rather than at connect time.
        return _fail(
            "unavailable",
            f"no answer from {address} within {args.timeout:g} s; is vstimd running there? "
            "`vstimctl discover` lists the rigs this machine can see",
            ExitStatus.UNAVAILABLE,
        )
    except zmq.ZMQError as problem:
        return _fail("unavailable", f"{address}: {problem}", ExitStatus.UNAVAILABLE)
    except SceneConfigNotFoundError as problem:
        return _fail("not_found", problem, ExitStatus.NOT_FOUND)
    except VstimdError as problem:
        return _fail("refused", problem, ExitStatus.REFUSED)
    except TimeoutError as problem:
        # The client's name for zmq.Again: no reply at all. Over REQ/REP that
        # is indistinguishable from nothing listening, so it is `unavailable`.
        return _fail("unavailable", problem, ExitStatus.UNAVAILABLE)
    except OSError as problem:
        return _fail("unreadable", problem, ExitStatus.FAILURE)
    finally:
        conn.close()
