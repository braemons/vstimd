"""Psychopy visual API tests — GratingStim."""
from __future__ import annotations

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import GratingMask, GratingParams, GratingTexture, StimulusType
from ..cases._helpers import Stage


@pytest.mark.onscreen(
    "PSY-10",
    "a 200 px sinusoidal grating patch in the centre, unmasked, at full "
    "contrast — the defaults of visual.GratingStim",
)
def test_create_grating_default(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, autoDraw=True)

    info = win._conn.stimuli.query(grat._handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == GratingTexture.SIN
    assert info.params.mask == GratingMask.NONE
    assert info.params.contrast == pytest.approx(1.0, abs=0.01)
    assert info.params.drift_coupled is True

    win.flip()
    stage.hold()
    grat.autoDraw = False


@pytest.mark.onscreen(
    "PSY-11",
    "a 300 px square-wave grating masked to a disc, tilted 30°, at 0.75 "
    "contrast with fairly coarse stripes",
)
def test_create_grating_sqr_circle_mask(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(
        win, tex="sqr", mask="circle", size=(300, 300),
        sf_cycles_per_px=0.03, phase_cycles=0.1, ori=30.0, color="white", contrast=0.75, autoDraw=True,
    )

    info = win._conn.stimuli.query(grat._handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == GratingTexture.SQR
    assert info.params.mask == GratingMask.CIRCLE
    assert info.params.sf_cycles_per_px == pytest.approx(0.03, rel=1e-2)
    assert info.params.phase_cycles == pytest.approx(0.1, abs=0.01)
    assert info.params.contrast == pytest.approx(0.75, abs=0.01)

    win.flip()
    stage.hold()
    grat.autoDraw = False


@pytest.mark.onscreen(
    "PSY-12",
    "a 200 px sine grating whose stripes get twice as fine, shift half a "
    "cycle and drop to 0.6 contrast, all in one step",
)
def test_grating_mutate_sf_phase_contrast(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, sf_cycles_per_px=0.05, autoDraw=True)
    win.flip()
    stage.hold()

    stage.show("sf_cycles_per_px=0.1, phase_cycles=0.5, contrast=0.6")
    grat.sf_cycles_per_px = 0.1
    grat.phase_cycles = 0.5
    grat.contrast = 0.6
    win.flip()
    stage.hold()

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf_cycles_per_px == pytest.approx(0.1, rel=1e-2)
    assert info.params.phase_cycles == pytest.approx(0.5, abs=0.01)
    assert info.params.contrast == pytest.approx(0.6, abs=0.01)

    grat.autoDraw = False


@pytest.mark.onscreen(
    "PSY-13",
    "a 200 px sine grating drifting at 1.5 Hz across its stripes, then "
    "drifting at 45° to them once the direction is decoupled",
)
def test_grating_drift_extension(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, drift_speed_hz=1.5, autoDraw=True)
    win.flip()
    stage.hold(3)

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed_hz == pytest.approx(1.5, abs=0.01)
    assert info.params.drift_coupled is True

    stage.show("decoupled, angle=45°")
    grat.drift_decoupled = True
    grat.drift_angle_deg = 45.0
    win.flip()
    stage.hold(3)
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle_deg == pytest.approx(45.0, abs=0.1)

    grat.autoDraw = False


@pytest.mark.onscreen(
    "PSY-14",
    "a 100 px sine grating that disappears when autoDraw is switched off — "
    "the stimulus still exists on the server, it is just not drawn",
)
def test_grating_autodraw(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(win, tex="sin", size=100, autoDraw=True)
    win.flip()
    stage.hold()

    info = win._conn.stimuli.query(grat._handle)
    assert info.enabled is True

    stage.show("hidden (autoDraw=False)")
    grat.autoDraw = False
    win.flip()
    stage.hold()
    info = win._conn.stimuli.query(grat._handle)
    assert info.enabled is False



@pytest.mark.onscreen(
    "PSY-15",
    "a 200 px grating made of red and blue bars instead of greys, set "
    "through color/backColor in rgb1",
)
def test_grating_two_color_create(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(
        win, tex="sin", size=200,
        color=(1.0, 0.0, 0.0), colorSpace="rgb1",
        backColor=(0.0, 0.0, 1.0), autoDraw=True,
    )
    win.flip()
    stage.hold()

    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.fore_color.b == pytest.approx(0.0, abs=0.01)
    assert info.params.fore_color.a == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.r == pytest.approx(0.0, abs=0.01)
    assert info.params.back_color.b == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.a == pytest.approx(1.0, abs=0.01)

    grat.autoDraw = False


@pytest.mark.onscreen(
    "PSY-16",
    "a grey sine grating whose bars go orange, then red, then get blue "
    "backgrounds, then the whole patch fades to 0.5 opacity",
)
def test_grating_color_setters(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, autoDraw=True)
    win.flip()
    stage.hold()

    stage.show("foreColor orange")
    grat.color = (0.5, 0.25, 0.0)
    grat.colorSpace = "rgb1"
    win.flip()
    stage.hold()

    stage.show("foreColor red")
    grat.foreColor = (1.0, 0.0, 0.0)
    win.flip()
    stage.hold()
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.fore_color.g == pytest.approx(0.0, abs=0.01)

    stage.show("backColor blue")
    grat.backColor = (0.0, 0.0, 1.0)
    win.flip()
    stage.hold()
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.back_color.b == pytest.approx(1.0, abs=0.01)

    stage.show("opacity=0.5")
    grat.opacity = 0.5
    win.flip()
    stage.hold()
    info = win._conn.stimuli.query(grat._handle)
    assert isinstance(info.params, GratingParams)
    # Opacity is the shared per-stimulus property; the carrier colours keep
    # their own alphas underneath it.
    assert info.opacity == pytest.approx(0.5, abs=0.01)
    assert info.params.fore_color.a == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.a == pytest.approx(1.0, abs=0.01)

    grat.autoDraw = False


@pytest.mark.onscreen(
    "PSY-17",
    "a 200 px sine grating tilted 45°, which then turns to 90° — stripes "
    "diagonal, then horizontal",
)
def test_grating_ori(win: visual.Window, stage: Stage) -> None:
    grat = visual.GratingStim(win, tex="sin", size=200, ori=45.0, autoDraw=True)
    win.flip()
    stage.hold()
    assert grat.ori == pytest.approx(45.0, abs=0.01)

    stage.show("90°")
    grat.ori = 90.0
    win.flip()
    stage.hold()
    assert grat.ori == pytest.approx(90.0, abs=0.01)

    grat.autoDraw = False
