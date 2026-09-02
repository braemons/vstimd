"""Unit tests for the server-error → exception mapping.

These need no server: they drive :meth:`Connection._send` with a fake socket
that returns whatever response the test wants.
"""
from __future__ import annotations

import pytest

from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import shared_set_requests_pb2
from vstimd.connection import Connection
from vstimd.exceptions import (
    SceneConfigError,
    SceneConfigNotFoundError,
    HandleNotFoundError,
    InvalidArgumentError,
    NotReadyError,
    ProtocolError,
    StimulusError,
    UnknownServerError,
    VstimdError,
    error_for_code,
    exception_type_for,
)
from vstimd.response import ErrorCode


# ── The enum against the wire contract ────────────────────────────────────────


def test_error_code_enum_matches_the_proto():
    """Every code in service.proto exists in ErrorCode, with the same number.

    The bug this guards: FILE_NOT_FOUND..FILE_ALREADY_EXISTS were added to the
    proto and never to ErrorCode, so ``ErrorCode(10)`` raised ValueError while
    the client was trying to report a missing config.
    """
    proto_codes = {
        value.name.removeprefix("ERROR_CODE_"): value.number
        for value in service_pb2.ErrorCode.DESCRIPTOR.values
    }
    python_codes = {c.name: c.value for c in ErrorCode}
    # Without this the test passes vacuously if the descriptor yields nothing.
    assert {"OK", "UNKNOWN", "FILE_ALREADY_EXISTS"} <= set(proto_codes)

    missing = {n: v for n, v in proto_codes.items() if n not in python_codes}
    extra = {n: v for n, v in python_codes.items() if n not in proto_codes}
    renumbered = {
        n: (v, python_codes[n])
        for n, v in proto_codes.items()
        if n in python_codes and python_codes[n] != v
    }
    assert not missing, f"in service.proto but not in ErrorCode: {missing}"
    assert not extra, f"in ErrorCode but not in service.proto: {extra}"
    assert not renumbered, f"different numbers (proto, python): {renumbered}"


def test_every_proto_code_can_be_parsed():
    # ErrorCode._missing_ buckets unknown values, so the check above is what
    # catches drift — but a round-trip proves nothing is silently rebucketed.
    for value in service_pb2.ErrorCode.DESCRIPTOR.values:
        assert ErrorCode(value.number).name == value.name.removeprefix("ERROR_CODE_")


def test_every_failure_code_maps_to_its_own_exception():
    failures = [c for c in ErrorCode if c is not ErrorCode.OK]
    mapped = {c: exception_type_for(c) for c in failures}
    assert all(issubclass(t, VstimdError) for t in mapped.values())
    # Distinct classes, so `except` can single out any one of them.
    assert len(set(mapped.values())) == len(failures)


def test_unknown_future_code_is_reported_not_crashed():
    # An older client against a newer server: the code is not in the enum.
    exc = error_for_code(999, "")
    assert isinstance(exc, UnknownServerError)
    assert "999" in str(exc)  # the raw number survives for a bug report


# ── Exception payload ─────────────────────────────────────────────────────────


def test_exception_carries_code_detail_and_context():
    exc = error_for_code(
        ErrorCode.HANDLE_NOT_FOUND, "no such stimulus", command="set_position", handle=7
    )
    assert isinstance(exc, HandleNotFoundError)
    assert exc.code is ErrorCode.HANDLE_NOT_FOUND
    assert exc.detail == "no such stimulus"
    assert exc.command == "set_position"
    assert exc.handle == 7
    assert str(exc) == "no such stimulus (set_position, handle 7)"


def test_system_command_has_no_handle_in_its_message():
    exc = error_for_code(ErrorCode.NOT_READY, "still starting", command="clear_all")
    assert exc.handle is None
    assert str(exc) == "still starting (clear_all)"


def test_exception_knows_its_own_code_when_raised_by_hand():
    assert NotReadyError("nope").code is ErrorCode.NOT_READY


def test_families_can_be_caught_together():
    assert issubclass(SceneConfigNotFoundError, SceneConfigError)
    assert issubclass(HandleNotFoundError, StimulusError)
    # ...without losing the common base.
    assert issubclass(SceneConfigError, VstimdError)
    assert issubclass(StimulusError, VstimdError)
    # A grouping is not itself a code, so it never shadows a real error.
    assert SceneConfigError.code is None
    assert StimulusError.code is None


# ── Connection._send ──────────────────────────────────────────────────────────


class _FakeSocket:
    """Stands in for a REQ socket, replaying one canned reply."""

    def __init__(self, reply: bytes) -> None:
        self.reply = reply
        self.sent: list[bytes] = []

    def send(self, payload: bytes) -> None:
        self.sent.append(payload)

    def recv(self) -> bytes:
        return self.reply

    def setsockopt(self, *_: object) -> None:
        pass

    def close(self) -> None:
        pass


def _connection_replying(reply: bytes) -> Connection:
    conn = Connection.__new__(Connection)  # no ZMQ context, no network
    conn._address = "tcp://test:5555"
    conn._recv_timeout_ms = -1
    conn._sock = _FakeSocket(reply)
    return conn


def _reply(code: int, error: str = "") -> bytes:
    return service_pb2.Response(code=code, error=error).SerializeToString()


def test_send_returns_the_response_on_ok():
    conn = _connection_replying(_reply(service_pb2.ERROR_CODE_OK))
    resp = conn._send(service_pb2.Request(system=service_pb2.SystemTarget()))
    assert resp.code == service_pb2.ERROR_CODE_OK


def test_send_raises_with_the_failing_command_and_handle():
    conn = _connection_replying(
        _reply(service_pb2.ERROR_CODE_HANDLE_NOT_FOUND, "stimulus 7 is gone")
    )
    req = service_pb2.Request(
        stimulus=7, set_alpha=shared_set_requests_pb2.SetAlphaRequest(opacity=0.5)
    )
    with pytest.raises(HandleNotFoundError) as exc_info:
        conn._send(req)
    exc = exc_info.value
    assert exc.command == "set_alpha"
    assert exc.handle == 7
    assert exc.detail == "stimulus 7 is gone"


def test_send_raises_config_errors_for_file_codes():
    # The path that used to fail while building the error rather than raising it.
    conn = _connection_replying(
        _reply(service_pb2.ERROR_CODE_FILE_NOT_FOUND, "no config 'gratings'")
    )
    with pytest.raises(SceneConfigNotFoundError) as exc_info:
        conn._send(service_pb2.Request(system=service_pb2.SystemTarget()))
    assert exc_info.value.code is ErrorCode.FILE_NOT_FOUND


def test_send_raises_protocol_error_on_an_undecodable_reply():
    conn = _connection_replying(b"\xff\xff\xff\xff not protobuf")
    with pytest.raises(ProtocolError) as exc_info:
        conn._send(service_pb2.Request(system=service_pb2.SystemTarget()))
    assert "other than vstimd" in str(exc_info.value)


def test_send_raises_protocol_error_when_no_code_is_set():
    # An empty frame parses cleanly into a response whose code is UNSPECIFIED;
    # treating that as success would let a mutation silently do nothing.
    conn = _connection_replying(b"")
    with pytest.raises(ProtocolError):
        conn._send(service_pb2.Request(system=service_pb2.SystemTarget()))


def test_send_gives_a_message_even_when_the_server_sends_none():
    conn = _connection_replying(_reply(service_pb2.ERROR_CODE_INVALID_ARGUMENT))
    with pytest.raises(InvalidArgumentError) as exc_info:
        conn._send(service_pb2.Request(system=service_pb2.SystemTarget()))
    assert str(exc_info.value)
