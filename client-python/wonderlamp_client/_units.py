"""Unit-system conversion: pix / norm / height / deg / cm → pixels.

All position and size values sent to the server are in pixels.
The server's coordinate origin is the window centre (matches psychopy default).
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from typing import Any


def to_pixels(
    value: tuple[float, float] | float,
    units: str,
    win_size_px: tuple[int, int],
    monitor: "Any | None" = None,
) -> tuple[float, float]:
    """Convert *value* from *units* to pixels.

    Parameters
    ----------
    value:
        A (x, y) pair or a single scalar (used for radius / width / height).
        A scalar is treated as (v, v).
    units:
        'pix', 'norm', 'height', 'deg', 'cm', or '' (caller must supply
        the window default before calling).
    win_size_px:
        Window size in pixels as (width, height).
    monitor:
        psychopy Monitor object (or compatible). Required for 'deg' / 'cm'.
    """
    if isinstance(value, (int, float)):
        value = (float(value), float(value))
    else:
        value = (float(value[0]), float(value[1]))

    units = (units or "pix").lower()
    w, h = win_size_px

    match units:
        case "pix":
            return value
        case "norm":
            return (value[0] * w / 2.0, value[1] * h / 2.0)
        case "height":
            return (value[0] * h, value[1] * h)
        case "deg":
            if monitor is None:
                raise ValueError(
                    "Unit 'deg' requires a monitor object with deg2pix() method. "
                    "Pass monitor=... to Window()."
                )
            # psychopy Monitor.deg2pix accepts a scalar or array
            px_x = float(monitor.deg2pix(value[0]))
            px_y = float(monitor.deg2pix(value[1]))
            return (px_x, px_y)
        case "cm":
            if monitor is None:
                raise ValueError(
                    "Unit 'cm' requires a monitor object with cm2pix() method. "
                    "Pass monitor=... to Window()."
                )
            px_x = float(monitor.cm2pix(value[0]))
            px_y = float(monitor.cm2pix(value[1]))
            return (px_x, px_y)
        case _:
            raise ValueError(f"Unknown unit: {units!r}")


def apply_operation(
    current: tuple[float, float],
    delta: tuple[float, float],
    operation: str,
) -> tuple[float, float]:
    """Apply an arithmetic operation to a position/size value.

    Supported operations: '' or '=' (assign), '+' (add), '-' (subtract),
    '*' (multiply), '/' (divide).
    """
    op = operation.strip()
    x, y = current
    dx, dy = delta
    match op:
        case "" | "=":
            return (dx, dy)
        case "+":
            return (x + dx, y + dy)
        case "-":
            return (x - dx, y - dy)
        case "*":
            return (x * dx, y * dy)
        case "/":
            return (x / dx, y / dy)
        case _:
            raise ValueError(f"Unknown operation: {operation!r}")
