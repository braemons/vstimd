"""The event stream against a real vstimd, with a real frame clock running.

    make test-e2e-events

**What only this can be wrong about.** The unit tests drive the publisher and
the Rust integration tests drive `handle_request`; both pass on a server whose
frame loop never runs. Here a real process is rendering (into nothing), a real
command goes over the real ZMQ socket, and the question is whether the event
that comes out the other side describes what actually happened -- above all
whether it carries a **frame number that is moving**, which is the one field no
test with a still clock can check.

That matters more than it sounds. The frame is the join key: everything triald
will ever attribute to a trial, it attributes by frame. A stream that published
frame 0 forever would satisfy every schema and lose the entire experiment.

**Not under `tests/e2e/`, and deliberately.** That suite is built for a person
watching a display: autouse fixtures caption every test on screen and reset the
scene between them, both by sending commands. Here every command sent is an
assertion, so a fixture that quietly sends its own would be testing the fixture.
It also assumes a server on the default port and blocks forever if there is
none, which is how this suite spent its first hour.

Null mode is not a lesser path here. It runs the same `frame_loop::advance_frame`
as the display backends, deliberately -- it once kept its own copy and the copy
had drifted, publishing no VTL edges and stamping every command frame 0.
"""

from __future__ import annotations

import os
import pathlib
import socket
import subprocess
import sys
import time

import pytest
import zmq

from vstimd import Connection
from vstimd._proto import service_pb2, system_pb2
from vstimd.events import EventSubscriber, Topic, decode_command
from vstimd.exceptions import HandleNotFoundError
from vstimd.stimuli import RectParams
from vstimd.vtl import VtlHandle

_REPO_ROOT = pathlib.Path(__file__).parents[4]

#: How long any one command waits for a reply.
#:
#: Set at all because the default is to block forever, and a server that died
#: after the readiness check would then wedge the whole run with no output —
#: which is a much worse way to learn about it than a failed test.
RECV_TIMEOUT_SECONDS = 10.0


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


def reachable(address: str, timeout_ms: int = 500) -> bool:
    """Whether a vstimd answers on `address`.

    Its own copy rather than an import from `tests/e2e`: this suite shares no
    fixtures with that one on purpose, and a helper is a cheaper thing to repeat
    than a dependency on a conftest built for a different kind of test.
    """
    sock = zmq.Context.instance().socket(zmq.REQ)
    sock.setsockopt(zmq.LINGER, 0)
    sock.setsockopt(zmq.RCVTIMEO, timeout_ms)
    sock.connect(address)
    try:
        request = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_server_info=system_pb2.QueryServerInfoRequest(),
        )
        sock.send(request.SerializeToString())
        sock.recv()
        return True
    except zmq.Again:
        return False
    finally:
        sock.close()


def _server_binary() -> pathlib.Path:
    exe = "vstimd.exe" if sys.platform == "win32" else "vstimd"
    binary = _REPO_ROOT / "target" / "release" / exe
    if not binary.exists():
        if subprocess.run(["cargo", "build", "--release"], cwd=_REPO_ROOT).returncode:
            pytest.fail("cargo build --release failed")
    if not binary.exists():
        pytest.fail(f"server binary not found at {binary}")
    return binary


@pytest.fixture(scope="module")
def rig(tmp_path_factory) -> tuple[str, int]:
    """Its own server, because it needs its own event port.

    Not shared with the other null suites for the usual reason: every test there
    resets the scene, and a reset arriving mid-assertion here would look like
    anything but the cause.
    """
    if os.environ.get("VSTIMD_SERVER"):
        pytest.skip("VSTIMD_SERVER is set; this suite starts a server of its own")

    command_port = _free_port()
    event_port = _free_port()
    # Two ephemeral ports can come back the same number, and the collision is
    # silent: the second bind fails and the stream is simply never there.
    while event_port == command_port:
        event_port = _free_port()

    address = f"tcp://localhost:{command_port}"
    log = tmp_path_factory.mktemp("vstimd") / "server.log"
    with log.open("w") as sink:
        proc = subprocess.Popen(
            [
                str(_server_binary()),
                "--null",
                "--zmq-port",
                str(command_port),
                "--event-port",
                str(event_port),
                "--no-web",
            ],
            stdout=sink,
            stderr=subprocess.STDOUT,
            env={**os.environ, "RUST_LOG": "info"},
        )

        for _ in range(20):
            if proc.poll() is not None:
                pytest.fail(f"the server exited at once:\n{log.read_text()}")
            if reachable(address):
                break
            time.sleep(0.5)
        else:
            proc.terminate()
            pytest.fail(f"server not ready in time:\n{log.read_text()}")

        yield address, event_port

        proc.terminate()
        proc.wait(timeout=5)
    # Its own log, printed only when something above failed: a server that
    # started and then died mid-suite is otherwise invisible, and the tests just
    # time out one after another saying nothing about why.
    print(log.read_text())


@pytest.fixture
def events(rig):
    """A subscriber attached and settled before any test issues a command.

    The sleep is not politeness. ZMQ's connect is asynchronous and a PUB socket
    discards whatever it publishes before a subscription reaches it, so a test
    that sent a command immediately would fail intermittently and blame the
    server.
    """
    _, event_port = rig
    subscriber = EventSubscriber("127.0.0.1", event_port, topic=Topic.ALL)
    time.sleep(0.5)
    yield subscriber
    subscriber.close()


def rect(conn: Connection, width: float, height: float | None = None) -> int:
    """The smallest command that creates something, and returns a handle."""
    return conn.stimuli.shapes.create_rect(
        params=RectParams(width_px=width, height_px=height if height else width)
    )


def until(events: EventSubscriber, topic: str, *, timeout_ms: int = 3000):
    """The next event on `topic`, ignoring the others."""
    deadline = time.monotonic() + timeout_ms / 1000
    while time.monotonic() < deadline:
        event = events.receive(timeout_ms=500)
        if event is not None and event.topic == topic:
            return event
    pytest.fail(f"no {topic} event within {timeout_ms} ms")


# ── The clock is running ──────────────────────────────────────────────────────


def test_the_frame_index_advances_while_the_server_renders(rig, events):
    """The check no test with a still clock can make.

    Two commands a few frames apart must carry different frame numbers, and the
    second must be the later one. Everything triald attributes to a trial, it
    attributes by this number.
    """
    address, _ = rig
    with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
        rect(vstimd, 10)
        first = until(events, Topic.COMMAND_APPLIED)
        time.sleep(0.25)  # ~15 frames at 60 Hz
        rect(vstimd, 20)
        second = until(events, Topic.COMMAND_APPLIED)

    assert first.frame > 0, "the frame clock never started"
    assert second.frame > first.frame, (
        "the frame index did not advance between two commands a quarter of a "
        "second apart — every event would be attributable to the same trial"
    )


# ── A command produces its record ─────────────────────────────────────────────


def test_a_command_over_the_wire_comes_back_out_as_an_event(rig, events):
    """The round trip that matters: what went in on the REP socket is what the
    PUB socket says was applied, byte for byte."""
    address, _ = rig
    with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
        handle = rect(vstimd, 123, 45)

    event = until(events, Topic.COMMAND_APPLIED)
    assert event.payload.accepted
    assert event.payload.response_handle == handle, (
        "the record names the handle the caller was given, so a replay can "
        "check it allocated the same one"
    )
    request = decode_command(event.payload)
    assert request.WhichOneof("body") == "create_rect"
    assert request.create_rect.params.width_px == 123
    assert request.create_rect.params.height_px == 45


def test_a_refused_command_is_recorded_with_its_error(rig, events):
    """A refusal changed nothing — but a replay in which it *succeeds* has
    diverged, and only the record makes that visible instead of silent."""
    address, _ = rig
    with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
        with pytest.raises(HandleNotFoundError):
            vstimd.stimuli.delete(9999)

    event = until(events, Topic.COMMAND_APPLIED)
    assert not event.payload.accepted
    assert event.payload.error_code != 0


def test_every_command_is_recorded_and_in_the_order_it_was_applied(rig, events):
    """No sampling, no coalescing: a record with holes in it is not a record."""
    address, _ = rig
    sizes = [11, 22, 33, 44, 55]
    with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
        for size in sizes:
            rect(vstimd, size)

    seen = [until(events, Topic.COMMAND_APPLIED) for _ in sizes]
    widths = [decode_command(e.payload).create_rect.params.width_px for e in seen]
    assert widths == sizes
    # Frames are non-decreasing: several commands can share an inter-frame
    # window, but none may appear to have been applied before an earlier one.
    frames = [e.frame for e in seen]
    assert frames == sorted(frames)
    assert events.gaps == 0, "the subscriber kept up and nothing was lost"


# ── VTL edges reach the stream ────────────────────────────────────────────────


def test_setting_a_trigger_line_publishes_the_edge_the_frame_drained(rig, events):
    """The record of what the animations branched on.

    An input line, set over ZMQ the way the animation tests drive one, because
    the input edges are the ones a replay cannot do without: animations *branch*
    on them, so the same commands with different inputs are a different
    stimulus.
    """
    address, _ = rig
    with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
        vstimd.vtl.set_line(VtlHandle.input(0, 3), True)

    edge = until(events, Topic.VTL_EDGE)
    assert (edge.payload.bank, edge.payload.bit) == (0, 3)
    assert edge.frame > 0, "an edge belongs to the frame it was drained at"


# ── Filtering, against the real socket ────────────────────────────────────────


def test_a_filtered_subscriber_gets_its_topic_and_no_others(rig):
    """What triald does: ask for what you need, at the socket.

    Filtering happens inside ZeroMQ before anything is queued for the client, so
    a consumer with work to do never pays to receive the rest.
    """
    address, event_port = rig
    with EventSubscriber("127.0.0.1", event_port, topic=Topic.COMMAND_APPLIED) as only:
        time.sleep(0.5)
        with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
            vstimd.vtl.set_line(VtlHandle.input(0, 5), True)
            rect(vstimd, 10)

        for _ in range(5):
            event = only.receive(timeout_ms=1000)
            if event is None:
                break
            assert event.topic == Topic.COMMAND_APPLIED, (
                "a topic this subscriber did not ask for reached it"
            )


def test_filtering_does_not_look_like_loss(rig):
    """**The trap, and the reason `topic_sequence` exists.**

    The global `sequence` counts every event vstimd publishes, so a filtered
    subscriber sees holes it made itself by asking for less — indistinguishable
    from events ZeroMQ discarded on its behalf. A loss signal that cries wolf on
    every filtered subscriber is one nobody reads, and every real consumer
    filters.
    """
    address, event_port = rig
    with EventSubscriber("127.0.0.1", event_port, topic=Topic.COMMAND_APPLIED) as only:
        time.sleep(0.5)
        with Connection(address, recv_timeout_s=RECV_TIMEOUT_SECONDS) as vstimd:
            for size in (10, 20, 30):
                vstimd.vtl.set_line(VtlHandle.input(0, 1), True)
                vstimd.vtl.set_line(VtlHandle.input(0, 1), False)
                # Let a frame pass. VTL edges are drained at frame *start*,
                # so without this the whole loop lands inside one frame window
                # and the edges are all published after the last command --
                # which would leave nothing interleaved to prove anything with.
                time.sleep(0.05)
                rect(vstimd, size)

        received = [only.receive(timeout_ms=2000) for _ in range(3)]
        assert all(e is not None for e in received)
        assert only.gaps == 0, (
            "holes left by this subscriber's own filter were counted as loss"
        )
        # And the interleaved VTL events really were in between: the global
        # sequence has the holes, the per-topic one does not. Consecutive, not
        # starting at 1 — the server is shared across this module and has been
        # publishing since the first test.
        per_topic = [e.topic_sequence for e in received]
        assert per_topic == list(range(per_topic[0], per_topic[0] + 3)), (
            "the topic counter skipped, so this subscriber really did lose events"
        )
        globals_ = [e.sequence for e in received]
        assert globals_ != per_topic
        assert max(b - a for a, b in zip(globals_, globals_[1:], strict=False)) > 1, (
            "the VTL events were not actually interleaved, so this proves nothing"
        )
