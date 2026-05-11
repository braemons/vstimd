"""Shared visual API test cases (psychopy-compatible).

Imported by test_visual.py (real server) and test_visual_null.py (null server).
Each function receives a `win` fixture — a vstimd.psychopy.visual.Window — so the
same cases run against both backends.
"""

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import DiscParams, GratingParams, RectParams, StimulusType
from vstimd._proto.vstimd.v1 import stimuli_2d_pb2 as _pb2


def test_create_rect(win: visual.Window) -> None:
    rect = visual.Rect(win, width=200, height=100, fillColor="red")

    info = win._conn.stimuli.query(rect._handle)
    assert info.stimulus_type == StimulusType.RECT
    assert isinstance(info.params, RectParams)
    assert info.params.width == pytest.approx(200.0, abs=0.5)
    assert info.params.height == pytest.approx(100.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(1.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(0.0, abs=0.01)

    rect.draw()
    win.flip()


def test_create_circle(win: visual.Window) -> None:
    circle = visual.Circle(win, radius=50, fillColor="blue")

    info = win._conn.stimuli.query(circle._handle)
    assert info.stimulus_type == StimulusType.DISC
    assert isinstance(info.params, DiscParams)
    assert info.params.radius == pytest.approx(50.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(1.0, abs=0.01)

    circle.draw()
    win.flip()


def test_create_grating_default(win: visual.Window) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200)

    info = win._conn.stimuli.query(grat._handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == _pb2.WAVEFORM_TYPE_SIN
    assert info.params.mask == _pb2.MASK_TYPE_NONE
    assert info.params.contrast == pytest.approx(1.0, abs=0.01)
    assert info.params.drift_coupled is True

    grat.draw()
    win.flip()


def test_create_grating_sqr_circle_mask(win: visual.Window) -> None:
    grat = visual.GratingStim(
        win, tex="sqr", mask="circle",
        size=(300, 300), sf=0.03, phase=0.1, ori=30.0,
        color="white", contrast=0.75,
    )

    info = win._conn.stimuli.query(grat._handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == _pb2.WAVEFORM_TYPE_SQR
    assert info.params.mask == _pb2.MASK_TYPE_CIRCLE
    assert info.params.sf == pytest.approx(0.03, rel=1e-2)
    assert info.params.phase == pytest.approx(0.1, abs=0.01)
    assert info.params.contrast == pytest.approx(0.75, abs=0.01)

    grat.draw()
    win.flip()


def test_grating_mutate_sf_phase_contrast(win: visual.Window) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, sf=0.05)

    grat.sf = 0.1
    grat.phase = 0.5
    grat.contrast = 0.6

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf == pytest.approx(0.1, rel=1e-2)
    assert info.params.phase == pytest.approx(0.5, abs=0.01)
    assert info.params.contrast == pytest.approx(0.6, abs=0.01)


def test_grating_drift_extension(win: visual.Window) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, drift_speed=1.5)

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed == pytest.approx(1.5, abs=0.01)
    assert info.params.drift_coupled is True

    grat.drift_decoupled = True
    grat.drift_angle = 45.0
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle == pytest.approx(45.0, abs=0.1)


def test_grating_autodraw(win: visual.Window) -> None:
    grat = visual.GratingStim(win, tex="sin", size=100, autoDraw=True)

    info = win._conn.stimuli.query(grat._handle)
    assert info.enabled is True

    grat.autoDraw = False
    info = win._conn.stimuli.query(grat._handle)
    assert info.enabled is False


def test_grating_ori(win: visual.Window) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, ori=45.0)
    assert grat.ori == pytest.approx(45.0, abs=0.01)

    grat.ori = 90.0
    assert grat.ori == pytest.approx(90.0, abs=0.01)
