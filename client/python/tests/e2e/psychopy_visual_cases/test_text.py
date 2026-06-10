"""Psychopy visual API tests — TextBox2."""
from __future__ import annotations

import time

import pytest

import vstimd.psychopy.visual as visual
from ._helpers import label as _label, update_label as _update_label


def test_create_textbox2(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "white 'Hello vstimd'")
    tb = visual.TextBox2(
        win, text="Hello vstimd",
        pos=(0, 0), size=(600, 100), letterHeight=56,
        color="white", autoDraw=True,
    )
    win.flip()
    time.sleep(step_delay)
    tb.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_textbox2_text_update(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "'Before'")
    tb = visual.TextBox2(win, text="Before", pos=(0, 0),
                         size=(600, 100), letterHeight=56,
                         color="white", autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    _update_label(win, lbl, tid, "'After'")
    tb.text = "After"
    win.flip()
    time.sleep(step_delay)

    tb.autoDraw = False
    win._conn.stimuli.delete(lbl)


def test_textbox2_colors(win: visual.Window, step_delay: float, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(win, tid, "white")
    tb = visual.TextBox2(win, text="Color test", pos=(0, 0),
                         size=(500, 100), letterHeight=56,
                         color="white", autoDraw=True)
    win.flip()
    time.sleep(step_delay)

    for color, name in [("red", "red"), ("cyan", "cyan"), ("yellow", "yellow")]:
        _update_label(win, lbl, tid, name)
        tb.color = color
        win.flip()
        time.sleep(step_delay)

    tb.autoDraw = False
    win._conn.stimuli.delete(lbl)
