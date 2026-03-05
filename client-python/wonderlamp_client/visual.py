"""wonderlamp_client.visual — PsychoPy-compatible stimulus classes.

Drop-in replacement for psychopy.visual for scripts that target wonderlamp_server.

    from wonderlamp_client import visual
    win = visual.Window(size=(1920, 1080), address='tcp://192.168.1.10:5555')
    circle = visual.Circle(win, radius=50, fillColor='red')
    circle.autoDraw = True
    win.flip()

The *address* parameter on Window carries the server IP and port.

Architecture
------------
- deferred=True  (default): property setters stage commands locally; flip()
  sends them all + 'deferred_flip' so the server applies every change
  simultaneously at the next render frame — giving the experimenter precise
  control over when stimulus changes become visible.
- deferred=False: property setters send immediately (changes may appear at
  different times); flip() is a no-op.

All coordinates sent to the server are in pixels (origin = window centre).
"""

from __future__ import annotations

import itertools
import logging
from typing import Any

from ._colors import normalize_color
from ._commands import (
    CreateCircleCmd, CreateRectCmd, CreatePolygonCmd,
    CreateLineCmd, CreateShapeCmd,
    SetPosCmd, SetOriCmd, SetOpacityCmd, SetFillColorCmd, SetLineColorCmd,
    SetLineWidthCmd, SetEnabledCmd, SetAutoDrawCmd, SetSizeCmd, DestroyCmd,
    DeferredFlipCmd, SetWindowColorCmd, CloseWindowCmd,
)
from ._connection import Connection
from ._units import to_pixels, apply_operation

log = logging.getLogger(__name__)

__all__ = ["Window", "Circle", "Rect", "Polygon", "Line", "ShapeStim"]

_handle_counter = itertools.count(1)


# ---------------------------------------------------------------------------
# Marker base — no behaviour, only for isinstance() checks
# ---------------------------------------------------------------------------

class _WonderlampBase:
    """Marker base. No methods or fields — only used for isinstance() checks."""


# ---------------------------------------------------------------------------
# Window
# ---------------------------------------------------------------------------

class Window:
    """Connection to wonderlamp_server plus frame-buffer semantics.

    Parameters
    ----------
    size:
        Window dimensions in pixels.
    pos:
        Window position on screen (hint to server).
    color:
        Background clear color.
    colorSpace:
        Color space for *color* ('rgb', 'rgb255', 'hsv').
    fullscr:
        Request fullscreen.
    monitor:
        psychopy Monitor object (or compatible) for deg/cm unit conversion.
    units:
        Default unit system: 'pix', 'norm', 'height', 'deg', 'cm'.
    screen:
        Monitor index (0-based).
    waitBlanking:
        Hint to server: sync to vertical blank.
    name:
        Logical name for this window.
    title:
        OS window title bar text.
    deferred:
        If True (default), property setters stage commands locally; flip()
        sends them all so the server applies every change simultaneously at
        the next render frame.  If False, setters send immediately (changes
        may become visible at different times) and flip() is a no-op.
    address:
        ZeroMQ endpoint of wonderlamp_server, e.g. 'tcp://192.168.1.10:5555'.
        Change host and port here to connect to a remote machine.
    autoLog:
        Enable Python logging for this window.
    """

    def __init__(
        self,
        size: tuple[int, int] = (800, 600),
        pos: tuple[int, int] | None = None,
        color=(0, 0, 0),
        colorSpace: str = "rgb",
        fullscr: bool = False,
        monitor: Any = None,
        units: str = "pix",
        screen: int = 0,
        waitBlanking: bool = True,
        name: str = "window1",
        title: str = "wonderlamp",
        deferred: bool = True,
        address: str = "tcp://localhost:5555",
        autoLog: bool = True,
    ) -> None:
        self.size = size
        self.pos = pos
        self.colorSpace = colorSpace
        self.fullscr = fullscr
        self.monitor = monitor
        self.units = units
        self.screen = screen
        self.waitBlanking = waitBlanking
        self.name = name
        self.title = title
        self.deferred = deferred
        self.address = address
        self.autoLog = autoLog

        self._connection = Connection(address)
        self._queue: list[dict[str, Any]] = []
        self._to_draw_once: set[int] = set()  # handles for one-shot draw

        # Send window creation
        self._color = normalize_color(color, colorSpace) or [0.0, 0.0, 0.0, 1.0]
        self._send("open_window", {
            "size": list(size),
            "pos": list(pos) if pos else None,
            "color": self._color,
            "fullscr": fullscr,
            "screen": screen,
            "wait_blanking": waitBlanking,
            "title": title,
            "name": name,
        })

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def flip(self, clearBuffer: bool = True) -> None:
        """Apply all staged changes simultaneously and advance the render frame.

        In deferred mode (default): sends all staged commands plus one-shot
        draw handles in a single multipart ZMQ message, then sends
        'deferred_flip'. The server holds every change until that sentinel
        arrives and applies them all at once before rendering the next frame,
        giving the experimenter precise control over when changes become visible.

        In immediate mode (deferred=False): no-op.
        """
        if not self.deferred:
            return

        batch: list[dict[str, Any]] = list(self._queue)
        self._queue.clear()

        # Enable one-shot draw handles
        for handle in self._to_draw_once:
            batch.append(SetEnabledCmd(handle=handle, enabled=True).to_dict())

        # Signal end of frame
        batch.append(DeferredFlipCmd().to_dict())

        if batch:
            self._connection.send_batch(batch)

        # Disable one-shot handles after flip
        if self._to_draw_once:
            disable_batch = [
                SetEnabledCmd(handle=h, enabled=False).to_dict()
                for h in self._to_draw_once
            ]
            self._connection.send_batch(disable_batch)
            self._to_draw_once.clear()

    def close(self) -> None:
        """Shut down the server window and close the ZMQ socket."""
        try:
            self._send(CloseWindowCmd().cmd, {})
        except Exception:
            pass
        self._connection.close()

    def setColor(self, color: Any, colorSpace: str = "rgb", log: Any = None) -> None:
        self._color = normalize_color(color, colorSpace) or [0.0, 0.0, 0.0, 1.0]
        cmd = SetWindowColorCmd(color=self._color)
        self._dispatch(cmd.to_dict())

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _send(self, cmd: str, payload: dict[str, Any]) -> dict[str, Any]:
        """Send a single command immediately (bypass deferred queue)."""
        return self._connection.send({"cmd": cmd, **payload})

    def _enqueue(self, payload: dict[str, Any]) -> None:
        """Stage a command for the next flip() (deferred mode)."""
        self._queue.append(payload)

    def _flush(self) -> None:
        """Send all staged commands immediately (called by flip)."""
        if self._queue:
            self._connection.send_batch(self._queue)
            self._queue.clear()

    def _dispatch(self, payload: dict[str, Any]) -> None:
        """Route to _enqueue (deferred) or _send (immediate)."""
        if self.deferred:
            self._enqueue(payload)
        else:
            self._send(payload["cmd"], {k: v for k, v in payload.items() if k != "cmd"})

    def _queue_single_draw(self, handle: int) -> None:
        """Mark *handle* to be enabled for exactly one flip."""
        self._to_draw_once.add(handle)

    def _units_for(self, units: str) -> str:
        return units if units else self.units

    def _to_px(self, value: Any, units: str) -> tuple[float, float]:
        return to_pixels(value, self._units_for(units), self.size, self.monitor)

    # ------------------------------------------------------------------
    # Context manager
    # ------------------------------------------------------------------

    def __enter__(self) -> "Window":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()


# ---------------------------------------------------------------------------
# Circle
# ---------------------------------------------------------------------------

class Circle(_WonderlampBase):
    def __init__(
        self,
        win: Window,
        radius: float = 0.5,
        edges: int = 32,
        units: str = "",
        pos: tuple = (0.0, 0.0),
        lineWidth: float = 1.5,
        lineColor: Any = None,
        fillColor: Any = "white",
        colorSpace: str = "rgb",
        ori: float = 0.0,
        opacity: float | None = None,
        contrast: float = 1.0,
        name: str | None = None,
        autoDraw: bool = False,
        autoLog: bool | None = None,
    ) -> None:
        self._win = win
        self._units = units
        self._name = name
        self._handle = next(_handle_counter)

        self._pos = win._to_px(pos, units)
        self._ori = ori
        self._opacity = opacity if opacity is not None else 1.0
        self._line_width = lineWidth
        self._fill_color = normalize_color(fillColor, colorSpace)
        self._line_color = normalize_color(lineColor, colorSpace)
        self._radius = float(win._to_px(radius, units)[0])
        self._edges = edges
        self._auto_draw = autoDraw

        cmd = CreateCircleCmd(
            handle=self._handle,
            radius=self._radius,
            pos=list(self._pos),
            fill_color=self._fill_color,
            line_color=self._line_color,
            line_width=self._line_width,
            ori=self._ori,
            opacity=self._opacity,
            enabled=autoDraw,
        )
        win._dispatch(cmd.to_dict())

    # --- Properties ---

    @property
    def pos(self) -> tuple[float, float]:
        return self._pos

    @pos.setter
    def pos(self, value: Any) -> None:
        self._pos = self._win._to_px(value, self._units)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    @property
    def ori(self) -> float:
        return self._ori

    @ori.setter
    def ori(self, value: float) -> None:
        self._ori = float(value)
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: float) -> None:
        self._opacity = float(value)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    @property
    def fillColor(self) -> list[float] | None:
        return self._fill_color

    @fillColor.setter
    def fillColor(self, value: Any) -> None:
        self._fill_color = normalize_color(value)
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    @property
    def lineColor(self) -> list[float] | None:
        return self._line_color

    @lineColor.setter
    def lineColor(self, value: Any) -> None:
        self._line_color = normalize_color(value)
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    @property
    def lineWidth(self) -> float:
        return self._line_width

    @lineWidth.setter
    def lineWidth(self, value: float) -> None:
        self._line_width = float(value)
        self._win._dispatch(SetLineWidthCmd(handle=self._handle, line_width=self._line_width).to_dict())

    @property
    def autoDraw(self) -> bool:
        return self._auto_draw

    @autoDraw.setter
    def autoDraw(self, value: bool) -> None:
        self._auto_draw = bool(value)
        self._win._dispatch(SetEnabledCmd(handle=self._handle, enabled=self._auto_draw).to_dict())

    # --- Methods ---

    def draw(self) -> None:
        """Queue this stimulus for one-shot rendering on next flip()."""
        self._win._queue_single_draw(self._handle)

    def setPos(self, newPos: Any, operation: str = "", log: Any = None) -> None:
        new_px = self._win._to_px(newPos, self._units)
        self._pos = apply_operation(self._pos, new_px, operation)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    def setOri(self, newOri: float, operation: str = "", log: Any = None) -> None:
        delta = (float(newOri), 0.0)
        cur = (self._ori, 0.0)
        self._ori = apply_operation(cur, delta, operation)[0]
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    def setSize(self, newSize: Any, operation: str = "", units: str | None = None, log: Any = None) -> None:
        u = units or self._units
        new_px = self._win._to_px(newSize, u)
        self._radius = new_px[0]
        self._win._dispatch(SetSizeCmd(handle=self._handle, size=list(new_px)).to_dict())

    def setColor(self, color: Any, colorSpace: str | None = None, operation: str = "", log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setFillColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setLineColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._line_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    def setOpacity(self, newOpacity: float, operation: str = "", log: Any = None) -> None:
        self._opacity = float(newOpacity)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    def setAutoDraw(self, value: bool, log: Any = None) -> None:
        self.autoDraw = value

    def contains(self, x: Any, y: Any = None, units: str | None = None) -> bool:
        # Stub — spatial queries not yet implemented
        raise NotImplementedError("contains() is not yet implemented in wonderlamp_client v1")


# ---------------------------------------------------------------------------
# Rect
# ---------------------------------------------------------------------------

class Rect(_WonderlampBase):
    def __init__(
        self,
        win: Window,
        width: float = 0.5,
        height: float = 0.5,
        units: str = "",
        pos: tuple = (0.0, 0.0),
        lineWidth: float = 1.5,
        lineColor: Any = None,
        fillColor: Any = "white",
        colorSpace: str = "rgb",
        ori: float = 0.0,
        opacity: float | None = None,
        contrast: float = 1.0,
        name: str | None = None,
        autoDraw: bool = False,
        autoLog: bool | None = None,
    ) -> None:
        self._win = win
        self._units = units
        self._name = name
        self._handle = next(_handle_counter)

        self._pos = win._to_px(pos, units)
        self._ori = ori
        self._opacity = opacity if opacity is not None else 1.0
        self._line_width = lineWidth
        self._fill_color = normalize_color(fillColor, colorSpace)
        self._line_color = normalize_color(lineColor, colorSpace)
        size_px = win._to_px((width, height), units)
        self._width = size_px[0]
        self._height = size_px[1]
        self._auto_draw = autoDraw

        cmd = CreateRectCmd(
            handle=self._handle,
            width=self._width,
            height=self._height,
            pos=list(self._pos),
            fill_color=self._fill_color,
            line_color=self._line_color,
            line_width=self._line_width,
            ori=self._ori,
            opacity=self._opacity,
            enabled=autoDraw,
        )
        win._dispatch(cmd.to_dict())

    @property
    def pos(self) -> tuple[float, float]:
        return self._pos

    @pos.setter
    def pos(self, value: Any) -> None:
        self._pos = self._win._to_px(value, self._units)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    @property
    def ori(self) -> float:
        return self._ori

    @ori.setter
    def ori(self, value: float) -> None:
        self._ori = float(value)
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: float) -> None:
        self._opacity = float(value)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    @property
    def fillColor(self) -> list[float] | None:
        return self._fill_color

    @fillColor.setter
    def fillColor(self, value: Any) -> None:
        self._fill_color = normalize_color(value)
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    @property
    def lineColor(self) -> list[float] | None:
        return self._line_color

    @lineColor.setter
    def lineColor(self, value: Any) -> None:
        self._line_color = normalize_color(value)
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    @property
    def lineWidth(self) -> float:
        return self._line_width

    @lineWidth.setter
    def lineWidth(self, value: float) -> None:
        self._line_width = float(value)
        self._win._dispatch(SetLineWidthCmd(handle=self._handle, line_width=self._line_width).to_dict())

    @property
    def autoDraw(self) -> bool:
        return self._auto_draw

    @autoDraw.setter
    def autoDraw(self, value: bool) -> None:
        self._auto_draw = bool(value)
        self._win._dispatch(SetEnabledCmd(handle=self._handle, enabled=self._auto_draw).to_dict())

    def draw(self) -> None:
        self._win._queue_single_draw(self._handle)

    def setPos(self, newPos: Any, operation: str = "", log: Any = None) -> None:
        new_px = self._win._to_px(newPos, self._units)
        self._pos = apply_operation(self._pos, new_px, operation)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    def setOri(self, newOri: float, operation: str = "", log: Any = None) -> None:
        delta = (float(newOri), 0.0)
        cur = (self._ori, 0.0)
        self._ori = apply_operation(cur, delta, operation)[0]
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    def setSize(self, newSize: Any, operation: str = "", units: str | None = None, log: Any = None) -> None:
        u = units or self._units
        new_px = self._win._to_px(newSize, u)
        self._width, self._height = new_px
        self._win._dispatch(SetSizeCmd(handle=self._handle, size=list(new_px)).to_dict())

    def setColor(self, color: Any, colorSpace: str | None = None, operation: str = "", log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setFillColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setLineColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._line_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    def setOpacity(self, newOpacity: float, operation: str = "", log: Any = None) -> None:
        self._opacity = float(newOpacity)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    def setAutoDraw(self, value: bool, log: Any = None) -> None:
        self.autoDraw = value

    def contains(self, x: Any, y: Any = None, units: str | None = None) -> bool:
        raise NotImplementedError("contains() is not yet implemented in wonderlamp_client v1")


# ---------------------------------------------------------------------------
# Polygon
# ---------------------------------------------------------------------------

class Polygon(_WonderlampBase):
    def __init__(
        self,
        win: Window,
        edges: int = 3,
        radius: float = 0.5,
        units: str = "",
        pos: tuple = (0.0, 0.0),
        lineWidth: float = 1.5,
        lineColor: Any = None,
        fillColor: Any = "white",
        colorSpace: str = "rgb",
        ori: float = 0.0,
        opacity: float | None = None,
        contrast: float = 1.0,
        name: str | None = None,
        autoDraw: bool = False,
        autoLog: bool | None = None,
    ) -> None:
        self._win = win
        self._units = units
        self._name = name
        self._handle = next(_handle_counter)

        self._pos = win._to_px(pos, units)
        self._ori = ori
        self._opacity = opacity if opacity is not None else 1.0
        self._line_width = lineWidth
        self._fill_color = normalize_color(fillColor, colorSpace)
        self._line_color = normalize_color(lineColor, colorSpace)
        self._radius = float(win._to_px(radius, units)[0])
        self._edges = edges
        self._auto_draw = autoDraw

        cmd = CreatePolygonCmd(
            handle=self._handle,
            radius=self._radius,
            edges=edges,
            pos=list(self._pos),
            fill_color=self._fill_color,
            line_color=self._line_color,
            line_width=self._line_width,
            ori=self._ori,
            opacity=self._opacity,
            enabled=autoDraw,
        )
        win._dispatch(cmd.to_dict())

    @property
    def pos(self) -> tuple[float, float]:
        return self._pos

    @pos.setter
    def pos(self, value: Any) -> None:
        self._pos = self._win._to_px(value, self._units)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    @property
    def ori(self) -> float:
        return self._ori

    @ori.setter
    def ori(self, value: float) -> None:
        self._ori = float(value)
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: float) -> None:
        self._opacity = float(value)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    @property
    def fillColor(self) -> list[float] | None:
        return self._fill_color

    @fillColor.setter
    def fillColor(self, value: Any) -> None:
        self._fill_color = normalize_color(value)
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    @property
    def lineColor(self) -> list[float] | None:
        return self._line_color

    @lineColor.setter
    def lineColor(self, value: Any) -> None:
        self._line_color = normalize_color(value)
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    @property
    def lineWidth(self) -> float:
        return self._line_width

    @lineWidth.setter
    def lineWidth(self, value: float) -> None:
        self._line_width = float(value)
        self._win._dispatch(SetLineWidthCmd(handle=self._handle, line_width=self._line_width).to_dict())

    @property
    def autoDraw(self) -> bool:
        return self._auto_draw

    @autoDraw.setter
    def autoDraw(self, value: bool) -> None:
        self._auto_draw = bool(value)
        self._win._dispatch(SetEnabledCmd(handle=self._handle, enabled=self._auto_draw).to_dict())

    def draw(self) -> None:
        self._win._queue_single_draw(self._handle)

    def setPos(self, newPos: Any, operation: str = "", log: Any = None) -> None:
        new_px = self._win._to_px(newPos, self._units)
        self._pos = apply_operation(self._pos, new_px, operation)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    def setOri(self, newOri: float, operation: str = "", log: Any = None) -> None:
        delta = (float(newOri), 0.0)
        cur = (self._ori, 0.0)
        self._ori = apply_operation(cur, delta, operation)[0]
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    def setSize(self, newSize: Any, operation: str = "", units: str | None = None, log: Any = None) -> None:
        u = units or self._units
        new_px = self._win._to_px(newSize, u)
        self._radius = new_px[0]
        self._win._dispatch(SetSizeCmd(handle=self._handle, size=list(new_px)).to_dict())

    def setColor(self, color: Any, colorSpace: str | None = None, operation: str = "", log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setFillColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setLineColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._line_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    def setOpacity(self, newOpacity: float, operation: str = "", log: Any = None) -> None:
        self._opacity = float(newOpacity)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    def setAutoDraw(self, value: bool, log: Any = None) -> None:
        self.autoDraw = value

    def contains(self, x: Any, y: Any = None, units: str | None = None) -> bool:
        raise NotImplementedError("contains() is not yet implemented in wonderlamp_client v1")


# ---------------------------------------------------------------------------
# Line
# ---------------------------------------------------------------------------

class Line(_WonderlampBase):
    def __init__(
        self,
        win: Window,
        start: tuple = (-0.5, 0.0),
        end: tuple = (0.5, 0.0),
        units: str = "",
        lineWidth: float = 1.5,
        lineColor: Any = "white",
        fillColor: Any = None,
        colorSpace: str = "rgb",
        ori: float = 0.0,
        opacity: float | None = None,
        contrast: float = 1.0,
        name: str | None = None,
        autoDraw: bool = False,
        autoLog: bool | None = None,
    ) -> None:
        self._win = win
        self._units = units
        self._name = name
        self._handle = next(_handle_counter)

        self._start = win._to_px(start, units)
        self._end = win._to_px(end, units)
        self._pos = (
            (self._start[0] + self._end[0]) / 2.0,
            (self._start[1] + self._end[1]) / 2.0,
        )
        self._ori = ori
        self._opacity = opacity if opacity is not None else 1.0
        self._line_width = lineWidth
        self._fill_color = normalize_color(fillColor, colorSpace)
        self._line_color = normalize_color(lineColor, colorSpace)
        self._auto_draw = autoDraw

        cmd = CreateLineCmd(
            handle=self._handle,
            start=list(self._start),
            end=list(self._end),
            line_color=self._line_color,
            line_width=self._line_width,
            ori=self._ori,
            opacity=self._opacity,
            enabled=autoDraw,
        )
        win._dispatch(cmd.to_dict())

    @property
    def pos(self) -> tuple[float, float]:
        return self._pos

    @pos.setter
    def pos(self, value: Any) -> None:
        self._pos = self._win._to_px(value, self._units)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    @property
    def ori(self) -> float:
        return self._ori

    @ori.setter
    def ori(self, value: float) -> None:
        self._ori = float(value)
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: float) -> None:
        self._opacity = float(value)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    @property
    def lineColor(self) -> list[float] | None:
        return self._line_color

    @lineColor.setter
    def lineColor(self, value: Any) -> None:
        self._line_color = normalize_color(value)
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    @property
    def fillColor(self) -> list[float] | None:
        return self._fill_color

    @fillColor.setter
    def fillColor(self, value: Any) -> None:
        self._fill_color = normalize_color(value)
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    @property
    def lineWidth(self) -> float:
        return self._line_width

    @lineWidth.setter
    def lineWidth(self, value: float) -> None:
        self._line_width = float(value)
        self._win._dispatch(SetLineWidthCmd(handle=self._handle, line_width=self._line_width).to_dict())

    @property
    def autoDraw(self) -> bool:
        return self._auto_draw

    @autoDraw.setter
    def autoDraw(self, value: bool) -> None:
        self._auto_draw = bool(value)
        self._win._dispatch(SetEnabledCmd(handle=self._handle, enabled=self._auto_draw).to_dict())

    def draw(self) -> None:
        self._win._queue_single_draw(self._handle)

    def setPos(self, newPos: Any, operation: str = "", log: Any = None) -> None:
        new_px = self._win._to_px(newPos, self._units)
        self._pos = apply_operation(self._pos, new_px, operation)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    def setOri(self, newOri: float, operation: str = "", log: Any = None) -> None:
        delta = (float(newOri), 0.0)
        cur = (self._ori, 0.0)
        self._ori = apply_operation(cur, delta, operation)[0]
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    def setSize(self, newSize: Any, operation: str = "", units: str | None = None, log: Any = None) -> None:
        u = units or self._units
        new_px = self._win._to_px(newSize, u)
        self._win._dispatch(SetSizeCmd(handle=self._handle, size=list(new_px)).to_dict())

    def setColor(self, color: Any, colorSpace: str | None = None, operation: str = "", log: Any = None) -> None:
        self._line_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    def setFillColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setLineColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._line_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    def setOpacity(self, newOpacity: float, operation: str = "", log: Any = None) -> None:
        self._opacity = float(newOpacity)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    def setAutoDraw(self, value: bool, log: Any = None) -> None:
        self.autoDraw = value

    def contains(self, x: Any, y: Any = None, units: str | None = None) -> bool:
        raise NotImplementedError("contains() is not yet implemented in wonderlamp_client v1")


# ---------------------------------------------------------------------------
# ShapeStim
# ---------------------------------------------------------------------------

class ShapeStim(_WonderlampBase):
    def __init__(
        self,
        win: Window,
        vertices: Any = ((-0.5, 0.0), (0.0, 0.5), (0.5, 0.0)),
        units: str = "",
        pos: tuple = (0.0, 0.0),
        lineWidth: float = 1.5,
        lineColor: Any = "white",
        fillColor: Any = None,
        colorSpace: str = "rgb",
        ori: float = 0.0,
        opacity: float | None = None,
        contrast: float = 1.0,
        closeShape: bool = True,
        name: str | None = None,
        autoDraw: bool = False,
        autoLog: bool | None = None,
    ) -> None:
        self._win = win
        self._units = units
        self._name = name
        self._handle = next(_handle_counter)
        self._close_shape = closeShape

        self._pos = win._to_px(pos, units)
        self._ori = ori
        self._opacity = opacity if opacity is not None else 1.0
        self._line_width = lineWidth
        self._fill_color = normalize_color(fillColor, colorSpace)
        self._line_color = normalize_color(lineColor, colorSpace)
        self._auto_draw = autoDraw

        verts_px = [list(win._to_px(v, units)) for v in vertices]
        self._vertices = verts_px

        cmd = CreateShapeCmd(
            handle=self._handle,
            vertices=verts_px,
            pos=list(self._pos),
            fill_color=self._fill_color,
            line_color=self._line_color,
            line_width=self._line_width,
            ori=self._ori,
            opacity=self._opacity,
            enabled=autoDraw,
        )
        win._dispatch(cmd.to_dict())

    @property
    def pos(self) -> tuple[float, float]:
        return self._pos

    @pos.setter
    def pos(self, value: Any) -> None:
        self._pos = self._win._to_px(value, self._units)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    @property
    def ori(self) -> float:
        return self._ori

    @ori.setter
    def ori(self, value: float) -> None:
        self._ori = float(value)
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: float) -> None:
        self._opacity = float(value)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    @property
    def fillColor(self) -> list[float] | None:
        return self._fill_color

    @fillColor.setter
    def fillColor(self, value: Any) -> None:
        self._fill_color = normalize_color(value)
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    @property
    def lineColor(self) -> list[float] | None:
        return self._line_color

    @lineColor.setter
    def lineColor(self, value: Any) -> None:
        self._line_color = normalize_color(value)
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    @property
    def lineWidth(self) -> float:
        return self._line_width

    @lineWidth.setter
    def lineWidth(self, value: float) -> None:
        self._line_width = float(value)
        self._win._dispatch(SetLineWidthCmd(handle=self._handle, line_width=self._line_width).to_dict())

    @property
    def autoDraw(self) -> bool:
        return self._auto_draw

    @autoDraw.setter
    def autoDraw(self, value: bool) -> None:
        self._auto_draw = bool(value)
        self._win._dispatch(SetEnabledCmd(handle=self._handle, enabled=self._auto_draw).to_dict())

    def draw(self) -> None:
        self._win._queue_single_draw(self._handle)

    def setPos(self, newPos: Any, operation: str = "", log: Any = None) -> None:
        new_px = self._win._to_px(newPos, self._units)
        self._pos = apply_operation(self._pos, new_px, operation)
        self._win._dispatch(SetPosCmd(handle=self._handle, pos=list(self._pos)).to_dict())

    def setOri(self, newOri: float, operation: str = "", log: Any = None) -> None:
        delta = (float(newOri), 0.0)
        cur = (self._ori, 0.0)
        self._ori = apply_operation(cur, delta, operation)[0]
        self._win._dispatch(SetOriCmd(handle=self._handle, ori=self._ori).to_dict())

    def setSize(self, newSize: Any, operation: str = "", units: str | None = None, log: Any = None) -> None:
        u = units or self._units
        new_px = self._win._to_px(newSize, u)
        self._win._dispatch(SetSizeCmd(handle=self._handle, size=list(new_px)).to_dict())

    def setColor(self, color: Any, colorSpace: str | None = None, operation: str = "", log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setFillColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._fill_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetFillColorCmd(handle=self._handle, fill_color=self._fill_color).to_dict())

    def setLineColor(self, color: Any, colorSpace: str | None = None, log: Any = None) -> None:
        self._line_color = normalize_color(color, colorSpace or "rgb")
        self._win._dispatch(SetLineColorCmd(handle=self._handle, line_color=self._line_color).to_dict())

    def setOpacity(self, newOpacity: float, operation: str = "", log: Any = None) -> None:
        self._opacity = float(newOpacity)
        self._win._dispatch(SetOpacityCmd(handle=self._handle, opacity=self._opacity).to_dict())

    def setAutoDraw(self, value: bool, log: Any = None) -> None:
        self.autoDraw = value

    def contains(self, x: Any, y: Any = None, units: str | None = None) -> bool:
        raise NotImplementedError("contains() is not yet implemented in wonderlamp_client v1")
