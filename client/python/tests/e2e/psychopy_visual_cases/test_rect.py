"""Psychopy visual API tests — Rect."""
from __future__ import annotations

import time

import pytest

import vstimd.psychopy.visual as visual
from vstimd.stimuli import RectParams, StimulusType
from ._helpers import label as _label, update_label as _update_label


def test_create_rect(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "red 200×100 rect")
    rect = visual.Rect(win, width=200, height=100, fillColor="red", autoDraw=True)

    info = win._conn.stimuli.query(rect._handle)
    assert info.stimulus_type == StimulusType.RECT
    assert isinstance(info.params, RectParams)
    assert info.params.width == pytest.approx(200.0, abs=0.5)
    assert info.params.height == pytest.approx(100.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(1.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(0.0, abs=0.01)

    win.flip()
    time.sleep(step_delay)
    rect.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_rect_position_size(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "blue 400×300 at centre")
    rect = visual.Rect(win, width=400, height=300, fillColor="blue", pos=(0, 0), autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "green 100×100 top-right")
    rect.size = (100, 100)
    rect.pos = (300, 200)
    rect.fillColor = "green"
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "yellow 100×100 bottom-left")
    rect.pos = (-300, -200)
    rect.fillColor = "yellow"
    win.flip()
    time.sleep(step_delay)

    rect.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_rect_colors(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "red")
    rect = visual.Rect(win, width=200, height=200, fillColor="red", autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    for color, name in [("green", "green"), ("blue", "blue"), ("white", "white"),
                        ((1.0, 0.5, 0.0), "orange (rgb1 tuple)")]:
        _update_label(win, lbl, tid, name)
        rect.fillColor = color
        win.flip()
        time.sleep(step_delay)

    rect.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_rect_opacity(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "red + blue, both opaque")
    rect1 = visual.Rect(win, width=300, height=300, fillColor="red", pos=(-100, 0), autoDraw=True)
    rect2 = visual.Rect(win, width=300, height=300, fillColor="blue", pos=(100, 0), autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "blue semi-transparent (0.5)")
    rect2.opacity = 0.5
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "both semi-transparent (0.7 / 0.5)")
    rect1.opacity = 0.7
    win.flip()
    time.sleep(step_delay)

    rect1.autoDraw = False
    rect2.autoDraw = False
    win._conn.stimuli.delete(lbl)
