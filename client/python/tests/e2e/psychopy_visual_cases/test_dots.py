"""Psychopy visual API tests — DotStim."""
from __future__ import annotations

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import (
    CoherenceCount,
    DotShape,
    DotsParams,
    NoiseRule,
    RegionShape,
    Reinsertion,
    SignalRule,
    StimulusType,
)
from ..cases._helpers import Stage


def _params(win: visual.Window, stim: visual.DotStim) -> DotsParams:
    info = win._conn.stimuli.query(stim._handle)
    assert info.stimulus_type == StimulusType.DOTS
    assert isinstance(info.params, DotsParams)
    return info.params


@pytest.mark.onscreen(
    "PSY-18",
    "PsychoPy's dots demo: 500 small white square dots in a 400 px circle, 90% of "
    "them drifting down together, the rest each in a random direction of its own",
)
def test_dotstim_demo(win: visual.Window, stage: Stage) -> None:
    # demos/coder/stimuli/dots.py, in pixels: fieldSize=1 norm on a 600 px window.
    dots = visual.DotStim(
        win, units="pix", color=(1.0, 1.0, 1.0), dir=270, nDots=500,
        fieldShape="circle", fieldPos=(0.0, 0.0), fieldSize=400, dotLife=5,
        signalDots="same", noiseDots="direction", speed=3, coherence=0.9,
        dotSize=3, seed=1, autoDraw=True,
    )
    win.flip()

    p = _params(win, dots)
    assert p.field is not None
    assert p.field.shape == RegionShape.ELLIPSE, "fieldShape='circle' is the field, not a mask"
    assert (p.field.width_px, p.field.height_px) == pytest.approx((400.0, 400.0))
    assert p.dot_count == 500
    assert p.dot_shape == DotShape.SQUARE
    assert p.dot_size_px == pytest.approx(3.0)
    assert p.direction_deg == pytest.approx(270.0)
    assert p.coherence == pytest.approx(0.9)
    assert p.coherence_count == CoherenceCount.EXACT
    assert p.signal_rule == SignalRule.SAME
    assert p.noise_rule == NoiseRule.DIRECTION
    assert p.reinsertion == Reinsertion.RESPAWN
    assert p.dot_lifetime_frames == 5
    assert p.seed == 1
    # 3 px per frame at the rig's nominal rate.
    rate = win._conn.system.query_server_info().frame_rate_hz
    assert p.speed_px_per_s == pytest.approx(3 * rate, rel=1e-3)

    stage.hold(3)
    dots.autoDraw = False


@pytest.mark.onscreen(
    "PSY-19",
    "a 300 × 300 px square field of 200 dots moving right; it turns up, loses half "
    "its coherence, and then its noise dots switch to jumping to a new place every "
    "frame while the signal dots keep moving up",
)
def test_dotstim_live_changes(win: visual.Window, stage: Stage) -> None:
    dots = visual.DotStim(
        win, units="pix", nDots=200, fieldSize=300, dotSize=4, speed=2,
        coherence=1.0, dotLife=-1, seed=2, autoDraw=True,
    )
    win.flip()
    stage.hold(2)

    stage.show("dir 90°, coherence 0.5")
    dots.setDir(90, "+")
    dots.coherence = 0.5
    win.flip()
    stage.hold(2)
    p = _params(win, dots)
    assert p.direction_deg == pytest.approx(90.0)
    assert p.coherence == pytest.approx(0.5)
    assert p.dot_lifetime_frames == 0, "dotLife=-1 is infinite"

    stage.show("noiseDots='position', signalDots='different'")
    dots.noiseDots = "position"
    dots.signalDots = "different"
    win.flip()
    stage.hold(2)
    p = _params(win, dots)
    assert p.noise_rule == NoiseRule.POSITION
    assert p.signal_rule == SignalRule.DIFFERENT
    assert p.direction_deg == pytest.approx(90.0), "resending the block kept the rest"
    assert p.seed == 2, "resending the block kept the seed"

    dots.autoDraw = False


@pytest.mark.onscreen(
    "PSY-20",
    "a field of red dots at 0.5 contrast (so a muted red on grey terms) that "
    "grows into a wide ellipse, redraws a fresh set of dots, then disappears",
)
def test_dotstim_field_colour_and_refresh(win: visual.Window, stage: Stage) -> None:
    dots = visual.DotStim(
        win, units="pix", nDots=300, fieldSize=(300, 300), dotSize=5, speed=1,
        color="red", contrast=0.5, seed=3, autoDraw=True,
    )
    win.flip()
    stage.hold()
    p = _params(win, dots)
    assert p.dot_color.r == pytest.approx(0.75, abs=0.01)
    assert p.dot_color.g == pytest.approx(0.25, abs=0.01)

    stage.show("fieldShape='circle', fieldSize=(600, 250)")
    dots.fieldShape = "circle"
    dots.fieldSize = (600, 250)
    win.flip()
    stage.hold()
    p = _params(win, dots)
    assert p.field is not None and p.field.shape == RegionShape.ELLIPSE
    assert (p.field.width_px, p.field.height_px) == pytest.approx((600.0, 250.0))

    stage.show("refreshDots()")
    dots.refreshDots()
    win.flip()
    stage.hold()
    assert _params(win, dots).seed == 4

    stage.show("hidden (autoDraw=False)")
    dots.autoDraw = False
    win.flip()
    assert win._conn.stimuli.query(dots._handle).enabled is False
