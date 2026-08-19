"""A live list of what is in the server's scene."""

from __future__ import annotations

from textual.widgets import DataTable

from ..connection import Connection
from ..exceptions import VstimdError


class StimulusList(DataTable):
    """Every stimulus the server holds: handle, name, type, whether it is drawn.

    Polls, because the protocol has no scene-changed event to subscribe to.
    ``refresh_interval`` of 0 turns the polling off, for a program that would
    rather call :meth:`reload` itself at moments it knows are interesting.
    """

    DEFAULT_CSS = """
    StimulusList { height: 1fr; }
    """

    def __init__(
        self,
        connection: Connection,
        *,
        refresh_interval: float = 1.0,
        show_disabled: bool = True,
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
        self.show_disabled = show_disabled

    def on_mount(self) -> None:
        self.add_columns("handle", "name", "type", "on")
        self.reload()
        if self.refresh_interval:
            self.set_interval(self.refresh_interval, self.reload)

    @property
    def selected_handle(self) -> int | None:
        """Handle of the highlighted stimulus, if the list is not empty."""
        if not self.row_count:
            return None
        return int(self.get_row_at(self.cursor_row)[0])

    def reload(self) -> None:
        """Ask the server what is in the scene and redraw the list.

        A stimulus can go away between the listing and the query that describes
        it — an animation's teardown, another client, a test cleaning up — so a
        row whose stimulus has vanished is simply dropped rather than raising.
        """
        try:
            entries = self.connection.system.list_stimuli()
        except VstimdError:
            return

        rows = []
        for entry in entries:
            if not entry.enabled and not self.show_disabled:
                continue
            try:
                info = self.connection.stimuli.query(entry.handle)
                type_name = info.stimulus_type.name.lower()
            except VstimdError:
                continue
            rows.append(
                (
                    str(int(entry.handle)),
                    entry.name or "—",
                    type_name,
                    "●" if entry.enabled else "○",
                )
            )

        # Redraw only on a real change: rebuilding every second would fight the
        # cursor for the row the user is looking at.
        if rows == getattr(self, "_rows_shown", None):
            return
        self._rows_shown = rows
        cursor = self.cursor_row
        self.clear()
        for row in rows:
            self.add_row(*row)
        if rows:
            self.move_cursor(row=min(cursor, len(rows) - 1))
