"""Shared pytest configuration and fixtures for e2e tests."""

import argparse
import os
import time
import warnings

import pytest
import zmq

from vstimd import Connection
from vstimd.events import DEFAULT_EVENT_PORT, EventSubscriber, Topic
from vstimd.system import Camera3D, Lighting3D
from vstimd._proto import service_pb2, system_pb2

from .cases._helpers import Pacing, Stage

#: Where to look for a server when nothing says otherwise. The null suites take
#: this as "no server was asked for" and start one of their own instead.
DEFAULT_SERVER = os.environ.get("VSTIMD_SERVER", "tcp://localhost:5555")
#: The event-stream port of that server. The null suites pick their own.
DEFAULT_EVENT_PORT_OPTION = int(os.environ.get("VSTIMD_EVENT_PORT", DEFAULT_EVENT_PORT))


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--server",
        default=DEFAULT_SERVER,
        help=f"ZMQ address of the vstimd for e2e tests (default: {DEFAULT_SERVER})",
    )
    parser.addoption(
        "--step-delay",
        type=float,
        default=1.0,
        help="Seconds to hold each visual state so a human can inspect it "
        "(default: 1.0). The null suites pin it to 0",
    )
    parser.addoption(
        "--recv-timeout",
        type=float,
        default=None,
        help="Seconds to wait for any one server reply before giving up "
        "(default: block forever). The review TUI sets this so a stalled "
        "server surfaces as a failed test instead of wedging the run.",
    )
    parser.addoption(
        "--event-port",
        type=int,
        default=DEFAULT_EVENT_PORT_OPTION,
        help="Event-stream port of the --server, used to say which frames a "
        f"check_frame_stats failure dropped (default: {DEFAULT_EVENT_PORT_OPTION})",
    )
    parser.addoption(
        "--allow-dropped-frames",
        type=int,
        default=0,
        help="Frames a check_frame_stats test may drop before it fails, on top "
        "of its own max_dropped (default: 0). For reviewing on a desktop, whose "
        "compositor drops frames a rig would not — never for a rig.",
    )
    parser.addoption(
        "--check-frame-stats",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Fail a test marked @check_frame_stats that drops frames while it "
        "runs (default: on). The null renderer never drops, so this only bites "
        "on a real display.",
    )


def pytest_configure(config: pytest.Config) -> None:
    config.addinivalue_line(
        "markers",
        "onscreen(test_id, description, deferred=False): the id an operator "
        "writes down for this test and the caption it shows while it runs. Set "
        "deferred=True when the caption would disturb what the test checks (a "
        "test that compares the whole scene); such a test puts the caption up "
        "itself, with stage.step(), once it is past that point.",
    )
    config.addinivalue_line(
        "markers",
        "check_frame_stats(max_dropped=0): reset the server's frame statistics "
        "before the test body and fail the test if more than max_dropped frames "
        "were dropped by the end of it. Disabled with --no-check-frame-stats.",
    )


class FrameDropWatch:
    """A ``frame.dropped`` subscriber, for saying *where* a test dropped frames.

    The pass/fail verdict is the server's frame-statistics counter, which cannot
    lose anything. Events can: PUB discards for a subscriber that falls behind.
    So these only add detail — which frames — to a failure the counter already
    decided, and say so when the detail is incomplete.
    """

    def __init__(self, subscriber: EventSubscriber) -> None:
        self._subscriber = subscriber
        self._gaps_at_start = subscriber.gaps

    @classmethod
    def connect(
        cls, conn: Connection, host: str, port: int, timeout_s: float = 2.0
    ) -> "FrameDropWatch | None":
        """Subscribe, and wait until the subscription is actually live.

        A SUB socket connects in the background, so a drop published right after
        ``connect()`` returns can be missed. Every command publishes
        ``command.applied``, so the subscription is live once a command sent now
        comes back as an event — after which that topic is dropped again, lest
        each test's hundreds of commands crowd out the drops.

        None when no event arrives in time: the server runs with ``--no-events``,
        or on another port.
        """
        subscriber = EventSubscriber(
            host, port, topic=[Topic.FRAME_DROPPED, Topic.COMMAND_APPLIED]
        )
        deadline = time.monotonic() + timeout_s
        while time.monotonic() < deadline:
            conn.system.query_frame_stats()
            event = subscriber.receive(timeout_ms=100)
            while event is not None:
                if event.topic == Topic.COMMAND_APPLIED:
                    subscriber.unsubscribe(Topic.COMMAND_APPLIED)
                    return cls(subscriber)
                event = subscriber.receive(timeout_ms=0)
        subscriber.close()
        return None

    def close(self) -> None:
        self._subscriber.close()

    def drain(self) -> None:
        """Discard everything queued: drops from before the window opened."""
        while self._subscriber.receive(timeout_ms=0) is not None:
            pass
        self._gaps_at_start = self._subscriber.gaps

    def collect(self, expected: int, timeout_s: float = 0.5) -> tuple[list[tuple[int, int]], int]:
        """``(frame, count)`` per drop event since :meth:`drain`, and events lost.

        Waits up to `timeout_s` for `expected` drops: the render thread publishes
        through a queue, so an event can still be on its way when the counter
        that saw the same drop has already answered.
        """
        drops: list[tuple[int, int]] = []
        deadline = time.monotonic() + timeout_s
        while sum(count for _, count in drops) < expected:
            remaining_ms = int((deadline - time.monotonic()) * 1000)
            event = self._subscriber.receive(timeout_ms=max(0, remaining_ms))
            if event is None:
                break
            if event.topic == Topic.FRAME_DROPPED:
                drops.append((event.frame, event.payload.count))
        return drops, self._subscriber.gaps - self._gaps_at_start


#: The ``user_properties`` key a ``check_frame_stats`` test's report carries its
#: frame statistics under.
FRAME_STATS_PROPERTY = "frame_stats"

_FRAME_DROP_WATCH = pytest.StashKey["FrameDropWatch | None"]()


@pytest.fixture(scope="session")
def event_port(request: pytest.FixtureRequest) -> int:
    return request.config.getoption("--event-port")


@pytest.fixture(scope="session")
def frame_drop_watch(conn: Connection, server_address: str, event_port: int):
    host = server_address.split("://", 1)[-1].rsplit(":", 1)[0]
    watch = FrameDropWatch.connect(conn, host, event_port)
    if watch is None:
        warnings.warn(
            f"no event stream at {host}:{event_port}: check_frame_stats failures "
            "will give counts only, not which frames dropped",
            stacklevel=1,
        )
    yield watch
    if watch is not None:
        watch.close()


@pytest.fixture(autouse=True)
def _frame_stats_subscription(request: pytest.FixtureRequest) -> None:
    """Hand a ``check_frame_stats`` test's hook the drop subscriber.

    Requested lazily, so a run with no such test, or with the check off, never
    opens the event socket at all.
    """
    if request.node.get_closest_marker("check_frame_stats") and request.config.getoption(
        "--check-frame-stats"
    ):
        request.node.stash[_FRAME_DROP_WATCH] = request.getfixturevalue("frame_drop_watch")


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_call(item: pytest.Item):
    """Wrap a ``check_frame_stats`` test body in a frame-statistics window.

    A hook around the call rather than a fixture, so a drop fails the test
    itself instead of erroring its teardown — and so the window covers exactly
    the body: the caption going up before it and the scene reset after it are
    not the test's frames.
    """
    marker = item.get_closest_marker("check_frame_stats")
    conn = getattr(item, "funcargs", {}).get("conn")
    if marker is None or conn is None or not item.config.getoption("--check-frame-stats"):
        yield
        return

    max_dropped = max(
        marker.kwargs.get("max_dropped", 0),
        item.config.getoption("--allow-dropped-frames"),
    )
    watch = item.stash.get(_FRAME_DROP_WATCH, None)
    if watch is not None:
        watch.drain()
    conn.system.reset_frame_stats()
    outcome = yield
    stats = conn.system.reset_frame_stats()

    summary = (
        f"{stats.presented_frames} frames from frame {stats.window_start_frame}, "
        f"{stats.dropped_frames} dropped (allowed: {max_dropped}); intervals "
        f"mean {stats.mean_frame_interval_ms:.2f} ± {stats.std_frame_interval_ms:.2f} ms, "
        f"min {stats.min_frame_interval_ms:.2f} ms, max {stats.max_frame_interval_ms:.2f} ms "
        f"(nominal {stats.nominal_frame_interval_ms:.2f} ms)"
    )
    if stats.dropped_frames:
        summary += _where_frames_dropped(watch, stats)
    # On the item, not the report: pytest copies an item's user_properties into
    # every report made after this, which is how the review TUI and a JUnit XML
    # see it. Replaced rather than appended — the TUI runs one item many times.
    item.user_properties[:] = [p for p in item.user_properties if p[0] != FRAME_STATS_PROPERTY]
    item.user_properties.append((FRAME_STATS_PROPERTY, summary))

    if outcome.excinfo is not None:
        return  # the test already failed; say why, not that it also dropped
    if stats.dropped_frames > max_dropped:
        outcome.force_exception(pytest.fail.Exception(
            f"{stats.dropped_frames} frame(s) dropped during the test\n  {summary}",
            pytrace=False,
        ))


def _where_frames_dropped(watch: "FrameDropWatch | None", stats) -> str:
    """Which frames the drops fell on, from the event stream, as extra lines."""
    if watch is None:
        return "\n  no event stream: which frames dropped is unknown"
    lines = ""
    drops, lost = watch.collect(stats.dropped_frames)
    for frame, count in drops:
        offset = frame - stats.window_start_frame
        lines += f"\n  {count} dropped before frame {frame} ({offset:+d} into the test)"
    seen = sum(count for _, count in drops)
    if lost:
        lines += f"\n  {lost} drop event(s) lost on the event stream"
    if seen < stats.dropped_frames:
        lines += f"\n  events account for {seen} of {stats.dropped_frames}"
    return lines


def onscreen_marker(item: pytest.Item) -> tuple[str, str]:
    """The id and caption a test carries — its own name if it carries none.

    Shared with the review TUI, which reads the same markers to build its list.
    """
    marker = item.get_closest_marker("onscreen")
    if marker is not None:
        return marker.args[0], marker.args[1]
    doc = (getattr(item, "function", None).__doc__ or "").strip().splitlines()
    return item.name, doc[0] if doc else ""


@pytest.fixture(scope="session")
def server_address(request: pytest.FixtureRequest) -> str:
    return request.config.getoption("--server")


@pytest.fixture(scope="session")
def conn(server_address: str, request: pytest.FixtureRequest) -> Connection:
    c = Connection(server_address, recv_timeout_s=request.config.getoption("--recv-timeout"))
    # Clear any VTL names left over from a previous failed run.
    for line in c.vtl.list_lines():
        c.vtl.set_line_name(bank=line.bank, bit=line.bit, kind=line.kind, name="")
    yield c
    c.close()


@pytest.fixture
def step_delay(request: pytest.FixtureRequest) -> float:
    return request.config.getoption("--step-delay")


@pytest.fixture(autouse=True)
def stage(
    request: pytest.FixtureRequest, conn: Connection, step_delay: float
) -> Stage:
    """Caption every test on screen with its id and what should be visible.

    Autouse, so a test cannot end up running anonymously: without an
    ``onscreen`` marker the id falls back to the test's own name and the
    caption to the first line of its docstring.
    """
    test_id, description = onscreen_marker(request.node)
    deferred = False
    marker = request.node.get_closest_marker("onscreen")
    if marker is not None:
        deferred = marker.kwargs.get("deferred", False)

    s = Stage(
        conn,
        test_id,
        description,
        step_delay,
        node_id=request.node.nodeid,
        pacing=request.config.pluginmanager.get_plugin(Pacing.PLUGIN_NAME),
    )
    if not deferred:
        s.show()
    yield s
    s.close()


@pytest.fixture(autouse=True)
def scene_reset(conn: Connection, stage: Stage):
    """Hand the next test an empty scene, whatever this one left behind.

    Tests delete what they create, but a failed assertion skips the rest of the
    body — and a stimulus that is merely disabled, or a stimulus an animation
    hid, is invisible rather than gone. Without this the scene silently fills up
    over a run and later tests inherit clutter they cannot see.

    Ordered after ``stage`` so it tears down first. The caption comes down here
    rather than with the clear, so the server is never asked to delete a handle
    the clear has already taken.
    """
    yield
    stage.close()
    # Deferred mode first: left on by a test that failed half way through, it
    # would swallow every command below. `cancel` drops whatever was staged;
    # the plain "off" ends deferred mode by scheduling a flip, which is the
    # wrong thing to ask for when there may be nothing staged at all.
    conn.system.set_deferred_mode(False, cancel=True)
    conn.system.clear_all()
    # Scene-wide 3-D state is not a stimulus, so clear_all leaves it alone.
    conn.system.set_camera(Camera3D())
    conn.system.set_lighting(Lighting3D())
    conn.system.set_camera_zones([])
    for anim in conn.animations.list_animations():
        conn.animations.delete(anim.handle)
    for line in conn.vtl.list_lines():
        conn.vtl.set_line_name(line.bank, line.bit, line.kind, name="")
    conn.system.set_background(0.0, 0.0, 0.0)


def reachable(address: str, timeout_ms: int = 500) -> bool:
    ctx = zmq.Context.instance()
    sock = ctx.socket(zmq.REQ)
    sock.setsockopt(zmq.LINGER, 0)
    sock.setsockopt(zmq.RCVTIMEO, timeout_ms)
    sock.connect(address)
    try:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_server_info=system_pb2.QueryServerInfoRequest(),
        )
        sock.send(req.SerializeToString())
        sock.recv()
        return True
    except zmq.Again:
        return False
    finally:
        sock.close()
