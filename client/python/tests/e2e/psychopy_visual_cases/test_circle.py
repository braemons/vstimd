"""Psychopy visual API tests — Circle."""
from __future__ import annotations

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import CircleParams, StimulusType
from ..cases._helpers import Stage


@pytest.mark.onscreen(
    "PSY-05",
    "a blue disc of radius 50 px (100 px across) in the centre, built "
    "through visual.Circle",
)
def test_create_circle(win: visual.Window, stage: Stage) -> None:
    circle = visual.Circle(win, radius=50, fillColor="blue", autoDraw=True)

    info = win._conn.stimuli.query(circle._handle)
    assert info.stimulus_type == StimulusType.CIRCLE
    assert isinstance(info.params, CircleParams)
    # visual.Circle takes a radius; the server reports the full extent.
    assert info.params.diameter_px == pytest.approx(100.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(1.0, abs=0.01)

    win.flip()
    stage.hold()
    circle.autoDraw = False


@pytest.mark.onscreen(
    "PSY-06",
    "a large red disc in the centre, then a small green one top-left, then "
    "a yellow one bottom-right, and finally a red/green/blue trio in a row",
)
def test_circle_sizes(win: visual.Window, stage: Stage) -> None:
    circle = visual.Circle(win, radius=150, fillColor="red", pos=(0, 0), autoDraw=True)
    win.flip()
    stage.hold()

    stage.show("green r=50 top-left")
    circle.radius = 50
    circle.pos_px = (-200, 150)
    circle.fillColor = "green"
    win.flip()
    stage.hold()

    stage.show("yellow r=100 bottom-right")
    circle.radius = 100
    circle.pos_px = (200, -150)
    circle.fillColor = "yellow"
    win.flip()
    stage.hold()

    circle.autoDraw = False

    stage.show("RGB trio r=60")
    c1 = visual.Circle(win, radius=60, fillColor="red",   pos=(-150, 0), autoDraw=True)
    c2 = visual.Circle(win, radius=60, fillColor="green", pos=(0, 0),    autoDraw=True)
    c3 = visual.Circle(win, radius=60, fillColor="blue",  pos=(150, 0),  autoDraw=True)
    win.flip()
    stage.hold()

    c1.autoDraw = False
    c2.autoDraw = False
    c3.autoDraw = False
