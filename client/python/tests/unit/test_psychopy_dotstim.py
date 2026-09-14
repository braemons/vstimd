"""``visual.DotStim`` — the translation to vstimd, without a server.

The window's connection is a mock, so every test reads back exactly what would
have crossed the wire: the ``DotsParams`` a create sends and the setter calls a
property change stages. What the server then does with them is the e2e suite's
job (``tests/e2e/psychopy_visual_cases/test_dots.py``).
"""

from __future__ import annotations

import inspect
from unittest.mock import MagicMock

import pytest

from vstimd.psychopy import visual
from vstimd.psychopy.visual.dot import DotStim
from vstimd.stimuli import (
    CoherenceCount,
    DotShape,
    DotsParams,
    NoiseRule,
    Region,
    RegionShape,
    Reinsertion,
    SignalRule,
)
from vstimd.stimuli.vec import Vec2


class FakeMonitor:
    def deg2pix(self, deg: float) -> float:
        return deg * 30.0

    def cm2pix(self, cm: float) -> float:
        return cm * 40.0


def make_win(size=(800, 600), units="pix", hz=60.0, deferred=False) -> visual.Window:
    win = visual.Window.__new__(visual.Window)
    win._conn = MagicMock()
    win._conn.stimuli.dots.create_dots.return_value = 7
    win.size = size
    win.units = units
    win.monitor = FakeMonitor()
    win.deferred = deferred
    win.colorSpace = "rgb"
    win._frame_rate_hz = hz
    win._queue = []
    win._to_draw_once = []
    return win


def created(win) -> tuple[Vec2, DotsParams]:
    kwargs = win._conn.stimuli.dots.create_dots.call_args.kwargs
    return kwargs["position_px"], kwargs["params"]


# ── Creation ──────────────────────────────────────────────────────────────────


def test_psychopys_look_and_rules_are_what_is_sent():
    win = make_win()
    DotStim(win, nDots=100, fieldSize=(200, 100), dotSize=3, dir=90, speed=2,
            coherence=0.3, dotLife=5, seed=11)
    _, p = created(win)
    assert p.field == Region.rect(200.0, 100.0)
    assert p.aperture.region is None, "the aperture is the field"
    assert p.dot_count == 100
    assert p.dot_size_px == 3.0
    assert p.dot_shape == DotShape.SQUARE, "PsychoPy draws aliased GL_POINTS"
    assert p.direction_deg == 90.0
    assert p.coherence == pytest.approx(0.3)
    assert p.coherence_count == CoherenceCount.EXACT
    assert p.reinsertion == Reinsertion.RESPAWN, "PsychoPy respawns dots that leave"
    assert p.signal_rule == SignalRule.SAME
    assert p.noise_rule == NoiseRule.DIRECTION
    assert p.dot_lifetime_frames == 5
    assert p.seed == 11


@pytest.mark.parametrize("shape, expected", [
    ("sqr", RegionShape.RECT), ("square", RegionShape.RECT),
    (None, RegionShape.RECT), ("circle", RegionShape.ELLIPSE),
])
def test_field_shape_is_the_field_not_a_mask(shape, expected):
    win = make_win()
    DotStim(win, fieldShape=shape, fieldSize=(300, 200))
    _, p = created(win)
    assert p.field.shape == expected
    # A circle field with an unequal size is an ellipse, as in PsychoPy.
    assert (p.field.width_px, p.field.height_px) == (300.0, 200.0)


def test_speed_per_frame_becomes_per_second_at_the_nominal_rate():
    win = make_win(hz=120.0)
    DotStim(win, speed=1.5)
    assert created(win)[1].speed_px_per_s == pytest.approx(180.0)


def test_units_convert_field_position_and_speed():
    win = make_win(units="deg")
    DotStim(win, fieldPos=(1, -2), fieldSize=10, speed=0.1, dotSize=4)
    pos, p = created(win)
    assert (pos.x, pos.y) == (30.0, -60.0)
    assert p.field == Region.rect(300.0, 300.0), "a scalar fieldSize applies to both axes"
    assert p.speed_px_per_s == pytest.approx(0.1 * 30 * 60)
    assert p.dot_size_px == 4.0, "dotSize is always pixels"


def test_norm_units_on_a_non_square_window_warn():
    win = make_win(size=(800, 600), units="norm")
    with pytest.warns(UserWarning, match="norm"):
        DotStim(win, fieldSize=(1, 1))
    assert created(win)[1].field == Region.rect(400.0, 300.0)


@pytest.mark.parametrize("coherence, n, stored", [(0.25, 10, 0.2), (0.33, 3, 1 / 3), (0.5, 7, 4 / 7)])
def test_coherence_is_stored_rounded_to_whole_dots(coherence, n, stored):
    win = make_win()
    stim = DotStim(win, nDots=n, coherence=coherence)
    assert stim.coherence == pytest.approx(stored)
    assert created(win)[1].coherence == pytest.approx(stored)


def test_coherence_out_of_range_is_rejected_as_in_psychopy():
    with pytest.raises(ValueError):
        DotStim(make_win(), coherence=1.5)


@pytest.mark.parametrize("life, frames", [(-1, 0), (0, 0), (1, 1), (12, 12)])
def test_psychopy_infinite_lifetimes_map_to_zero(life, frames):
    win = make_win()
    DotStim(win, dotLife=life)
    assert created(win)[1].dot_lifetime_frames == frames


def test_contrast_scales_colour_about_mid_grey():
    win = make_win()
    DotStim(win, color=(1.0, 1.0, 1.0), contrast=0.5)
    c = created(win)[1].dot_color
    assert (c.r, c.g, c.b) == pytest.approx((0.75, 0.75, 0.75))


def test_opacity_is_the_shared_stimulus_alpha():
    win = make_win()
    DotStim(win, opacity=0.4)
    win._conn.stimuli.set_alpha.assert_called_once_with(7, 0.4)


def test_anchor_places_the_field_by_its_corner():
    win = make_win()
    DotStim(win, fieldPos=(0, 0), fieldSize=(200, 100), fieldAnchor="top-left")
    pos, _ = created(win)
    assert (pos.x, pos.y) == (100.0, -50.0)


def test_a_seed_is_drawn_and_recorded_when_not_given():
    win = make_win()
    a = DotStim(win)
    b = DotStim(win)
    assert a.seed != b.seed
    assert created(win)[1].seed == b.seed


@pytest.mark.parametrize("kwargs", [
    {"element": object()},
    {"dotSize": [1, 2]},
    {"rgb": (1, 1, 1)},
    {"colorSpace": "dkl"},
    {"fieldShape": "hexagon"},
    {"noiseDots": "brownian"},
    {"signalDots": "sometimes"},
])
def test_unsupported_features_refuse_loudly(kwargs):
    with pytest.raises(NotImplementedError):
        DotStim(make_win(), **kwargs)


# ── Mutation ──────────────────────────────────────────────────────────────────


def test_setters_reach_the_matching_commands():
    win = make_win(hz=60.0)
    stim = DotStim(win, nDots=10, seed=3)
    dots = win._conn.stimuli.dots

    stim.dir = 45
    dots.set_direction.assert_called_with(7, 45.0)
    stim.setDir(10, "+")
    dots.set_direction.assert_called_with(7, 55.0)
    stim.speed = 2
    dots.set_speed.assert_called_with(7, 120.0)
    stim.coherence = 0.26
    dots.set_coherence.assert_called_with(7, pytest.approx(0.3))
    stim.dotSize = 5
    dots.set_dot_size.assert_called_with(7, 5.0)
    stim.dotLife = -1
    dots.set_dot_lifetime.assert_called_with(7, 0)
    stim.nDots = 40
    dots.set_dot_count.assert_called_with(7, 40)
    stim.fieldPos = (5, 6)
    win._conn.stimuli.set_position.assert_called_with(7, Vec2(5.0, 6.0))
    stim.fieldSize = (50, 60)
    dots.set_field.assert_called_with(7, Region.rect(50.0, 60.0))


def test_rule_and_shape_changes_resend_the_block_but_not_the_seed():
    win = make_win()
    stim = DotStim(win, seed=3)
    dots = win._conn.stimuli.dots
    stim.noiseDots = "walk"
    stim.signalDots = "different"
    stim.fieldShape = "circle"
    _, params = dots.set_params.call_args.args
    assert params.noise_rule == NoiseRule.WALK
    assert params.signal_rule == SignalRule.DIFFERENT
    assert params.field.shape == RegionShape.ELLIPSE
    dots.set_seed.assert_not_called()


def test_refresh_dots_moves_to_the_next_seed():
    win = make_win()
    stim = DotStim(win, seed=41)
    stim.refreshDots()
    assert stim.seed == 42
    win._conn.stimuli.dots.set_seed.assert_called_once_with(7, 42)


def test_deferred_windows_stage_until_flip():
    win = make_win(deferred=True)
    stim = DotStim(win)
    stim.dir = 180
    win._conn.stimuli.dots.set_direction.assert_not_called()
    win.flip()
    win._conn.stimuli.dots.set_direction.assert_called_once_with(7, 180.0)


def test_set_pos_does_nothing_as_in_psychopy(caplog):
    win = make_win()
    stim = DotStim(win)
    stim.setPos((10, 10))
    win._conn.stimuli.set_position.assert_not_called()
    assert "SetFieldPos" in caplog.text


# ── API surface ───────────────────────────────────────────────────────────────


def test_exported_from_visual():
    assert visual.DotStim is DotStim


def test_everything_psychopys_dotstim_defines_is_here():
    """Every public name ``psychopy.visual.dot.DotStim`` defines itself — its
    attributes, setters and methods, as opposed to the colour and window mix-ins
    every stimulus inherits — and every ``__init__`` parameter."""
    dot = pytest.importorskip("psychopy.visual.dot")
    # Accessors PsychoPy's mix-ins generate for colour roles a dot field has not
    # got (a background, a border, a fill, a font, a line) and for raw vertices.
    unrelated = ("Back", "Border", "border", "Fill", "Font", "Line", "RGB", "VerticesPix")
    theirs = {
        n for n in vars(dot.DotStim)
        if not n.startswith("_") and not any(u in n for u in unrelated)
    }
    missing = sorted(n for n in theirs if not hasattr(DotStim, n))
    assert not missing, f"DotStim is missing: {missing}"

    their_params = set(inspect.signature(dot.DotStim.__init__).parameters)
    our_params = set(inspect.signature(DotStim.__init__).parameters)
    assert their_params <= our_params, sorted(their_params - our_params)
