"""Shared pytest configuration and fixtures for e2e tests."""

import dataclasses
import datetime
import os
import pathlib
import sys

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
        help="Seconds to pause between visual stimulus changes so a human can inspect them (default: 1.0)",
    )
    parser.addoption(
        "--pause",
        nargs="?",
        const="test",
        default="off",
        choices=["off", "test", "step"],
        help="Wait for a keypress while the suite runs, so a frame can be "
        "studied for as long as it takes and anything wrong with it written "
        "down: 'test' (the default when the flag is given without a value) "
        "pauses once per test, 'step' pauses at every caption change "
        "(default: off)",
    )
    parser.addoption(
        "--review-log",
        default="e2e-review.md",
        help="Where to write the notes taken while pausing. Written only if "
        "something was flagged (default: e2e-review.md)",
    )


@dataclasses.dataclass
class Flag:
    """One test an operator marked as wrong, and what they said about it."""

    test_id: str
    description: str
    node_id: str
    note: str


class Reviewer:
    """The keyboard console the on-screen suite is watched through.

    At each pause it shows the id and caption of what is on screen and waits.
    The point of the wait is not only to look: `f` writes the test down as
    problematic, with a note, and the run carries on — so a whole review pass
    produces one list at the end instead of a scribbled page of ids.

    pytest owns stdin and stdout while a test runs, so the prompt has to
    suspend capturing for as long as it is waiting — otherwise it is neither
    printed nor answerable. A run with no terminal behind it (CI, a backgrounded
    make) has no one to ask, so the first EOF turns pausing off for good.
    """

    def __init__(self, config: pytest.Config) -> None:
        self.config = config
        self.mode = config.getoption("--pause")
        self.flags: list[Flag] = []

    # ── prompting ────────────────────────────────────────────────────────────

    def _ask(self, prompt: str) -> str | None:
        """Ask on the real terminal, or return None if there is not one."""
        capture = self.config.pluginmanager.getplugin("capturemanager")
        if capture is not None:
            capture.suspend_global_capture(in_=True)
        try:
            return input(prompt)
        except (EOFError, OSError):
            self.mode = "off"  # nothing on stdin to ask
            return None
        finally:
            if capture is not None:
                capture.resume_global_capture()

    def _say(self, message: str) -> None:
        """Write to the real terminal, around whatever pytest is capturing."""
        capture = self.config.pluginmanager.getplugin("capturemanager")
        if capture is not None:
            capture.suspend_global_capture(in_=True)
        try:
            print(message, file=sys.stderr, flush=True)
        finally:
            if capture is not None:
                capture.resume_global_capture()

    def _prompt(self, stage: Stage, where: str) -> None:
        if self.mode != where:
            return
        # The caption on screen is the state the test is in right now; the
        # summary is what the test as a whole is for. They differ often enough
        # that showing only one leaves the operator guessing.
        on_screen = (
            f"     on screen: {stage.description}\n"
            if stage.description != stage.summary
            else ""
        )
        while True:
            answer = self._ask(
                f"\n  {'─' * 68}\n"
                f"  ⏸  [{stage.test_id}]  {stage.summary}\n"
                f"{on_screen}"
                f"     [Enter] next   [f] flag a problem   "
                f"[c] run on   [q] quit: "
            )
            if answer is None:
                return
            answer = answer.strip().lower()
            if answer == "f":
                self._flag(stage)
                continue  # back to the prompt: the frame is still up
            if answer == "c":
                self.mode = "off"
            elif answer == "q":
                pytest.exit(f"stopped at [{stage.test_id}] by request", returncode=0)
            return

    def _flag(self, stage: Stage) -> None:
        note = self._ask(f"     what is wrong with [{stage.test_id}]? ")
        if note is None:
            return
        self.flags.append(
            Flag(stage.test_id, stage.summary, stage.node_id, note.strip())
        )
        # Straight to the terminal: a print here would land in pytest's capture
        # buffer and only surface if the test went on to fail.
        self._say(f"     ✗ flagged [{stage.test_id}]")

    # ── the two pause points ─────────────────────────────────────────────────

    def at_step(self, stage: Stage) -> None:
        self._prompt(stage, "step")

    def at_test(self, stage: Stage) -> None:
        self._prompt(stage, "test")

    # ── the report ───────────────────────────────────────────────────────────

    def report(self) -> str:
        """The flagged tests as markdown, with the command to re-run just them."""
        when = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
        lines = [f"# vstimd on-screen review — {when}", ""]
        for flag in self.flags:
            lines += [
                f"## [{flag.test_id}] {flag.note or '(no note)'}",
                "",
                f"- expected on screen: {flag.description}",
                f"- test: `{flag.node_id}`",
                "",
            ]
        node_ids = " ".join(f'"{f.node_id}"' for f in self.flags)
        lines += ["Re-run just these:", "", "```bash", f"uv run pytest {node_ids}", "```", ""]
        return "\n".join(lines)


_REVIEWER = pytest.StashKey[Reviewer]()


def pytest_configure(config: pytest.Config) -> None:
    config.stash[_REVIEWER] = Reviewer(config)
    config.addinivalue_line(
        "markers",
        "onscreen(test_id, description, deferred=False): the id an operator "
        "writes down for this test and the caption it shows while it runs. Set "
        "deferred=True when the caption would disturb what the test checks (a "
        "test that compares the whole scene); such a test puts the caption up "
        "itself, with stage.step(), once it is past that point.",
    )


def pytest_terminal_summary(
    terminalreporter: pytest.TerminalReporter, config: pytest.Config
) -> None:
    """List what was flagged, and leave it on disk to act on later."""
    reviewer = config.stash.get(_REVIEWER, None)
    if reviewer is None or not reviewer.flags:
        return
    terminalreporter.write_sep("=", f"on-screen review: {len(reviewer.flags)} flagged")
    for flag in reviewer.flags:
        terminalreporter.write_line(f"[{flag.test_id}] {flag.note or '(no note)'}")
        terminalreporter.write_line(f"    {flag.node_id}")
    out = pathlib.Path(config.getoption("--review-log"))
    out.write_text(reviewer.report(), encoding="utf-8")
    terminalreporter.write_line(f"\nwritten to {out}")


@pytest.fixture(scope="session")
def server_address(request: pytest.FixtureRequest) -> str:
    return request.config.getoption("--server")


@pytest.fixture(scope="session")
def reviewer(request: pytest.FixtureRequest) -> Reviewer:
    return request.config.stash[_REVIEWER]


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
    request: pytest.FixtureRequest,
    conn: Connection,
    step_delay: float,
    reviewer: Reviewer,
) -> Stage:
    """Caption every test on screen with its id and what should be visible.

    Autouse, so a test cannot end up running anonymously: without an
    ``onscreen`` marker the id falls back to the test's own name and the
    caption to the first line of its docstring.
    """
    marker = request.node.get_closest_marker("onscreen")
    if marker is not None:
        test_id, description = marker.args[0], marker.args[1]
        deferred = marker.kwargs.get("deferred", False)
    else:
        doc = (request.node.function.__doc__ or "").strip().splitlines()
        test_id, description = request.node.name, doc[0] if doc else ""
        deferred = False

    s = Stage(
        conn,
        test_id,
        description,
        step_delay,
        pause=reviewer.at_step,
        node_id=request.node.nodeid,
    )
    if not deferred:
        s.show()
    yield s
    reviewer.at_test(s)
    s.close()


@pytest.fixture(autouse=True)
def scene_reset(conn: Connection, stage: Stage):
    """Hand the next test an empty scene, whatever this one left behind.

    Tests delete what they create, but a failed assertion skips the rest of the
    body — and a stimulus that is merely disabled, or a stimulus an animation
    hid, is invisible rather than gone. Without this the scene silently fills up
    over a run and later tests inherit clutter they cannot see.

    Ordered after ``stage`` so it tears down first: the caption is deleted by
    the clear, and ``Stage.close`` copes with its handle already being gone.
    """
    yield
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
