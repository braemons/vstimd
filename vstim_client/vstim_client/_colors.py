"""Color normalisation to [r, g, b, a] float 0.0–1.0.

Supports the same inputs as psychopy:
  - Named strings: 'red', 'white', 'black', ...
  - Hex strings: '#ff0000', '#fff'
  - Float tuples (rgb -1..1 psychopy convention OR 0..1): (1.0, -1.0, 0.0)
  - Int tuples rgb255: (255, 0, 128)
  - Single float (greyscale -1..1 psychopy convention)
  - None → transparent / no color
"""

from __future__ import annotations

_NAMED_COLORS: dict[str, tuple[float, float, float]] = {
    "white":   (1.0, 1.0, 1.0),
    "black":   (0.0, 0.0, 0.0),
    "red":     (1.0, 0.0, 0.0),
    "green":   (0.0, 1.0, 0.0),
    "blue":    (0.0, 0.0, 1.0),
    "yellow":  (1.0, 1.0, 0.0),
    "cyan":    (0.0, 1.0, 1.0),
    "magenta": (1.0, 0.0, 1.0),
    "gray":    (0.5, 0.5, 0.5),
    "grey":    (0.5, 0.5, 0.5),
    "orange":  (1.0, 0.647, 0.0),
    "purple":  (0.502, 0.0, 0.502),
    "pink":    (1.0, 0.753, 0.796),
    "brown":   (0.647, 0.165, 0.165),
    "lime":    (0.0, 1.0, 0.0),
    "navy":    (0.0, 0.0, 0.502),
    "teal":    (0.0, 0.502, 0.502),
    "silver":  (0.753, 0.753, 0.753),
    "maroon":  (0.502, 0.0, 0.0),
    "olive":   (0.502, 0.502, 0.0),
}


def normalize_color(color, color_space: str = "rgb") -> list[float] | None:
    """Return [r, g, b, a] with all components 0.0–1.0, or None if color is None."""
    if color is None:
        return None

    rgb = _to_rgb01(color, color_space)
    return [rgb[0], rgb[1], rgb[2], 1.0]


def _to_rgb01(color, color_space: str) -> tuple[float, float, float]:
    # Named string
    if isinstance(color, str):
        key = color.lower().strip()
        if key in _NAMED_COLORS:
            return _NAMED_COLORS[key]
        # Hex
        return _parse_hex(color)

    # Single number → greyscale
    if isinstance(color, (int, float)):
        v = _psychopy_channel_to_01(float(color), color_space)
        return (v, v, v)

    # Sequence of 3 (or 4, alpha ignored on input)
    try:
        seq = tuple(color)
    except TypeError:
        raise ValueError(f"Cannot interpret color: {color!r}")

    if len(seq) not in (3, 4):
        raise ValueError(f"Color sequence must have 3 or 4 elements, got {len(seq)}")

    r, g, b = seq[0], seq[1], seq[2]

    if color_space == "rgb255":
        return (r / 255.0, g / 255.0, b / 255.0)
    elif color_space in ("rgb", ""):
        # psychopy rgb convention: -1..1 → remap to 0..1
        # But if all values are already 0..1 (e.g. passed as plain fractions) we keep them.
        # Heuristic: if any value is outside 0..1, assume psychopy -1..1 convention.
        values = (float(r), float(g), float(b))
        if any(v < 0.0 or v > 1.0 for v in values):
            return tuple(_psychopy_channel_to_01(v, "rgb") for v in values)  # type: ignore[return-value]
        return values
    elif color_space == "hsv":
        return _hsv_to_rgb(float(r), float(g), float(b))
    else:
        # Fallback: treat as 0..1
        return (float(r), float(g), float(b))


def _psychopy_channel_to_01(v: float, color_space: str) -> float:
    """Convert a single psychopy channel value (-1..1) to 0..1."""
    if color_space == "rgb":
        return (v + 1.0) / 2.0
    return float(v)  # already 0..1


def _parse_hex(s: str) -> tuple[float, float, float]:
    s = s.strip().lstrip("#")
    if len(s) == 3:
        s = "".join(c * 2 for c in s)
    if len(s) != 6:
        raise ValueError(f"Invalid hex color: #{s}")
    r = int(s[0:2], 16) / 255.0
    g = int(s[2:4], 16) / 255.0
    b = int(s[4:6], 16) / 255.0
    return (r, g, b)


def _hsv_to_rgb(h: float, s: float, v: float) -> tuple[float, float, float]:
    """h in 0..360, s and v in 0..1."""
    import math
    h = h % 360.0
    c = v * s
    x = c * (1 - abs((h / 60.0) % 2 - 1))
    m = v - c
    sector = int(h / 60)
    rgb_map = [
        (c, x, 0), (x, c, 0), (0, c, x),
        (0, x, c), (x, 0, c), (c, 0, x),
    ]
    r, g, b = rgb_map[sector % 6]
    return (r + m, g + m, b + m)
