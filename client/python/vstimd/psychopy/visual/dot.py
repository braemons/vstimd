from __future__ import annotations

import logging
import random
import warnings
from typing import Any

from vstimd.stimuli import (
    CoherenceCount,
    DotShape,
    DotsParams,
    NoiseRule,
    Region,
    Reinsertion,
    SignalRule,
    lifetime_from_psychopy,
)
from vstimd.stimuli.color import Color as StimulusColor
from vstimd.stimuli.vec import Vec2 as StimulusVec2

from ..._handles import StimulusHandle
from ._colors import to_color
from ._types import PsychoPyColor, PsychoPyVec2
from ._units import to_pixels
from .window import Window

_log = logging.getLogger(__name__)

_COLOR_SPACES = ("rgb", "rgb255", "rgb1", "")
_SIGNAL_DOTS = {"same": SignalRule.SAME, "different": SignalRule.DIFFERENT}
_NOISE_DOTS = {
    "direction": NoiseRule.DIRECTION,
    "position": NoiseRule.POSITION,
    "walk": NoiseRule.WALK,
}


def _field_shape(value: str | None) -> str:
    """PsychoPy's spellings: ``None``, ``'sqr'`` and ``'square'`` are one shape."""
    if value in (None, "sqr", "square"):
        return "sqr"
    if value == "circle":
        return "circle"
    raise NotImplementedError(
        f"DotStim: fieldShape={value!r} is not supported. Supported values: 'sqr', 'square', 'circle'."
    )


def _lookup(table: dict[str, Any], value: str, name: str) -> Any:
    try:
        return table[value]
    except KeyError:
        raise NotImplementedError(
            f"DotStim: {name}={value!r} is not supported. Supported values: {sorted(table)}."
        ) from None


def _apply_op(current: Any, value: Any, op: str) -> Any:
    """PsychoPy's attribute operations (``setDir(10, '+')``), on scalars and pairs."""
    if not op:
        return value
    ops = {
        "+": lambda a, b: a + b,
        "-": lambda a, b: a - b,
        "*": lambda a, b: a * b,
        "/": lambda a, b: a / b,
    }
    if op not in ops:
        raise ValueError(f"DotStim: unknown operation {op!r}")
    f = ops[op]
    if isinstance(current, tuple):
        other = (value, value) if isinstance(value, (int, float)) else value
        return (f(current[0], other[0]), f(current[1], other[1]))
    return f(current, value)


def _anchor_to_center(anchor: str, width_px: float, height_px: float) -> tuple[float, float]:
    """Offset from the anchor point to the field centre, in pixels (Y up).

    ``'top-left'`` puts the field's top-left corner at ``fieldPos``, so the centre
    is half a field right of it and half a field down.
    """
    a = anchor.lower()
    dx = width_px / 2.0 if "left" in a else -width_px / 2.0 if "right" in a else 0.0
    dy = -height_px / 2.0 if "top" in a else height_px / 2.0 if "bottom" in a else 0.0
    return dx, dy


class DotStim:
    """Random dot kinematogram compatible with ``psychopy.visual.DotStim``.

    The dots are simulated on the server, from the same rules PsychoPy uses:

    - ``nDots`` dots live in the field — a rectangle, or an ellipse for
      ``fieldShape='circle'`` — are born uniformly inside it, and a dot that leaves
      it respawns at a uniform random position inside it, as PsychoPy's does.
    - Exactly ``round(coherence * nDots)`` dots carry the signal, as in PsychoPy;
      under ``signalDots='same'`` they are the same dots on every frame.
    - ``noiseDots`` and ``signalDots`` follow the Scase, Braddick & Raymond (1996)
      categories exactly as PsychoPy names them.
    - Dots are aliased squares of ``dotSize`` pixels, which is what PsychoPy's
      ``GL_POINTS`` draw.

    Differences from PsychoPy — deliberate, and all consequences of the server
    owning the stimulus:

    - **Dots move on every display frame, not on every** ``draw()``. PsychoPy only
      advances the field when ``draw()`` is called; vstimd advances it every frame,
      visible or not, so that frame N depends only on the seed and N. A script
      calling ``draw()`` every frame sees no difference.
    - ``speed`` is in units per frame, as in PsychoPy, and is converted to pixels
      per second at the window's *nominal* refresh rate. The saved config records
      pixels per second.
    - The random stream is vstimd's, not numpy's: the same algorithm and statistics,
      but not the same dots. Lifetimes are staggered by dot index, so exactly
      ``nDots / dotLife`` dots are reborn each frame rather than about that many.
    - ``seed`` (a vstimd extension) makes the field reproducible. If omitted a
      random seed is drawn — which is PsychoPy's behaviour — and recorded, so
      ``stim.seed`` and the saved config still say which dots were shown.
      ``refreshDots()`` moves to the next seed.
    - In ``units='norm'`` a non-square window scales the two axes differently.
      vstimd's field is isotropic in pixels, so ``speed`` is converted with the
      horizontal scale, and a warning is given.

    Not supported (``NotImplementedError``): ``element``, a per-dot ``dotSize``
    array, the deprecated ``rgb`` argument, and colour spaces other than
    ``'rgb'``, ``'rgb1'`` and ``'rgb255'``.
    """

    def __init__(
        self,
        win: Window,
        units: str = "",
        nDots: int = 1,
        coherence: float = 0.5,
        fieldPos: PsychoPyVec2 = (0.0, 0.0),
        fieldSize: PsychoPyVec2 | float = (1.0, 1.0),
        fieldShape: str | None = "sqr",
        fieldAnchor: str = "center",
        dotSize: float = 2.0,
        dotLife: int = 3,
        dir: float = 0.0,  # PsychoPy's name, shadowing the builtin
        speed: float = 0.5,
        rgb: Any = None,
        color: PsychoPyColor = (1.0, 1.0, 1.0),
        colorSpace: str = "rgb",
        opacity: float | None = None,
        contrast: float = 1.0,
        depth: int = 0,
        element: Any = None,
        signalDots: str = "same",
        noiseDots: str = "direction",
        name: str | None = None,
        autoLog: bool | None = None,
        # vstimd extensions — not in PsychoPy
        seed: int | None = None,
        autoDraw: bool = False,
    ) -> None:
        if rgb is not None:
            raise NotImplementedError(
                "DotStim: the deprecated rgb argument is not supported. Use color and colorSpace."
            )
        if element is not None:
            raise NotImplementedError("DotStim: element is not supported; dots are drawn by the server.")
        if isinstance(dotSize, (tuple, list)):
            raise NotImplementedError("DotStim: per-dot dotSize arrays are not supported.")
        if colorSpace not in _COLOR_SPACES:
            raise NotImplementedError(
                f"DotStim: colorSpace={colorSpace!r} is not supported. "
                "Supported values: 'rgb', 'rgb255', 'rgb1'."
            )

        self._win = win
        self._units = units
        self.name = name
        self.depth = depth
        self.autoLog = autoLog
        self._auto_draw = False

        self._n_dots = int(nDots)
        self._field_pos = self._pair(fieldPos)
        self._field_size = self._pair(fieldSize)
        self._field_shape = _field_shape(fieldShape)
        self._anchor = fieldAnchor
        self._dot_size = float(dotSize)
        self._dot_life = int(dotLife)
        self._dir = float(dir)
        self._speed = float(speed)
        self._color: PsychoPyColor = color
        self._color_space = colorSpace
        self._opacity = 1.0 if opacity is None else float(opacity)
        self._contrast = float(contrast)
        self._signal_dots = signalDots
        self._noise_dots = noiseDots
        self._coherence = self._rounded_coherence(coherence)
        self._seed = random.SystemRandom().getrandbits(64) if seed is None else int(seed)

        self._warn_if_anisotropic()
        self._handle: StimulusHandle = win._conn.stimuli.dots.create_dots(
            name=name or "",
            position_px=self._center_px(),
            params=self._params(),
        )
        if self._opacity != 1.0:
            win._conn.stimuli.set_alpha(self._handle, self._opacity)
        if autoDraw:
            self.autoDraw = True

    # ── Internal helpers ──────────────────────────────────────────────────────

    @staticmethod
    def _pair(value: PsychoPyVec2 | float) -> tuple[float, float]:
        if isinstance(value, (int, float)):
            return (float(value), float(value))
        return (float(value[0]), float(value[1]))

    def _effective_units(self) -> str:
        return self._win._resolve_units(self._units)

    def _px_pair(self, value: tuple[float, float]) -> tuple[float, float]:
        result = to_pixels(value, self._effective_units(), self._win.size, self._win.monitor)
        assert isinstance(result, tuple)
        return result

    def _field_px(self) -> tuple[float, float]:
        # PsychoPy allows negative sizes; the extent is the same either way.
        w, h = self._px_pair(self._field_size)
        return (abs(w), abs(h))

    def _center_px(self) -> StimulusVec2:
        px, py = self._px_pair(self._field_pos)
        dx, dy = _anchor_to_center(self._anchor, *self._field_px())
        return StimulusVec2(px + dx, py + dy)

    def _speed_px_per_s(self) -> float:
        # Units per frame → pixels per frame (horizontal scale) → pixels per second.
        px_per_frame = self._px_pair((self._speed, self._speed))[0]
        return px_per_frame * self._win._frame_rate_hz

    def _warn_if_anisotropic(self) -> None:
        w, h = self._win.size
        if self._effective_units() == "norm" and w != h:
            warnings.warn(
                "DotStim: units='norm' on a non-square window scales x and y differently; "
                "vstimd converts speed with the horizontal scale, so motion is not "
                "distorted the way PsychoPy distorts it.",
                UserWarning,
                stacklevel=3,
            )

    def _dot_color(self) -> StimulusColor:
        c = to_color(self._color, self._color_space, 1.0) or StimulusColor(1.0, 1.0, 1.0, 1.0)
        # PsychoPy's contrast scales the colour in signed (-1..1) rgb, about mid-grey.
        k = self._contrast

        def scale(v: float) -> float:
            return max(0.0, min(1.0, ((v * 2.0 - 1.0) * k + 1.0) / 2.0))

        return StimulusColor(scale(c.r), scale(c.g), scale(c.b), c.a)

    def _rounded_coherence(self, coherence: float) -> float:
        """PsychoPy stores coherence rounded to a whole number of dots."""
        if not 0.0 <= coherence <= 1.0:
            raise ValueError("DotStim.coherence must be between 0 and 1")
        if self._n_dots <= 0:
            return float(coherence)
        return round(coherence * self._n_dots) / self._n_dots

    def _params(self) -> DotsParams:
        w, h = self._field_px()
        field = Region.ellipse(w, h) if self._field_shape == "circle" else Region.rect(w, h)
        return DotsParams(
            field=field,
            dot_count=self._n_dots,
            dot_size_px=self._dot_size,
            dot_color=self._dot_color(),
            dot_shape=DotShape.SQUARE,
            direction_deg=self._dir,
            speed_px_per_s=self._speed_px_per_s(),
            coherence=self._coherence,
            coherence_count=CoherenceCount.EXACT,
            signal_rule=_lookup(_SIGNAL_DOTS, self._signal_dots, "signalDots"),
            noise_rule=_lookup(_NOISE_DOTS, self._noise_dots, "noiseDots"),
            reinsertion=Reinsertion.RESPAWN,
            dot_lifetime_frames=lifetime_from_psychopy(self._dot_life),
            seed=self._seed,
        )

    def _dots(self) -> Any:
        return self._win._conn.stimuli.dots

    def _send_params(self) -> None:
        """For the parameters with no setter of their own; the seed is left alone."""
        self._win._dispatch(self._dots().set_params, self._handle, self._params())

    # ── autoDraw / draw ───────────────────────────────────────────────────────

    @property
    def win(self) -> Window:
        return self._win

    @property
    def autoDraw(self) -> bool:
        return self._auto_draw

    @autoDraw.setter
    def autoDraw(self, value: bool) -> None:
        self._auto_draw = bool(value)
        self._win._dispatch(self._win._conn.stimuli.set_enabled, self._handle, self._auto_draw)

    def setAutoDraw(self, value: bool, log: bool | None = None) -> None:
        self.autoDraw = value

    def draw(self, win: Window | None = None) -> None:
        if win is not None and win is not self._win:
            raise NotImplementedError("DotStim: drawing to another window is not supported.")
        self._win._to_draw_once.append(self._handle)

    def refreshDots(self) -> None:
        """Choose a new set of dots: move to the next seed, restarting the field."""
        self.seed = (self._seed + 1) % 2**64

    # ── Reproducibility (vstimd extension) ────────────────────────────────────

    @property
    def seed(self) -> int:
        """The seed the dots are drawn from. Record it to replay the stimulus."""
        return self._seed

    @seed.setter
    def seed(self, value: int) -> None:
        self._seed = int(value)
        self._win._dispatch(self._dots().set_seed, self._handle, self._seed)

    # ── Field ─────────────────────────────────────────────────────────────────

    @property
    def units(self) -> str:
        return self._units

    @property
    def nDots(self) -> int:
        return self._n_dots

    @nDots.setter
    def nDots(self, value: int) -> None:
        self._n_dots = int(value)
        self._win._dispatch(self._dots().set_dot_count, self._handle, self._n_dots)

    @property
    def fieldPos(self) -> tuple[float, float]:
        return self._field_pos

    @fieldPos.setter
    def fieldPos(self, value: PsychoPyVec2) -> None:
        self._field_pos = self._pair(value)
        self._win._dispatch(self._win._conn.stimuli.set_position, self._handle, self._center_px())

    def setFieldPos(self, val: PsychoPyVec2, op: str = "", log: bool | None = None) -> None:
        self.fieldPos = _apply_op(self._field_pos, val, op)

    @property
    def pos(self) -> tuple[float, float]:
        return self._field_pos

    @pos.setter
    def pos(self, value: PsychoPyVec2) -> None:
        self.fieldPos = value

    def setPos(self, newPos: PsychoPyVec2 | None = None, operation: str = "",
               units: str | None = None, log: bool | None = None) -> None:
        """Obsolete in PsychoPy, which logs an error and does nothing. So does this."""
        _log.error("User called DotStim.setPos(pos). Use DotStim.SetFieldPos(pos) instead.")

    @property
    def fieldSize(self) -> tuple[float, float]:
        return self._field_size

    @fieldSize.setter
    def fieldSize(self, value: PsychoPyVec2 | float) -> None:
        self._field_size = self._pair(value)
        w, h = self._field_px()
        shape = Region.ellipse if self._field_shape == "circle" else Region.rect
        self._win._dispatch(self._dots().set_field, self._handle, shape(w, h))
        # An anchor other than the centre moves the centre with the size.
        self._win._dispatch(self._win._conn.stimuli.set_position, self._handle, self._center_px())

    def setFieldSize(self, val: PsychoPyVec2 | float, op: str = "", log: bool | None = None) -> None:
        self.fieldSize = _apply_op(self._field_size, val, op)

    @property
    def size(self) -> tuple[float, float]:
        return self._field_size

    @size.setter
    def size(self, value: PsychoPyVec2 | float) -> None:
        self.fieldSize = value

    @property
    def fieldShape(self) -> str:
        return self._field_shape

    @fieldShape.setter
    def fieldShape(self, value: str | None) -> None:
        self._field_shape = _field_shape(value)
        self._send_params()

    @property
    def anchor(self) -> str:
        return self._anchor

    @anchor.setter
    def anchor(self, value: str) -> None:
        self._anchor = value
        self._win._dispatch(self._win._conn.stimuli.set_position, self._handle, self._center_px())

    def setAnchor(self, value: str, log: bool | None = None) -> None:
        self.anchor = value

    # ── Dots ──────────────────────────────────────────────────────────────────

    @property
    def dotSize(self) -> float:
        return self._dot_size

    @dotSize.setter
    def dotSize(self, value: float) -> None:
        if isinstance(value, (tuple, list)):
            raise NotImplementedError("DotStim: per-dot dotSize arrays are not supported.")
        self._dot_size = float(value)
        self._win._dispatch(self._dots().set_dot_size, self._handle, self._dot_size)

    @property
    def dotLife(self) -> int:
        return self._dot_life

    @dotLife.setter
    def dotLife(self, value: int) -> None:
        self._dot_life = int(value)
        self._win._dispatch(
            self._dots().set_dot_lifetime, self._handle, lifetime_from_psychopy(self._dot_life)
        )

    @property
    def element(self) -> None:
        return None

    @element.setter
    def element(self, value: Any) -> None:
        if value is not None:
            raise NotImplementedError("DotStim: element is not supported; dots are drawn by the server.")

    # ── Motion ────────────────────────────────────────────────────────────────

    @property
    def dir(self) -> float:
        return self._dir

    @dir.setter
    def dir(self, value: float) -> None:
        self._dir = float(value)
        self._win._dispatch(self._dots().set_direction, self._handle, self._dir)

    def setDir(self, val: float, op: str = "", log: bool | None = None) -> None:
        self.dir = _apply_op(self._dir, val, op)

    @property
    def speed(self) -> float:
        return self._speed

    @speed.setter
    def speed(self, value: float) -> None:
        self._speed = float(value)
        self._win._dispatch(self._dots().set_speed, self._handle, self._speed_px_per_s())

    def setSpeed(self, val: float, op: str = "", log: bool | None = None) -> None:
        self.speed = _apply_op(self._speed, val, op)

    @property
    def coherence(self) -> float:
        return self._coherence

    @coherence.setter
    def coherence(self, value: float) -> None:
        self._coherence = self._rounded_coherence(float(value))
        self._win._dispatch(self._dots().set_coherence, self._handle, self._coherence)

    def setFieldCoherence(self, val: float, op: str = "", log: bool | None = None) -> None:
        self.coherence = _apply_op(self._coherence, val, op)

    @property
    def signalDots(self) -> str:
        return self._signal_dots

    @signalDots.setter
    def signalDots(self, value: str) -> None:
        _lookup(_SIGNAL_DOTS, value, "signalDots")
        self._signal_dots = value
        self._send_params()

    @property
    def noiseDots(self) -> str:
        return self._noise_dots

    @noiseDots.setter
    def noiseDots(self, value: str) -> None:
        _lookup(_NOISE_DOTS, value, "noiseDots")
        self._noise_dots = value
        self._send_params()

    # ── Colour ────────────────────────────────────────────────────────────────

    def _resend_color(self) -> None:
        self._win._dispatch(self._dots().set_dot_color, self._handle, self._dot_color())

    @property
    def color(self) -> PsychoPyColor:
        return self._color

    @color.setter
    def color(self, value: PsychoPyColor) -> None:
        self._color = value
        self._resend_color()

    @property
    def foreColor(self) -> PsychoPyColor:
        return self._color

    @foreColor.setter
    def foreColor(self, value: PsychoPyColor) -> None:
        self.color = value

    @property
    def foreColorSpace(self) -> str:
        return self._color_space

    @foreColorSpace.setter
    def foreColorSpace(self, value: str) -> None:
        self.colorSpace = value

    @property
    def colorSpace(self) -> str:
        return self._color_space

    @colorSpace.setter
    def colorSpace(self, value: str) -> None:
        if value not in _COLOR_SPACES:
            raise NotImplementedError(
                f"DotStim: colorSpace={value!r} is not supported. "
                "Supported values: 'rgb', 'rgb255', 'rgb1'."
            )
        self._color_space = value
        self._resend_color()

    def setColor(self, color: PsychoPyColor, colorSpace: str | None = None,
                 operation: str = "", log: bool | None = None) -> None:
        if colorSpace is not None:
            self.colorSpace = colorSpace
        self.color = color

    def setForeColor(self, color: PsychoPyColor, colorSpace: str | None = None,
                     operation: str = "", log: bool | None = None) -> None:
        self.setColor(color, colorSpace=colorSpace, operation=operation, log=log)

    @property
    def contrast(self) -> float:
        return self._contrast

    @contrast.setter
    def contrast(self, value: float) -> None:
        self._contrast = float(value)
        self._resend_color()

    def setContrast(self, newContrast: float, operation: str = "", log: bool | None = None) -> None:
        self.contrast = _apply_op(self._contrast, newContrast, operation)

    @property
    def opacity(self) -> float:
        return self._opacity

    @opacity.setter
    def opacity(self, value: float | None) -> None:
        self._opacity = 1.0 if value is None else float(value)
        self._win._dispatch(self._win._conn.stimuli.set_alpha, self._handle, self._opacity)

    def setOpacity(self, newOpacity: float, operation: str = "", log: bool | None = None) -> None:
        self.opacity = _apply_op(self._opacity, newOpacity, operation)

    # ── Deprecated ────────────────────────────────────────────────────────────

    def set(self, attrib: str, val: Any, op: str = "", log: bool | None = None) -> None:
        """Deprecated in PsychoPy: use the specific setter instead."""
        if not hasattr(type(self), attrib) or not isinstance(getattr(type(self), attrib), property):
            raise AttributeError(f"DotStim has no settable attribute {attrib!r}")
        setattr(self, attrib, _apply_op(getattr(self, attrib), val, op))


def _add_accessors() -> None:
    """PsychoPy generates ``getX``/``setX`` for its attributes; these are the ones a
    dot field has. ``setX(value, operation, log)`` takes the operation as the
    attribute setters do."""
    for attr in ("coherence", "color", "colorSpace", "contrast", "dir", "dotLife", "dotSize",
                 "element", "fieldPos", "fieldShape", "fieldSize", "foreColor", "foreColorSpace", "noiseDots",
                 "signalDots", "speed"):
        cap = attr[0].upper() + attr[1:]

        def getter(self: DotStim, _attr: str = attr) -> Any:
            return getattr(self, _attr)

        def setter(self: DotStim, value: Any, operation: str = "", log: bool | None = None,
                   _attr: str = attr) -> None:
            setattr(self, _attr, _apply_op(getattr(self, _attr), value, operation))

        getter.__name__ = f"get{cap}"
        setter.__name__ = f"set{cap}"
        setattr(DotStim, f"get{cap}", getter)
        if not hasattr(DotStim, f"set{cap}"):
            setattr(DotStim, f"set{cap}", setter)


_add_accessors()

__all__ = ["DotStim"]
