"""Psychopy visual API tests — GratingStim."""
from __future__ import annotations

import time

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import GratingMask, GratingParams, GratingTexture, StimulusType
from ._helpers import label as _label, update_label as _update_label


def test_create_grating_default(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "sin, size=200, no mask")
    grat = visual.GratingStim(win, tex="sin", size=200, autoDraw=True)

    info = win._conn.stimuli.query(grat._handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == GratingTexture.SIN
    assert info.params.mask == GratingMask.NONE
    assert info.params.contrast == pytest.approx(1.0, abs=0.01)
    assert info.params.drift_coupled is True

    win.flip()
    time.sleep(step_delay)
    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_create_grating_sqr_circle_mask(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "sqr, circle mask, 30°, sf=0.03")
    grat = visual.GratingStim(
        win, tex="sqr", mask="circle", size=(300, 300),
        sf=0.03, phase=0.1, ori=30.0, color="white", contrast=0.75, autoDraw=True,
    )

    info = win._conn.stimuli.query(grat._handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == GratingTexture.SQR
    assert info.params.mask == GratingMask.CIRCLE
    assert info.params.sf == pytest.approx(0.03, rel=1e-2)
    assert info.params.phase == pytest.approx(0.1, abs=0.01)
    assert info.params.contrast == pytest.approx(0.75, abs=0.01)

    win.flip()
    time.sleep(step_delay)
    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_grating_mutate_sf_phase_contrast(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "sin sf=0.05")
    grat = visual.GratingStim(win, tex="sin", size=200, sf=0.05, autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "sf=0.1, phase=0.5, contrast=0.6")
    grat.sf = 0.1
    grat.phase = 0.5
    grat.contrast = 0.6
    win.flip()
    time.sleep(step_delay)

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf == pytest.approx(0.1, rel=1e-2)
    assert info.params.phase == pytest.approx(0.5, abs=0.01)
    assert info.params.contrast == pytest.approx(0.6, abs=0.01)

    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_grating_drift_extension(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "coupled, speed=1.5")
    grat = visual.GratingStim(win, tex="sin", size=200, drift_speed=1.5, autoDraw=True)
    win.flip()
    time.sleep(step_delay * 3)

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed == pytest.approx(1.5, abs=0.01)
    assert info.params.drift_coupled is True

    _update_label(win, lbl, tid, "decoupled, angle=45°")
    grat.drift_decoupled = True
    grat.drift_angle = 45.0
    win.flip()
    time.sleep(step_delay * 3)
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle == pytest.approx(45.0, abs=0.1)

    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_grating_autodraw(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "sin visible (autoDraw=True)")
    grat = visual.GratingStim(win, tex="sin", size=100, autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    info = win._conn.stimuli.query(grat._handle)
    assert info.enabled is True

    _update_label(win, lbl, tid, "hidden (autoDraw=False)")
    grat.autoDraw = False
    win.flip()
    time.sleep(step_delay)
    info = win._conn.stimuli.query(grat._handle)
    assert info.enabled is False

    win._conn.stimuli.delete(lbl)


def test_grating_two_color_create(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "red/blue fore/back")
    grat = visual.GratingStim(
        win, tex="sin", size=200,
        color=(1.0, 0.0, 0.0), colorSpace="rgb1",
        backColor=(0.0, 0.0, 1.0), autoDraw=True,
    )
    win.flip()
    time.sleep(step_delay)

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color[0] == pytest.approx(1.0, abs=0.01)
    assert info.params.fore_color[2] == pytest.approx(0.0, abs=0.01)
    assert info.params.fore_color[3] == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color[0] == pytest.approx(0.0, abs=0.01)
    assert info.params.back_color[2] == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color[3] == pytest.approx(1.0, abs=0.01)

    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_grating_color_setters(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "default sin")
    grat = visual.GratingStim(win, tex="sin", size=200, autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "foreColor orange")
    grat.color = (0.5, 0.25, 0.0)
    grat.colorSpace = "rgb1"
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "foreColor red")
    grat.foreColor = (1.0, 0.0, 0.0)
    win.flip()
    time.sleep(step_delay)
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color[0] == pytest.approx(1.0, abs=0.01)
    assert info.params.fore_color[1] == pytest.approx(0.0, abs=0.01)

    _update_label(win, lbl, tid, "backColor blue")
    grat.backColor = (0.0, 0.0, 1.0)
    win.flip()
    time.sleep(step_delay)
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.back_color[2] == pytest.approx(1.0, abs=0.01)

    _update_label(win, lbl, tid, "opacity=0.5")
    grat.opacity = 0.5
    win.flip()
    time.sleep(step_delay)
    info = win._conn.stimuli.query(grat._handle)
    assert info.params.opacity == pytest.approx(0.5, abs=0.01)
    assert info.params.fore_color[3] == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color[3] == pytest.approx(1.0, abs=0.01)

    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_grating_ori(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "45°")
    grat = visual.GratingStim(win, tex="sin", size=200, ori=45.0, autoDraw=True)
    win.flip()
    time.sleep(step_delay)
    assert grat.ori == pytest.approx(45.0, abs=0.01)

    _update_label(win, lbl, tid, "90°")
    grat.ori = 90.0
    win.flip()
    time.sleep(step_delay)
    assert grat.ori == pytest.approx(90.0, abs=0.01)

    grat.autoDraw = False
    win._conn.stimuli.delete(lbl)
