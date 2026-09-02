"""Psychopy visual API tests — Rect."""
from __future__ import annotations

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import RectParams, StimulusType
from ..cases._helpers import Stage


@pytest.mark.onscreen(
    "PSY-01",
    "a red 200×100 px rectangle in the centre, built through the "
    "PsychoPy-style visual.Rect API with autoDraw on",
)
def test_create_rect(win: visual.Window, stage: Stage) -> None:
    rect = visual.Rect(win, width_px=200, height_px=100, fillColor="red", autoDraw=True)

    info = win._conn.stimuli.query(rect._handle)
    assert info.stimulus_type == StimulusType.RECT
    assert isinstance(info.params, RectParams)
    assert info.params.width_px == pytest.approx(200.0, abs=0.5)
    assert info.params.height_px == pytest.approx(100.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(1.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(0.0, abs=0.01)

    win.flip()
    stage.hold()
    rect.autoDraw = False


@pytest.mark.onscreen(
    "PSY-02",
    "a blue 400×300 px rect in the centre, which becomes a small green "
    "100×100 px square at the top right, then a yellow one at the bottom "
    "left",
)
def test_rect_position_size(win: visual.Window, stage: Stage) -> None:
    rect = visual.Rect(win, width_px=400, height_px=300, fillColor="blue", pos=(0, 0), autoDraw=True)
    win.flip()
    stage.hold()

    stage.show("green 100×100 top-right")
    rect.size = (100, 100)
    rect.pos_px = (300, 200)
    rect.fillColor = "green"
    win.flip()
    stage.hold()

    stage.show("yellow 100×100 bottom-left")
    rect.pos_px = (-300, -200)
    rect.fillColor = "yellow"
    win.flip()
    stage.hold()

    rect.autoDraw = False


@pytest.mark.onscreen(
    "PSY-03",
    "one 200×200 px square in the centre cycling through red, green, blue, "
    "white and orange — the last one set as an rgb1 tuple rather than a "
    "name",
)
def test_rect_colors(win: visual.Window, stage: Stage) -> None:
    rect = visual.Rect(win, width_px=200, height_px=200, fillColor="red", autoDraw=True)
    win.flip()
    stage.hold()

    for color, name in [("green", "green"), ("blue", "blue"), ("white", "white"),
                        ((1.0, 0.5, 0.0), "orange (rgb1 tuple)")]:
        stage.show(name)
        rect.fillColor = color
        win.flip()
        stage.hold()

    rect.autoDraw = False


@pytest.mark.onscreen(
    "PSY-04",
    "two overlapping 300×300 px squares, red on the left and blue on the "
    "right; the blue fades to 0.5 and then the red to 0.7, showing the "
    "overlap through",
)
def test_rect_opacity(win: visual.Window, stage: Stage) -> None:
    rect1 = visual.Rect(win, width_px=300, height_px=300, fillColor="red", pos=(-100, 0), autoDraw=True)
    rect2 = visual.Rect(win, width_px=300, height_px=300, fillColor="blue", pos=(100, 0), autoDraw=True)
    win.flip()
    stage.hold()

    stage.show("blue semi-transparent (0.5)")
    rect2.opacity = 0.5
    win.flip()
    stage.hold()

    stage.show("both semi-transparent (0.7 / 0.5)")
    rect1.opacity = 0.7
    win.flip()
    stage.hold()

    rect1.autoDraw = False
    rect2.autoDraw = False
