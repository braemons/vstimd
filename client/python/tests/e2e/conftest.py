"""Shared pytest configuration and fixtures for e2e tests."""

import dataclasses
import datetime
import json
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
        "--start-at",
        type=int,
        default=0,
        help="Skip straight to test number N (see the numbering the pause "
        "prompt shows). Set by the browser when jumping backwards",
    )
    parser.addoption(
        "--nav-file",
        default="",
        help="JSON file the pause prompt hands a backward jump to, for the "
        "browser (tests/e2e/browse.py) to act on. Without it, only forward "
        "jumps are possible: pytest cannot re-run a test it has passed",
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


@dataclasses.dataclass
class Entry:
    """One test in run order: its number, its id and what it should show."""

    index: int
    node_id: str
    test_id: str
    summary: str


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
        #: Every test in run order, so the prompt can say where it is and jump.
        self.entries: list[Entry] = []
        self.index = 0
        self.total = 0
        #: Set by a forward jump; tests before it are skipped rather than run.
        self.skip_until: int | None = None
        #: Set only when this session's prompt asked to go back; anything left
        #: over from an earlier session is spent and must not fire again.
        self.pending_jump: int | None = None
        self.outcomes = _LAST_OUTCOMES
        self.session: pytest.Session | None = None
        nav = config.getoption("--nav-file")
        self.nav_file = pathlib.Path(nav) if nav else None
        self._load()

    # ── state carried across a restart ───────────────────────────────────────

    def _load(self) -> None:
        """Pick up the notes taken before a backward jump restarted pytest."""
        if self.nav_file is None or not self.nav_file.exists():
            return
        state = json.loads(self.nav_file.read_text(encoding="utf-8"))
        self.flags = [Flag(**f) for f in state.get("flags", [])]

    def save(self, extra: dict | None = None) -> None:
        if self.nav_file is None:
            return
        state = {"flags": [dataclasses.asdict(f) for f in self.flags], "jump": None}
        state.update(extra or {})
        self.nav_file.write_text(json.dumps(state, indent=2), encoding="utf-8")

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

    # ── what the prompt shows ────────────────────────────────────────────────

    def _progress(self) -> str:
        """`[ 12/147 ] ███░░░░░░░░` — where in the suite this test is."""
        if not self.total:
            return ""
        done = self.index / self.total
        bar = "█" * round(done * 12) + "░" * (12 - round(done * 12))
        return f"  [{self.index:>3}/{self.total}] {bar} {done * 100:3.0f}%"

    _KEYS = (
        "     j/⏎ next   k prev   5j/5k ±5   42G go to 42   gg/G first/last\n"
        "     r replay   /text search   l list   f flag   c run on   q quit"
    )

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
        outcome = self.outcomes.get(stage.node_id, "")
        mark = {"passed": "✓", "failed": "✗ FAILED", "skipped": "– skipped"}.get(
            outcome, ""
        )
        while True:
            answer = self._ask(
                f"\n{self._progress()}\n"
                f"  ⏸  [{stage.test_id}] {mark}  {stage.summary}\n"
                f"{on_screen}"
                f"{self._KEYS}\n"
                f"     > "
            )
            if answer is None:
                return
            if self._act(answer.strip(), stage):
                return

    # ── acting on a key ──────────────────────────────────────────────────────

    def _act(self, answer: str, stage: Stage) -> bool:
        """Handle one keypress. True when the prompt is done and the run goes on.

        The keys are vim's, because that is the muscle memory this audience
        already has: counts prefix the motion (`5j`), `G` takes a line number,
        `/` searches.
        """
        if answer in ("", "j", "n"):
            return True
        if answer in ("c", ":c"):
            self.mode = "off"
            return True
        if answer in ("q", ":q", ":q!"):
            self._stop(f"stopped at [{stage.test_id}] by request")
            return True
        if answer == "f":
            self._flag(stage)
            return False  # back to the prompt: the frame is still up
        if answer in ("?", ":h", ":help"):
            self._say(self._KEYS)
            return False
        if answer in ("l", ":ls", ":list"):
            self._list()
            return False
        if answer.startswith("/"):
            found = self._search(answer[1:].strip())
            return self._go(found, stage) if found else False
        return self._motion(answer, stage)

    def _motion(self, answer: str, stage: Stage) -> bool:
        """`k`, `5j`, `gg`, `G`, `42G`, `:42`, `42`, `r` — or nothing we know."""
        target: int | None = None
        if answer == "gg":
            target = 1
        elif answer == "G":
            target = self.total
        elif answer == "r":
            target = self.index
        elif answer in ("k", "p"):
            target = self.index - 1
        else:
            # `:42` is vim's line jump written the ex way; `42` alone means the
            # same thing here, since there is nothing else a bare number can be.
            raw = answer.removeprefix(":")
            digits = raw[: len(raw) - len(raw.lstrip("0123456789"))]
            motion = raw[len(digits):]
            if digits:
                count = int(digits)
                if motion == "j":
                    target = self.index + count
                elif motion == "k":
                    target = self.index - count
                elif motion in ("", "G"):
                    target = count
        if target is None:
            self._say(f"     ? {answer!r} is not a key here\n{self._KEYS}")
            return False
        return self._go(target, stage)

    def _go(self, target: int, stage: Stage) -> bool:
        """Move to test number ``target``, forwards in this run or by restarting."""
        target = max(1, min(target, self.total))
        if target == self.index + 1:
            return True                      # just carry on
        if target > self.index:
            self.skip_until = target          # skipped over as the session runs on
            self._say(f"     → jumping to {target}")
            return True
        if self.nav_file is None:
            self._say(
                "     ← going back needs the browser: pytest cannot re-run a "
                "test it has already passed.\n"
                "       Run `make test-e2e-browse` instead of this target."
            )
            return False
        self.pending_jump = target
        self.save({"jump": target})
        self._stop(f"jumping back to test {target}")
        return True

    def _stop(self, why: str) -> None:
        """End the session after this test, rather than in the middle of it.

        ``pytest.exit`` here would abandon the teardown this prompt is running
        inside, and with it the scene reset — leaving stimuli, a background
        colour or deferred mode behind for whatever runs next. ``shouldstop`` is
        checked between tests, so the current one finishes cleaning up first.
        """
        self._say(f"     {why}")
        if self.session is not None:
            self.session.shouldstop = why
        else:  # no session to ask: nothing has been set up to leave behind
            pytest.exit(why, returncode=0)

    def _search(self, text: str) -> int | None:
        """Number of the next test whose id or caption contains ``text``."""
        if not text:
            return None
        order = self.entries[self.index:] + self.entries[: self.index]
        for entry in order:
            haystack = f"{entry.test_id} {entry.summary}".lower()
            if text.lower() in haystack:
                return entry.index
        self._say(f"     ? nothing matches {text!r}")
        return None

    def _list(self) -> None:
        lines = []
        for entry in self.entries:
            here = "→" if entry.index == self.index else " "
            flagged = "✗" if any(f.node_id == entry.node_id for f in self.flags) else " "
            lines.append(
                f"   {here}{flagged} {entry.index:>3}  [{entry.test_id}]  {entry.summary[:60]}"
            )
        self._say("\n".join(lines))

    # ── the two pause points ─────────────────────────────────────────────────

    def at_step(self, stage: Stage) -> None:
        self._prompt(stage, "step")

    def at_test(self, stage: Stage, index: int) -> None:
        self.index = index
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
_INDEX = pytest.StashKey[int]()

#: How each test came out, by node id — filled in as the reports come through.
_LAST_OUTCOMES: dict[str, str] = {}


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


def pytest_sessionstart(session: pytest.Session) -> None:
    session.config.stash[_REVIEWER].session = session


def pytest_collection_modifyitems(
    config: pytest.Config, items: list[pytest.Item]
) -> None:
    """Number the tests in run order, and drop the ones before --start-at.

    The numbering is over everything collected, so it does not shift when a run
    starts in the middle: test 42 is test 42 whichever way it was reached.
    """
    reviewer = config.stash[_REVIEWER]
    for number, item in enumerate(items, start=1):
        item.stash[_INDEX] = number
        marker = item.get_closest_marker("onscreen")
        test_id = marker.args[0] if marker else item.name
        summary = marker.args[1] if marker else ""
        reviewer.entries.append(Entry(number, item.nodeid, test_id, summary))
    reviewer.total = len(items)

    start = config.getoption("--start-at")
    if start > 1:
        skipped, kept = items[: start - 1], items[start - 1:]
        config.hook.pytest_deselected(items=skipped)
        items[:] = kept


def pytest_runtest_setup(item: pytest.Item) -> None:
    """Step over the tests a forward jump asked to skip."""
    reviewer = item.config.stash[_REVIEWER]
    if reviewer.skip_until is None:
        return
    if item.stash.get(_INDEX, 0) < reviewer.skip_until:
        pytest.skip(f"jumped over, on the way to test {reviewer.skip_until}")
    reviewer.skip_until = None


@pytest.hookimpl(trylast=True)
def pytest_runtest_logreport(report: pytest.TestReport) -> None:
    """Remember how each test came out, so the prompt can say so."""
    if report.when == "call" or (report.when == "setup" and report.outcome != "passed"):
        _LAST_OUTCOMES[report.nodeid] = report.outcome


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


def pytest_sessionfinish(session: pytest.Session) -> None:
    """Leave the notes where a restarted session can pick them up again."""
    reviewer = session.config.stash.get(_REVIEWER, None)
    if reviewer is None or reviewer.nav_file is None:
        return
    # Only a jump this session's prompt asked for counts. Carrying an older one
    # over would send the browser back to the same test for ever.
    reviewer.save({"jump": reviewer.pending_jump})


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
    s.close()


@pytest.fixture(autouse=True)
def scene_reset(
    request: pytest.FixtureRequest, conn: Connection, stage: Stage, reviewer: Reviewer
):
    """Hand the next test an empty scene, whatever this one left behind.

    Tests delete what they create, but a failed assertion skips the rest of the
    body — and a stimulus that is merely disabled, or a stimulus an animation
    hid, is invisible rather than gone. Without this the scene silently fills up
    over a run and later tests inherit clutter they cannot see.

    Ordered after ``stage`` so it tears down first: the caption is deleted by
    the clear, and ``Stage.close`` copes with its handle already being gone.

    The per-test pause happens here too, before the clearing: pausing after it
    would offer a blank screen to study.
    """
    yield
    reviewer.at_test(stage, request.node.stash.get(_INDEX, 0))
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
