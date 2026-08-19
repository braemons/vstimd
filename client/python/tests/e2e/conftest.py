"""Shared pytest configuration and fixtures for e2e tests."""

import os

import pytest
import zmq

from vstimd import Connection
from vstimd._proto import service_pb2, system_pb2

from .cases._helpers import Stage

_E2E_DEFAULT = os.environ.get("VSTIMD_SERVER", "tcp://localhost:5555")


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--server",
        default=_E2E_DEFAULT,
        help=f"ZMQ address of the vstimd for e2e tests (default: {_E2E_DEFAULT})",
    )
    parser.addoption(
        "--step-delay",
        type=float,
        default=1.0,
        help="Seconds to hold each visual state so a human can inspect it "
        "(default: 1.0). The null suites pin it to 0",
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
def conn(server_address: str) -> Connection:
    c = Connection(server_address)
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

    s = Stage(conn, test_id, description, step_delay, node_id=request.node.nodeid)
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
    # would swallow every command below.
    conn.system.set_deferred_mode(False)
    conn.system.clear_all()
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
