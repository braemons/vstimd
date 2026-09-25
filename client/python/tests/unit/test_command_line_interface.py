"""Unit tests for ``vstimctl`` and mDNS discovery parsing."""
from __future__ import annotations

import argparse
import json

import pytest

from vstimd_client import __version__
from vstimd_client.command_line_interface import discovery
from vstimd_client.command_line_interface.address import AddressError, normalize_address
from vstimd_client.command_line_interface.discovery import DiscoveredServer, parse_avahi_browse
from vstimd_client.command_line_interface.exit_status import ExitStatus
from vstimd_client.command_line_interface.main import build_parser, cmd_shutdown, main


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

# ── which rig ─────────────────────────────────────────────────────────────────


def test_the_rig_is_the_flag_then_the_environment_then_localhost(monkeypatch):
    monkeypatch.delenv("BRAEMONS_RIG", raising=False)
    assert build_parser().parse_args(["state"]).rig == "localhost"
    monkeypatch.setenv("BRAEMONS_RIG", "rig-a.local")
    assert build_parser().parse_args(["state"]).rig == "rig-a.local"
    assert build_parser().parse_args(["--rig", "rig-b", "state"]).rig == "rig-b"


def test_a_rig_nobody_named_is_never_browsed_for(monkeypatch, capsys):
    monkeypatch.delenv("BRAEMONS_RIG", raising=False)

    def _no_browsing(*_args, **_kwargs):
        raise AssertionError("a command browsed the network to guess a rig")

    monkeypatch.setattr(discovery, "discover", _no_browsing)
    assert main(["--rig", "127.0.0.1:1", "--timeout", "0.2", "state"]) == ExitStatus.UNAVAILABLE


def test_a_bad_address_is_a_usage_error_without_a_traceback(capsys):
    assert main(["--rig", "rig:http", "state"]) == ExitStatus.USAGE
    failure = json.loads(capsys.readouterr().err)
    assert failure["error"] == "usage"


# ── discover ──────────────────────────────────────────────────────────────────


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


def test_discover_prints_every_rig_as_json(fake_discover, capsys):
    fake_discover(discovery._dedupe(parse_avahi_browse(AVAHI_OUTPUT)))
    assert main(["discover"]) == 0
    payload = json.loads(capsys.readouterr().out)
    assert [server["id"] for server in payload] == ["vstimd-a1b2c3", "vstimd-ffee00"]
    assert payload[0]["address"] == "tcp://vstimd-a1b2c3.local:5555"
    assert payload[0]["properties"] == {"id": "vstimd-a1b2c3"}


def test_discover_nothing_found_exits_not_found(fake_discover, capsys):
    fake_discover([])
    assert main(["discover"]) == ExitStatus.NOT_FOUND
    failure = json.loads(capsys.readouterr().err)
    assert failure["error"] == "not_found"
    assert "no vstimd servers found" in failure["detail"]


def test_discover_without_backend_fails_in_words(fake_discover, capsys):
    fake_discover([], error=discovery.DiscoveryUnavailableError("no mDNS backend available"))
    assert main(["discover"]) == ExitStatus.FAILURE
    assert "no mDNS backend available" in json.loads(capsys.readouterr().err)["detail"]


# ── parser wiring ─────────────────────────────────────────────────────────────


def test_every_subcommand_has_a_handler():
    parser = build_parser()
    (subparsers,) = [
        a for a in parser._subparsers._group_actions  # type: ignore[union-attr]
    ]
    for name, sub in subparsers.choices.items():
        defaults = sub.get_default("func")
        if defaults is None:
            # Command groups (e.g. `scene-configs`) dispatch via their own subparsers.
            assert sub._subparsers is not None, f"{name} has neither handler nor subcommands"
        else:
            assert callable(defaults)


def test_every_ctl_has_state_and_watch():
    parser = build_parser()
    (subparsers,) = [
        a for a in parser._subparsers._group_actions  # type: ignore[union-attr]
    ]
    assert {"state", "watch"} <= set(subparsers.choices)


def test_no_command_is_a_usage_error():
    with pytest.raises(SystemExit) as exc:
        main([])
    assert exc.value.code == ExitStatus.USAGE


def test_a_document_from_stdin_needs_a_name():
    with pytest.raises(SystemExit) as exc:
        main(["scene-configs", "put", "-"])
    assert exc.value.code == ExitStatus.USAGE


@pytest.mark.parametrize("flag", ["--version", "-V"])
def test_version_flag_prints_the_client_version(flag, capsys):
    # argparse's `version` action exits 0 after printing, before any subcommand
    # is required — so this also pins that --version works with no command.
    with pytest.raises(SystemExit) as exc:
        main([flag])
    assert exc.value.code == 0
    assert capsys.readouterr().out.strip() == f"vstimctl {__version__}"


def test_shutdown_requires_yes_when_stdin_is_non_interactive(monkeypatch, capsys):
    class _DummySystem:
        called = False

        def shutdown(self):
            self.called = True

    class _DummyConn:
        address = "tcp://localhost:5555"
        system = _DummySystem()

    monkeypatch.setattr("sys.stdin.isatty", lambda: False)
    args = argparse.Namespace(yes=False)

    assert cmd_shutdown(_DummyConn(), args) == ExitStatus.USAGE
    assert _DummyConn.system.called is False
    assert "--yes" in json.loads(capsys.readouterr().err)["detail"]


# ── exit statuses ─────────────────────────────────────────────────────────────


def test_nothing_answering_is_unavailable_as_json_on_stderr(capsys):
    # Port 1 has nothing on it; a REQ socket connects regardless and the send
    # queues, so this exercises the recv-timeout path, not the connect path.
    assert main(["--rig", "127.0.0.1:1", "--timeout", "0.2", "state"]) == ExitStatus.UNAVAILABLE
    err = capsys.readouterr().err
    failure = json.loads(err)
    assert failure["error"] == "unavailable"
    assert "tcp://127.0.0.1:1" in failure["detail"]
    assert "Traceback" not in err


def test_an_unknown_transport_is_unavailable(capsys):
    assert main(["--rig", "bogus://nowhere", "state"]) == ExitStatus.UNAVAILABLE
    assert json.loads(capsys.readouterr().err)["error"] == "unavailable"


def test_the_exit_statuses_are_the_familys():
    assert {status.name: status.value for status in ExitStatus} == {
        "OK": 0,
        "FAILURE": 1,
        "USAGE": 2,
        "UNAVAILABLE": 3,
        "TIMED_OUT": 4,
        "REFUSED": 5,
        "NOT_FOUND": 6,
        "INTERRUPTED": 130,
    }


def test_the_zmq_port_comes_from_its_txt_record_when_the_srv_port_is_the_web_port():
    # Since 0.3 the SRV port is 8080, where a console loads the panels; the
    # command socket is `zmq_port=`. The old record above has no such key and
    # its SRV port is still the ZMQ one.
    line = (
        "=;eth0;IPv4;vstimd-a1b2c3;_vstimd._tcp;local;vstimd-a1b2c3.local;192.168.1.10;8080;"
        '"id=vstimd-a1b2c3" "zmq_port=5555" "event_port=5556" "elements=/elements/vstimd.js"'
    )
    (server,) = parse_avahi_browse(line)
    assert server.port == 8080
    assert server.zmq_port == 5555
    assert server.address == "tcp://vstimd-a1b2c3.local:5555"
