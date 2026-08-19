"""What the server says about itself."""

from __future__ import annotations

from textual.reactive import reactive
from textual.timer import Timer
from textual.widgets import Static

from ..connection import Connection
from ..exceptions import VstimdError


class ServerStatus(Static):
    """A line of server facts: resolution, frame rate, version, background."""

    DEFAULT_CSS = """
    ServerStatus { height: auto; padding: 0 1; color: $text-muted; }
    """

    text: reactive[str] = reactive("connecting…")

    def __init__(
        self,
        connection: Connection,
        *,
        refresh_interval: float = 2.0,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes)
        self.connection: Connection = connection
        self.refresh_interval: float = refresh_interval
        self.timer: Timer | None = None

    def on_mount(self) -> None:
        self.reload()
        if self.refresh_interval:
            self.timer = self.set_interval(self.refresh_interval, self.reload)

    def watch_text(self, text: str) -> None:
        self.update(text)

    def reload(self) -> None:
        try:
            info = self.connection.system.query_server_info()
        except (VstimdError, TimeoutError) as exc:
            self.text = f"server unreachable: {exc}"
            return
        version = info.version
        background = info.background_color
        self.text = (
            f"{info.width_px}×{info.height_px} @ {info.frame_rate_hz:.1f} Hz   "
            f"v{version.major}.{version.minor}.{version.patch}   "
            f"background ({background.r:.2f}, {background.g:.2f}, {background.b:.2f})"
        )
