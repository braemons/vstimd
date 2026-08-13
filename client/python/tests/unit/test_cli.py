"""Unit tests for the ``vstimd-client`` CLI and mDNS discovery parsing."""
from __future__ import annotations

import argparse
import json

import pytest

from vstimd import __version__
from vstimd.cli import discovery
from vstimd.cli.address import AddressError, normalize_address
from vstimd.cli.discovery import DiscoveredServer, parse_avahi_browse
from vstimd.cli.exit_codes import ExitCode
from vstimd.cli.main import (
    _COMMAND_GROUPS,
    _CommandFailure,
    build_parser,
    choose_address,
    cmd_shutdown,
    format_overview,
    main,
    resolve_address,
)

AVAHI_OUTPUT = """\
+;eth0;IPv4;vstimd-a1b2c3;_vstimd._tcp;local
=;eth0;IPv4;vstimd-a1b2c3;_vstimd._tcp;local;vstimd-a1b2c3.local;192.168.1.10;5555;"id=vstimd-a1b2c3"
=;wlan0;IPv4;vstimd-a1b2c3;_vstimd._tcp;local;vstimd-a1b2c3.local;10.0.0.7;5555;"id=vstimd-a1b2c3"
=;eth0;IPv4;vstimd-ffee00 #2;_vstimd._tcp;local;vstimd-ffee00.local;192.168.1.11;5555;"id=vstimd-ffee00"
"""


# ── avahi-browse parsing ──────────────────────────────────────────────────────


def test_parse_avahi_browse_reads_resolved_records():
    servers = parse_avahi_browse(AVAHI_OUTPUT)
    assert len(servers) == 3  # the '+' announcement line is ignored
    first = servers[0]
    assert first.id == "vstimd-a1b2c3"
    assert first.hostname == "vstimd-a1b2c3.local"
    assert first.addresses == ("192.168.1.10",)
    assert first.port == 5555
    assert first.address == "tcp://vstimd-a1b2c3.local:5555"


def test_parse_avahi_browse_handles_escaped_separators():
    line = (
        r"=;eth0;IPv4;odd\;name;_vstimd._tcp;local;host.local;192.168.1.12;5555;"
        '"id=vstimd-odd"'
    )
    (server,) = parse_avahi_browse(line)
    assert server.name == "odd;name"
    assert server.id == "vstimd-odd"


def test_parse_avahi_browse_without_txt_record():
    line = "=;eth0;IPv4;plain;_vstimd._tcp;local;host.local;192.168.1.13;5555;"
    (server,) = parse_avahi_browse(line)
    assert server.id == ""
    assert server.properties == {}
    # Falls back to the service instance name when there is no id= record.
    assert server.address == "tcp://host.local:5555"


def test_dedupe_merges_interfaces_and_sorts_by_id():
    servers = discovery._dedupe(parse_avahi_browse(AVAHI_OUTPUT))
    assert [s.id for s in servers] == ["vstimd-a1b2c3", "vstimd-ffee00"]
    assert servers[0].addresses == ("192.168.1.10", "10.0.0.7")


def test_address_falls_back_to_ip_without_hostname():
    server = DiscoveredServer(name="x", id="x", hostname="", addresses=("10.0.0.1",))
    assert server.address == "tcp://10.0.0.1:5555"


# ── address resolution ────────────────────────────────────────────────────────


def _args(**kwargs) -> argparse.Namespace:
    defaults = {"address": None, "host": None, "port": 5555, "non_interactive": False}
    return argparse.Namespace(**{**defaults, **kwargs})


def test_resolve_address_is_none_when_nothing_was_given(monkeypatch):
    # None, not the default endpoint: choose_address decides what "nothing"
    # means, and it may go looking on the network first.
    monkeypatch.delenv("VSTIMD_ADDRESS", raising=False)
    assert resolve_address(_args()) is None


def test_resolve_address_env(monkeypatch):
    monkeypatch.setenv("VSTIMD_ADDRESS", "tcp://box:6000")
    assert resolve_address(_args()) == "tcp://box:6000"


def test_resolve_address_explicit_beats_env(monkeypatch):
    monkeypatch.setenv("VSTIMD_ADDRESS", "tcp://box:6000")
    assert resolve_address(_args(address="tcp://other:1")) == "tcp://other:1"


def test_resolve_address_bare_host_gets_mdns_suffix():
    assert resolve_address(_args(host="vstimd-a1b2c3")) == "tcp://vstimd-a1b2c3.local:5555"


def test_resolve_address_qualified_host_left_alone():
    assert resolve_address(_args(host="10.0.0.5", port=6000)) == "tcp://10.0.0.5:6000"


def test_address_and_host_are_mutually_exclusive():
    with pytest.raises(SystemExit):
        main(["--address", "tcp://a:1", "--host", "b", "info"])


# ── address normalisation ─────────────────────────────────────────────────────


@pytest.mark.parametrize(
    ("raw", "expected"),
    [
        # The three spellings of the same rig, all accepted.
        ("tcp://10.0.1.42:5555", "tcp://10.0.1.42:5555"),
        ("10.0.1.42:5555", "tcp://10.0.1.42:5555"),
        ("10.0.1.42", "tcp://10.0.1.42:5555"),
        # Partially-specified endpoints get the missing half filled in.
        ("tcp://rig.local", "tcp://rig.local:5555"),
        ("rig.local:6000", "tcp://rig.local:6000"),
        ("  10.0.1.42  ", "tcp://10.0.1.42:5555"),
        # IPv6 needs brackets to be distinguishable from host:port.
        ("::1", "tcp://[::1]:5555"),
        ("[::1]:6000", "tcp://[::1]:6000"),
        ("[fe80::1]", "tcp://[fe80::1]:5555"),
        # Non-TCP transports have nothing to complete.
        ("ipc:///tmp/vstimd.sock", "ipc:///tmp/vstimd.sock"),
        ("inproc://test", "inproc://test"),
    ],
)
def test_normalize_address(raw, expected):
    assert normalize_address(raw) == expected


def test_normalize_address_honours_an_explicit_default_port():
    assert normalize_address("rig.local", default_port=6000) == "tcp://rig.local:6000"
    # An address that names a port keeps it.
    assert normalize_address("rig.local:1", default_port=6000) == "tcp://rig.local:1"


@pytest.mark.parametrize("raw", ["", "   ", "tcp://", "tcp://:5555", "rig:http", "rig:99999"])
def test_normalize_address_rejects_what_it_cannot_repair(raw):
    with pytest.raises(AddressError):
        normalize_address(raw)


# ── auto-discovery when no address is given ───────────────────────────────────


@pytest.fixture
def no_address(monkeypatch):
    monkeypatch.delenv("VSTIMD_ADDRESS", raising=False)


def _servers(*ids) -> list[DiscoveredServer]:
    return [
        DiscoveredServer(name=i, id=i, hostname=f"{i}.local", addresses=("10.0.0.1",))
        for i in ids
    ]


def test_one_discovered_server_is_used_without_asking(fake_discover, no_address, capsys):
    fake_discover(_servers("rig-a"))
    assert choose_address(_args()) == "tcp://rig-a.local:5555"
    # Announced, because "whichever rig answered" is a bad thing to be unsure
    # of when the next word is `shutdown`.
    assert "using rig-a" in capsys.readouterr().err


def test_nothing_discovered_falls_back_to_localhost(fake_discover, no_address):
    fake_discover([])
    assert choose_address(_args()) == "tcp://localhost:5555"


def test_no_mdns_backend_falls_back_to_localhost(fake_discover, no_address):
    fake_discover([], error=discovery.DiscoveryUnavailableError("no backend"))
    assert choose_address(_args()) == "tcp://localhost:5555"


def test_an_explicit_address_skips_discovery_entirely(monkeypatch, no_address):
    def _explode(*_, **__):
        raise AssertionError("discovery ran despite an explicit address")

    monkeypatch.setattr(discovery, "discover", _explode)
    assert choose_address(_args(address="10.0.0.9")) == "tcp://10.0.0.9:5555"
    assert choose_address(_args(host="rig-b")) == "tcp://rig-b.local:5555"


def test_env_address_skips_discovery(monkeypatch):
    def _explode(*_, **__):
        raise AssertionError("discovery ran despite $VSTIMD_ADDRESS")

    monkeypatch.setattr(discovery, "discover", _explode)
    monkeypatch.setenv("VSTIMD_ADDRESS", "tcp://box:6000")
    assert choose_address(_args()) == "tcp://box:6000"


def test_several_servers_prompt_for_a_choice(fake_discover, no_address, monkeypatch, capsys):
    fake_discover(_servers("rig-a", "rig-b", "rig-c"))
    monkeypatch.setattr("sys.stdin.isatty", lambda: True)
    monkeypatch.setattr("builtins.input", lambda: "2")

    assert choose_address(_args()) == "tcp://rig-b.local:5555"
    err = capsys.readouterr().err
    assert "3 vstimd servers found" in err
    for label in ("rig-a", "rig-b", "rig-c"):
        assert label in err


def test_the_selector_reprompts_on_a_bad_answer(fake_discover, no_address, monkeypatch, capsys):
    fake_discover(_servers("rig-a", "rig-b"))
    monkeypatch.setattr("sys.stdin.isatty", lambda: True)
    answers = iter(["", "0", "9", "banana", "1"])
    monkeypatch.setattr("builtins.input", lambda: next(answers))

    assert choose_address(_args()) == "tcp://rig-a.local:5555"
    assert capsys.readouterr().err.count("not a choice") == 4


def test_the_selector_can_be_cancelled(fake_discover, no_address, monkeypatch):
    fake_discover(_servers("rig-a", "rig-b"))
    monkeypatch.setattr("sys.stdin.isatty", lambda: True)
    monkeypatch.setattr("builtins.input", lambda: "q")

    with pytest.raises(_CommandFailure) as exc_info:
        choose_address(_args())
    assert exc_info.value.code == ExitCode.FAILURE


@pytest.mark.parametrize(
    ("interactive_flag", "isatty"),
    [
        pytest.param(True, True, id="--non-interactive on a terminal"),
        pytest.param(False, False, id="piped stdin"),
    ],
)
def test_several_servers_never_prompt_when_nobody_can_answer(
    fake_discover, no_address, monkeypatch, capsys, interactive_flag, isatty
):
    fake_discover(_servers("rig-a", "rig-b"))
    monkeypatch.setattr("sys.stdin.isatty", lambda: isatty)
    monkeypatch.setattr(
        "builtins.input", lambda: pytest.fail("prompted with nobody to answer")
    )

    with pytest.raises(_CommandFailure) as exc_info:
        choose_address(_args(non_interactive=interactive_flag))
    assert exc_info.value.code == ExitCode.USAGE
    # The candidates are still listed, so the user knows what to pick from.
    err = capsys.readouterr().err
    assert "rig-a" in err and "rig-b" in err


def test_ambiguous_discovery_exits_usage_through_main(fake_discover, no_address, monkeypatch, capsys):
    fake_discover(_servers("rig-a", "rig-b"))
    monkeypatch.setattr("sys.stdin.isatty", lambda: False)
    assert main(["info"]) == ExitCode.USAGE
    assert "cannot choose between them" in capsys.readouterr().err


def test_bad_address_exits_usage_without_a_traceback(capsys):
    # The reported bug: a malformed address reached zmq and produced a stack.
    assert main(["-a", "rig:http", "info"]) == ExitCode.USAGE
    err = capsys.readouterr().err
    assert "non-numeric port" in err
    assert "Traceback" not in err


# ── discover command ──────────────────────────────────────────────────────────


@pytest.fixture
def fake_discover(monkeypatch):
    """Replace the network browse with a canned result set."""

    def install(servers, error=None):
        def _fake(timeout_s, *, backend=None):
            if error is not None:
                raise error
            return servers

        monkeypatch.setattr(discovery, "discover", _fake)

    return install


def test_discover_prints_table(fake_discover, capsys):
    fake_discover(discovery._dedupe(parse_avahi_browse(AVAHI_OUTPUT)))
    assert main(["discover"]) == 0
    out = capsys.readouterr().out
    assert "vstimd-a1b2c3" in out
    assert "tcp://vstimd-a1b2c3.local:5555" in out
    assert out.splitlines()[0].startswith("ID")


def test_discover_json(fake_discover, capsys):
    fake_discover(parse_avahi_browse(AVAHI_OUTPUT)[:1])
    assert main(["--json", "discover"]) == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload[0]["address"] == "tcp://vstimd-a1b2c3.local:5555"
    assert payload[0]["properties"] == {"id": "vstimd-a1b2c3"}


def test_discover_nothing_found_exits_not_found(fake_discover, capsys):
    fake_discover([])
    assert main(["discover"]) == ExitCode.NOT_FOUND
    assert "no vstimd servers found" in capsys.readouterr().err


def test_discover_json_nothing_found_still_exits_not_found(fake_discover, capsys):
    fake_discover([])
    assert main(["--json", "discover"]) == ExitCode.NOT_FOUND
    # The empty list still has to be valid JSON for a script to parse.
    assert json.loads(capsys.readouterr().out) == []


def test_discover_without_backend_exits_no_backend(fake_discover, capsys):
    fake_discover([], error=discovery.DiscoveryUnavailableError("no mDNS backend available"))
    assert main(["discover"]) == ExitCode.NO_BACKEND
    assert "no mDNS backend available" in capsys.readouterr().err


# ── parser wiring ─────────────────────────────────────────────────────────────


def test_every_subcommand_has_a_handler():
    parser = build_parser()
    (subparsers,) = [
        a for a in parser._subparsers._group_actions  # type: ignore[union-attr]
    ]
    for name, sub in subparsers.choices.items():
        defaults = sub.get_default("func")
        if defaults is None:
            # Command groups (e.g. `config`) dispatch via their own subparsers.
            assert sub._subparsers is not None, f"{name} has neither handler nor subcommands"
        else:
            assert callable(defaults)


def test_no_command_prints_the_overview(capsys):
    assert main([]) == ExitCode.USAGE
    err = capsys.readouterr().err
    # Not argparse's bare "the following arguments are required: COMMAND".
    assert "required" not in err
    assert "Examples:" in err
    for _, commands in _COMMAND_GROUPS:
        for name, _ in commands:
            assert name in err


def test_overview_groups_cover_every_subcommand():
    parser = build_parser()
    (subparsers,) = [
        a for a in parser._subparsers._group_actions  # type: ignore[union-attr]
    ]
    listed = {name for _, commands in _COMMAND_GROUPS for name, _ in commands}
    # `list` is an alias for `ls`; showing both would be noise.
    documented = set(subparsers.choices) - {"list"}
    assert listed == documented, "the overview and the parser have drifted apart"


def test_overview_mentions_how_to_reach_a_server():
    text = format_overview()
    assert "VSTIMD_ADDRESS" in text
    assert "tcp://localhost:5555" in text


@pytest.mark.parametrize("flag", ["--version", "-V"])
def test_version_flag_prints_the_client_version(flag, capsys):
    # argparse's `version` action exits 0 after printing, before any subcommand
    # is required — so this also pins that --version works with no command.
    with pytest.raises(SystemExit) as exc:
        main([flag])
    assert exc.value.code == 0
    assert capsys.readouterr().out.strip() == f"vstimd-client {__version__}"


def test_shutdown_requires_yes_when_stdin_is_non_interactive(monkeypatch, capsys):
    class _DummySystem:
        called = False

        def shutdown(self):
            self.called = True

    class _DummyConn:
        address = "tcp://localhost:5555"
        system = _DummySystem()

    monkeypatch.setattr("sys.stdin.isatty", lambda: False)
    args = argparse.Namespace(yes=False, as_json=False)

    assert cmd_shutdown(_DummyConn(), args) == ExitCode.USAGE
    assert _DummyConn.system.called is False
    assert "use --yes" in capsys.readouterr().err


# ── exit codes ────────────────────────────────────────────────────────────────


def test_unreachable_server_times_out_without_a_traceback(capsys):
    # Port 1 has nothing on it; a REQ socket connects regardless and the send
    # queues, so this exercises the recv-timeout path, not the connect path.
    assert main(["-a", "127.0.0.1:1", "-t", "0.2", "info"]) == ExitCode.TIMEOUT
    err = capsys.readouterr().err
    assert "no reply from tcp://127.0.0.1:1" in err
    assert "Traceback" not in err


def test_unknown_transport_reports_unavailable(capsys):
    assert main(["-a", "bogus://nowhere", "info"]) == ExitCode.UNAVAILABLE
    err = capsys.readouterr().err
    assert "cannot open a connection" in err
    assert "Traceback" not in err


def test_exit_codes_are_distinct():
    codes = [c.value for c in ExitCode]
    assert len(codes) == len(set(codes))
