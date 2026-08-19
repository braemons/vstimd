"""The virtual trigger lines, and their levels."""

from __future__ import annotations

from textual.binding import Binding
from textual.widgets import DataTable

from ..connection import Connection
from ..exceptions import VstimdError
from ..vtl import VtlHandle, VtlKind


class TriggerLines(DataTable):
    """Named VTL lines with their current level, and keys to drive them.

    Driving a line by hand is how a trigger-driven scene is exercised without a
    DAQ attached: `t` toggles the highlighted line, `p` pulses it high and back.
    Both are refused on an output line unless ``allow_output_writes`` says
    otherwise — outputs are what the server tells the rest of the rig, and
    faking one from a console is usually a mistake rather than a test.
    """

    DEFAULT_CSS = """
    TriggerLines { height: 1fr; }
    """

    BINDINGS = [
        Binding("t", "toggle_line", "toggle line"),
        Binding("p", "pulse_line", "pulse line"),
    ]

    def __init__(
        self,
        connection: Connection,
        *,
        refresh_interval: float = 0.5,
        allow_output_writes: bool = False,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(
            cursor_type="row",
            zebra_stripes=True,
            name=name,
            id=id,
            classes=classes,
            disabled=disabled,
        )
        self.connection = connection
        self.refresh_interval = refresh_interval
        self.allow_output_writes = allow_output_writes
        self._lines: list = []

    def on_mount(self) -> None:
        self.add_columns("line", "name", "level")
        self.reload()
        if self.refresh_interval:
            self.set_interval(self.refresh_interval, self.reload)

    @property
    def selected_line(self):
        """The highlighted line's info, or None when nothing is listed."""
        if not self._lines or self.cursor_row >= len(self._lines):
            return None
        return self._lines[self.cursor_row]

    def reload(self) -> None:
        try:
            lines = self.connection.vtl.list_lines()
        except (VstimdError, TimeoutError):
            return
        rows = [
            (
                f"{'in' if line.kind == VtlKind.INPUT else 'out'} {line.bank}.{line.bit}",
                line.name or "—",
                "HIGH" if line.high else "low",
            )
            for line in lines
        ]
        if rows == getattr(self, "_rows_shown", None):
            return
        self._rows_shown = rows
        self._lines = lines
        cursor = self.cursor_row
        self.clear()
        for row in rows:
            self.add_row(*row)
        if rows:
            self.move_cursor(row=min(cursor, len(rows) - 1))

    # ── driving a line ───────────────────────────────────────────────────────

    def _handle_for(self, line) -> VtlHandle | None:
        if line.kind == VtlKind.OUTPUT and not self.allow_output_writes:
            self.notify("output lines are driven by the server, not from here",
                        severity="warning")
            return None
        return (
            VtlHandle.input(line.bank, line.bit)
            if line.kind == VtlKind.INPUT
            else VtlHandle.output(line.bank, line.bit)
        )

    def action_toggle_line(self) -> None:
        line = self.selected_line
        if line is None:
            return
        handle = self._handle_for(line)
        if handle is not None:
            self.connection.vtl.toggle_line(handle)
            self.reload()

    def action_pulse_line(self) -> None:
        """High, then low again — the edge an armed animation is waiting for."""
        line = self.selected_line
        if line is None:
            return
        handle = self._handle_for(line)
        if handle is not None:
            self.connection.vtl.set_line(handle, True)
            self.connection.vtl.set_line(handle, False)
            self.reload()
