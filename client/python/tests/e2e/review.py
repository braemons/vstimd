"""A terminal front end for reviewing the on-screen e2e suite by eye.

    cd client/python && make test-e2e-review

The suite renders stimuli on a real display; judging whether they look right is
a human job, and the terminal side of that job is picking what to run, watching
it, and writing down what was wrong. That is a UI, not a prompt — so this is a
Textual app, and pytest runs underneath it.

pytest still owns collection, fixtures and reporting: this plugin takes over
`pytest_runtestloop` and runs whichever item the app asks for, in whatever
order, as often as asked. Passing a *next* item to the protocol keeps
session-scoped fixtures (the server connection) alive between runs; only what
the test itself set up is torn down.

Two pytest settings keep the terminal to ourselves: `-p no:terminal` silences
pytest's own reporting, and `--capture=sys` captures at the Python level rather
than the file-descriptor level, leaving the real stdout for Textual to draw on.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime
import json
import pathlib
import subprocess
import sys
import time

import pytest
from textual import on, work
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import (
    DataTable,
    Footer,
    Header,
    Input,
    Label,
    ProgressBar,
    Static,
    TabbedContent,
    TabPane,
)

from vstimd import Connection
from vstimd.tui import ServerStatus, StimulusList, TriggerLines

sys.path.insert(0, str(pathlib.Path(__file__).parents[2]))

from tests.e2e.conftest import onscreen_marker, reachable  # noqa: E402

_PYTHON_CLIENT = pathlib.Path(__file__).parents[2]
_REPO_ROOT = _PYTHON_CLIENT.parents[1]
_SUITES = ["tests/e2e/test_e2e.py", "tests/e2e/test_psychopy_visual.py"]

_STATUS = {"passed": "✓", "failed": "✗", "skipped": "–", "": "", "running": "…"}


@dataclasses.dataclass
class Entry:
    """One test: what it is, how it last went, and what was said about it."""

    index: int
    node_id: str
    test_id: str
    summary: str
    status: str = ""
    note: str | None = None
    failure: str = ""

    @property
    def flagged(self) -> bool:
        return self.note is not None


class Session:
    """The review so far, on disk, so a pass can be picked up where it stopped.

    Judging 147 stimuli by eye is not one sitting. What survives is the part a
    person produced — which tests have been looked at, how they came out, and
    what was said about them — keyed by the stable test id rather than pytest's
    node id, so renaming a test function does not lose its notes.

    Records for tests this run did not collect (a `-k` selection, a test since
    deleted) are carried through untouched rather than dropped: an unrelated
    selection must not throw away yesterday's notes.
    """

    VERSION = 1

    def __init__(self, path: pathlib.Path) -> None:
        self.path = path
        self.unmatched: dict[str, dict] = {}
        self.updated: str | None = None

    def load(self, entries: list[Entry]) -> int:
        """Restore what is on disk onto ``entries``; return how many matched."""
        if not self.path.exists():
            return 0
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return 0
        if data.get("version") != self.VERSION:
            return 0

        self.updated = data.get("updated")
        saved = dict(data.get("tests", {}))
        restored = 0
        for entry in entries:
            record = saved.pop(entry.test_id, None)
            if record is None:
                continue
            entry.status = record.get("status", "")
            entry.note = record.get("note")
            entry.failure = record.get("failure", "")
            restored += 1
        self.unmatched = saved
        return restored

    def save(self, entries: list[Entry]) -> None:
        tests = dict(self.unmatched)
        for entry in entries:
            if not entry.status and not entry.flagged:
                continue  # nothing worth remembering about an untouched test
            tests[entry.test_id] = {
                "status": entry.status,
                "note": entry.note,
                "failure": entry.failure,
                "node_id": entry.node_id,
            }
        payload = {
            "version": self.VERSION,
            "updated": datetime.datetime.now().isoformat(timespec="seconds"),
            "tests": tests,
        }
        try:
            self.path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        except OSError:
            pass  # a review that cannot save is still a review worth finishing


class PromptScreen(ModalScreen[str | None]):
    """One line of typing — a note on a flagged test, or something to search for.

    Escape backs out with None, which is the difference between "no note" and
    "never mind": an empty string still flags the test.
    """

    BINDINGS = [Binding("escape", "cancel", "cancel")]

    def __init__(self, title: str, subtitle: str = "", value: str = "",
                 placeholder: str = "") -> None:
        super().__init__()
        self.prompt_title = title
        self.subtitle = subtitle
        self.value = value
        self.placeholder = placeholder

    def compose(self) -> ComposeResult:
        with Vertical(id="note-box"):
            yield Label(self.prompt_title)
            if self.subtitle:
                yield Label(self.subtitle, id="note-summary")
            yield Input(
                value=self.value, placeholder=self.placeholder, id="note-input"
            )

    def on_mount(self) -> None:
        self.query_one(Input).focus()

    @on(Input.Submitted)
    def _submit(self, event: Input.Submitted) -> None:
        self.dismiss(event.value.strip())

    def action_cancel(self) -> None:
        self.dismiss(None)


class ReviewApp(App[None]):
    """The list of tests, what each should show, and what you made of it."""

    CSS = """
    #body { height: 1fr; }
    #list { width: 3fr; }
    #side { width: 2fr; }
    #detail { padding: 0 1; }
    #status { height: auto; padding: 0 1; }
    #note-box {
        padding: 1 2; width: 80%; height: auto;
        background: $surface; border: thick $accent;
    }
    #note-summary { color: $text-muted; padding-bottom: 1; }
    .caption { color: $text-muted; }
    """

    BINDINGS = [
        Binding("j,down", "move(1)", "down", show=False),
        Binding("k,up", "move(-1)", "up", show=False),
        Binding("enter", "run_and_advance", "run + next"),
        Binding("space", "run_current", "run"),
        Binding("r", "run_current", "replay"),
        Binding("a", "run_all", "run from here"),
        Binding("s,escape", "stop", "stop"),
        Binding("f", "flag", "flag"),
        Binding("u", "unflag", "unflag", show=False),
        Binding("g", "top", "first", show=False),
        Binding("G", "bottom", "last", show=False),
        Binding("slash", "search", "search", show=False),
        Binding("w", "write_report", "write notes"),
        Binding("R", "resume", "first unrun", show=False),
        Binding("S", "save", "save progress", show=False),
        Binding("v", "next_tab", "scene / triggers"),
        Binding("q", "quit", "quit"),
    ]

    def __init__(
        self,
        driver: "PytestDriver",
        report_path: pathlib.Path,
        server_address: str = "tcp://localhost:5555",
        session: Session | None = None,
    ) -> None:
        super().__init__()
        self.driver = driver
        self.report_path = report_path
        self.entries = driver.entries
        self.session = session or Session(pathlib.Path(".e2e-review-session.json"))
        self.restored = self.session.load(self.entries)
        self.running = False
        self.continuous = False
        # The app's own connection: the tests run on a worker thread with the
        # session's connection, and a ZMQ socket belongs to one thread at a time.
        # With a receive timeout, because these panels poll from the UI thread —
        # a server that stops answering must not take the interface with it.
        self.connection = Connection(server_address, recv_timeout_s=2.0)

    # ── layout ───────────────────────────────────────────────────────────────

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="body"):
            yield DataTable(id="list", cursor_type="row", zebra_stripes=True)
            with TabbedContent(id="side"):
                with TabPane("test", id="tab-test"):
                    yield Static(id="detail")
                with TabPane("scene", id="tab-scene"):
                    yield StimulusList(self.connection, id="scene")
                with TabPane("triggers", id="tab-triggers"):
                    yield TriggerLines(self.connection, id="triggers")
        with Vertical(id="status"):
            yield ServerStatus(self.connection)
            yield ProgressBar(total=max(1, len(self.entries)), id="progress")
            yield Static(id="status-line")
        yield Footer()

    def on_mount(self) -> None:
        self.title = "vstimd on-screen review"
        table = self.query_one(DataTable)
        table.add_columns("#", "id", "", "should show")
        for entry in self.entries:
            table.add_row(*self._row(entry), key=entry.node_id)
        table.focus()
        self.action_resume()
        self._refresh_detail()
        if self.restored:
            when = f" (saved {self.session.updated})" if self.session.updated else ""
            self._refresh_status(f"resumed {self.restored} test(s) from earlier{when}")
        else:
            self._refresh_status("ready — ⏎ runs the selected test on the display")

    # ── the table ────────────────────────────────────────────────────────────

    def _row(self, entry: Entry) -> tuple[str, str, str, str]:
        mark = "✗" if entry.flagged else _STATUS.get(entry.status, "")
        return (str(entry.index), entry.test_id, mark, entry.summary)

    def _update_row(self, entry: Entry) -> None:
        table = self.query_one(DataTable)
        for column, value in zip(table.columns, self._row(entry)):
            table.update_cell(entry.node_id, column, value)

    @property
    def current(self) -> Entry:
        return self.entries[self.query_one(DataTable).cursor_row]

    @on(DataTable.RowHighlighted)
    def _highlighted(self) -> None:
        self._refresh_detail()

    @on(DataTable.RowSelected)
    def _selected(self) -> None:
        # The table takes Enter for itself, so the binding alone never fires.
        self.action_run_and_advance()

    def _refresh_detail(self) -> None:
        entry = self.current
        lines = [
            f"[b]\\[{entry.test_id}][/b]  test {entry.index} of {len(self.entries)}",
            "",
            f"[i]should show:[/i] {entry.summary}",
            "",
            f"[dim]{entry.node_id}[/dim]",
        ]
        if entry.status:
            lines += ["", f"last run: {_STATUS.get(entry.status, '')} {entry.status}"]
        if entry.flagged:
            lines += ["", f"[b]flagged:[/b] {entry.note or '(no note)'}"]
        if entry.failure:
            lines += ["", "[b]failure[/b]", entry.failure[-2000:]]
        self.query_one("#detail", Static).update("\n".join(lines))

    def _refresh_status(self, message: str) -> None:
        done = sum(1 for e in self.entries if e.status)
        flagged = sum(1 for e in self.entries if e.flagged)
        self.query_one(ProgressBar).update(progress=done)
        self.query_one("#status-line", Static).update(
            f"{done}/{len(self.entries)} run   {flagged} flagged   {message}"
        )

    # ── running ──────────────────────────────────────────────────────────────

    def action_move(self, delta: int) -> None:
        table = self.query_one(DataTable)
        table.move_cursor(row=max(0, min(len(self.entries) - 1, table.cursor_row + delta)))

    def action_resume(self) -> None:
        """Jump to the first test nobody has looked at yet."""
        for entry in self.entries:
            if not entry.status:
                self.query_one(DataTable).move_cursor(row=entry.index - 1)
                return
        self.query_one(DataTable).move_cursor(row=0)

    def action_save(self) -> None:
        self.session.save(self.entries)
        self._refresh_status(f"progress saved to {self.session.path}")

    def action_top(self) -> None:
        self.query_one(DataTable).move_cursor(row=0)

    def action_bottom(self) -> None:
        self.query_one(DataTable).move_cursor(row=len(self.entries) - 1)

    def action_run_current(self) -> None:
        self._run(self.current, advance=False)

    def action_run_and_advance(self) -> None:
        self._run(self.current, advance=True)

    def action_run_all(self) -> None:
        """Run from here to the end, until something is flagged or `s` is hit."""
        self.continuous = True
        self._run(self.current, advance=True)

    def action_stop(self) -> None:
        if self.continuous:
            self.continuous = False
            self._refresh_status("stopped — ⏎ to carry on one at a time")

    def _run(self, entry: Entry, advance: bool) -> None:
        if self.running:
            return
        self.running = True
        entry.status = "running"
        self._update_row(entry)
        self._refresh_status(f"running [{entry.test_id}] — watch the display")
        self._run_worker(entry, advance)

    @work(thread=True)
    def _run_worker(self, entry: Entry, advance: bool) -> None:
        """Run one test on the pytest side, off the UI thread."""
        outcome, failure = self.driver.run(entry.node_id)
        self.call_from_thread(self._finished, entry, outcome, failure, advance)

    def _finished(self, entry: Entry, outcome: str, failure: str, advance: bool) -> None:
        entry.status = outcome
        entry.failure = failure
        self._update_row(entry)
        self.running = False
        self._refresh_detail()
        self.session.save(self.entries)
        self._refresh_status(f"[{entry.test_id}] {outcome}")
        if advance and entry.index < len(self.entries):
            self.action_move(1)
        if self.continuous and entry.index < len(self.entries):
            self._run(self.current, advance=True)
        elif self.continuous:
            self.continuous = False

    # ── notes ────────────────────────────────────────────────────────────────

    def action_flag(self) -> None:
        entry = self.current
        self.push_screen(
            PromptScreen(
                f"What is wrong with [{entry.test_id}]?",
                entry.summary,
                entry.note or "",
                "…leave empty to flag it without a note",
            ),
            lambda note: self._flagged(entry, note),
        )

    def _flagged(self, entry: Entry, note: str | None) -> None:
        if note is None:
            return
        entry.note = note
        self._update_row(entry)
        self._refresh_detail()
        self.session.save(self.entries)
        self._refresh_status(f"flagged [{entry.test_id}]")

    def action_unflag(self) -> None:
        entry = self.current
        entry.note = None
        self._update_row(entry)
        self._refresh_detail()
        self.session.save(self.entries)
        self._refresh_status(f"un-flagged [{entry.test_id}]")

    def action_next_tab(self) -> None:
        """Cycle the side panel: what the test claims, what the scene holds."""
        tabs = self.query_one(TabbedContent)
        order = ["tab-test", "tab-scene", "tab-triggers"]
        current = order.index(tabs.active) if tabs.active in order else 0
        tabs.active = order[(current + 1) % len(order)]

    def action_search(self) -> None:
        self.push_screen(
            PromptScreen("Find a test", placeholder="id or words from its caption"),
            self._search,
        )

    def _search(self, text: str | None) -> None:
        if not text:
            return
        table = self.query_one(DataTable)
        order = self.entries[table.cursor_row + 1:] + self.entries[: table.cursor_row + 1]
        for entry in order:
            if text.lower() in f"{entry.test_id} {entry.summary}".lower():
                table.move_cursor(row=entry.index - 1)
                self._refresh_status(f"found [{entry.test_id}]")
                return
        self._refresh_status(f"nothing matches {text!r}")

    # ── the report ───────────────────────────────────────────────────────────

    def action_write_report(self) -> None:
        path = self.write_report()
        self._refresh_status(f"notes written to {path}" if path else "nothing flagged")

    def write_report(self) -> pathlib.Path | None:
        flagged = [e for e in self.entries if e.flagged]
        if not flagged:
            return None
        when = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
        lines = [f"# vstimd on-screen review — {when}", ""]
        for entry in flagged:
            lines += [
                f"## [{entry.test_id}] {entry.note or '(no note)'}",
                "",
                f"- should show: {entry.summary}",
                f"- test: `{entry.node_id}`",
                "",
            ]
        node_ids = " ".join(f'"{e.node_id}"' for e in flagged)
        lines += ["Re-run just these:", "", "```bash", f"uv run pytest {node_ids}", "```", ""]
        self.report_path.write_text("\n".join(lines), encoding="utf-8")
        return self.report_path

    def on_unmount(self) -> None:
        self.connection.close()
        self.session.save(self.entries)
        if self.write_report():
            print(f"notes written to {self.report_path}", file=sys.stderr)


class PytestDriver:
    """The pytest side: collect once, then run whatever the app asks for."""

    def __init__(self) -> None:
        self.items: list[pytest.Item] = []
        self.entries: list[Entry] = []
        self.report_path = pathlib.Path("e2e-review.md")
        self.session = Session(pathlib.Path(".e2e-review-session.json"))
        self._outcome = ("", "")

    # ── hooks ────────────────────────────────────────────────────────────────

    def pytest_collection_modifyitems(self, items: list[pytest.Item]) -> None:
        self.items = list(items)
        self.entries = [
            Entry(number, item.nodeid, *onscreen_marker(item))
            for number, item in enumerate(items, start=1)
        ]

    def pytest_runtest_logreport(self, report: pytest.TestReport) -> None:
        # setup errors and xfails come through phases other than the call, so
        # take the first thing that is not a plain pass as the outcome.
        if report.when == "call" or (report.when == "setup" and report.outcome != "passed"):
            failure = str(report.longrepr) if report.failed else ""
            self._outcome = (report.outcome, failure)

    def pytest_runtestloop(self, session: pytest.Session) -> bool:
        if not self.items:
            return True
        address = session.config.getoption("--server")
        ReviewApp(self, self.report_path, address, self.session).run()
        return True

    # ── what the app calls ───────────────────────────────────────────────────

    def run(self, node_id: str) -> tuple[str, str]:
        item = next(i for i in self.items if i.nodeid == node_id)
        position = self.items.index(item)
        # A *next* item that shares the session keeps session-scoped fixtures
        # alive; with None, pytest would tear the server connection down after
        # every single test.
        nextitem = self.items[(position + 1) % len(self.items)]
        self._outcome = ("", "")
        item.config.hook.pytest_runtest_protocol(item=item, nextitem=nextitem)
        return self._outcome


def _start_server(address: str, window: str | None, log: pathlib.Path):
    """Start a server for the session, unless one is already listening."""
    if reachable(address):
        print(f"review: using the server already at {address}")
        return None
    binary = _REPO_ROOT / "target" / "release" / (
        "vstimd.exe" if sys.platform == "win32" else "vstimd"
    )
    if not binary.exists():
        if subprocess.run(["cargo", "build", "--release"], cwd=_REPO_ROOT).returncode:
            sys.exit("review: cargo build --release failed")
    argv = [str(binary)] + (["--windowed", window] if window else [])
    print(f"review: starting {' '.join(argv)}  (log: {log})")
    # Into a file, not the terminal: the app is about to draw on it.
    handle = log.open("w")
    proc = subprocess.Popen(argv, stdout=handle, stderr=handle)
    for _ in range(40):
        if reachable(address):
            return proc
        time.sleep(0.5)
    proc.terminate()
    sys.exit("review: the server did not come up")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Review the on-screen e2e suite.")
    parser.add_argument("--server", default="tcp://localhost:5555")
    parser.add_argument(
        "--windowed",
        default="1280x720",
        help="Server window size (default: 1280x720); --fullscreen overrides it",
    )
    parser.add_argument("--fullscreen", action="store_true")
    parser.add_argument("--step-delay", default="1.0")
    parser.add_argument("--review-log", default="e2e-review.md")
    parser.add_argument(
        "--session",
        default=".e2e-review-session.json",
        help="Where progress is kept between sittings (default: "
        ".e2e-review-session.json). Deleting it starts the review over",
    )
    parser.add_argument(
        "--fresh",
        action="store_true",
        help="Ignore saved progress and start the review from nothing",
    )
    parser.add_argument(
        "--server-log", default="vstimd-review.log", help="Where the server's output goes"
    )
    parser.add_argument("suites", nargs="*", default=_SUITES)
    args = parser.parse_args(argv)

    server = _start_server(
        args.server,
        None if args.fullscreen else args.windowed,
        pathlib.Path(args.server_log),
    )
    driver = PytestDriver()
    driver.report_path = pathlib.Path(args.review_log)
    driver.session = Session(pathlib.Path(args.session))
    if args.fresh:
        driver.session.path.unlink(missing_ok=True)
    try:
        return pytest.main(
            [
                *(args.suites or _SUITES),
                "-p", "no:terminal",          # the app draws the terminal
                "-p", "no:cacheprovider",
                "--capture=sys",              # leave the real stdout to Textual
                f"--server={args.server}",
                f"--step-delay={args.step_delay}",
            ],
            plugins=[driver],
        )
    finally:
        if server is not None:
            server.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
