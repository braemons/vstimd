"""Psychopy visual API tests — Circle."""
from __future__ import annotations

import time

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import CircleParams, StimulusType
from ._helpers import label as _label, update_label as _update_label


def test_create_circle(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "blue r=50")
    circle = visual.Circle(win, radius=50, fillColor="blue", autoDraw=True)

    info = win._conn.stimuli.query(circle._handle)
    assert info.stimulus_type == StimulusType.CIRCLE
    assert isinstance(info.params, CircleParams)
    assert info.params.radius == pytest.approx(50.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(1.0, abs=0.01)

    win.flip()
    time.sleep(step_delay)
    circle.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_circle_sizes(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "red r=150 at centre")
    circle = visual.Circle(win, radius=150, fillColor="red", pos=(0, 0), autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "green r=50 top-left")
    circle.radius = 50
    circle.pos = (-200, 150)
    circle.fillColor = "green"
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "yellow r=100 bottom-right")
    circle.radius = 100
    circle.pos = (200, -150)
    circle.fillColor = "yellow"
    win.flip()
    time.sleep(step_delay)

    circle.autoDraw = False

    _update_label(win, lbl, tid, "RGB trio r=60")
    c1 = visual.Circle(win, radius=60, fillColor="red",   pos=(-150, 0), autoDraw=True)
    c2 = visual.Circle(win, radius=60, fillColor="green", pos=(0, 0),    autoDraw=True)
    c3 = visual.Circle(win, radius=60, fillColor="blue",  pos=(150, 0),  autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    c1.autoDraw = False
    c2.autoDraw = False
    c3.autoDraw = False
    win._conn.stimuli.delete(lbl)
