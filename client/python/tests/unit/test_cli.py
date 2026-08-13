"""Unit tests for the ``vstimd-client`` CLI and mDNS discovery parsing."""
from __future__ import annotations

import argparse
import json

import pytest

from vstimd import __version__
from vstimd.cli import discovery
from vstimd.cli.discovery import DiscoveredServer, parse_avahi_browse
from vstimd.cli.main import build_parser, cmd_shutdown, main, resolve_address

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
    defaults = {"address": None, "host": None, "port": 5555}
    return argparse.Namespace(**{**defaults, **kwargs})


def test_resolve_address_default(monkeypatch):
    monkeypatch.delenv("VSTIMD_ADDRESS", raising=False)
    assert resolve_address(_args()) == "tcp://localhost:5555"


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


def test_discover_nothing_found_exits_nonzero(fake_discover, capsys):
    fake_discover([])
    assert main(["discover"]) == 1
    assert "No vstimd servers found" in capsys.readouterr().err


def test_discover_without_backend_exits_two(fake_discover, capsys):
    fake_discover([], error=discovery.DiscoveryUnavailableError("no mDNS backend available"))
    assert main(["discover"]) == 2
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


def test_command_requires_a_subcommand():
    with pytest.raises(SystemExit):
        main([])


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

    assert cmd_shutdown(_DummyConn(), args) == 1
    assert _DummyConn.system.called is False
    assert "use --yes" in capsys.readouterr().err
