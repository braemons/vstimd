"""Psychopy visual API tests — TextBox2."""
from __future__ import annotations

import pytest

import vstimd.psychopy.visual as visual
from ..cases._helpers import Stage


@pytest.mark.onscreen(
    "PSY-07",
    "white 56 px text reading 'Hello vstimd' in the centre, built through "
    "visual.TextBox2",
)
def test_create_textbox2(win: visual.Window, stage: Stage) -> None:
    tb = visual.TextBox2(
        win, text="Hello vstimd",
        pos=(0, 0), size=(600, 100), letterHeight=56,
        color="white", autoDraw=True,
    )
    win.flip()
    stage.hold()
    tb.autoDraw = False


@pytest.mark.onscreen(
    "PSY-08",
    "centre text reading 'Before' that is rewritten in place to 'After'",
)
def test_textbox2_text_update(win: visual.Window, stage: Stage) -> None:
    tb = visual.TextBox2(win, text="Before", pos=(0, 0),
                         size=(600, 100), letterHeight=56,
                         color="white", autoDraw=True)
    win.flip()
    stage.hold()

    stage.show("'After'")
    tb.text = "After"
    win.flip()
    stage.hold()

    tb.autoDraw = False


@pytest.mark.onscreen(
    "PSY-09",
    "centre text reading 'Color test' in white, then red, then cyan, then "
    "yellow — same wording throughout",
)
def test_textbox2_colors(win: visual.Window, stage: Stage) -> None:
    tb = visual.TextBox2(win, text="Color test", pos=(0, 0),
                         size=(500, 100), letterHeight=56,
                         color="white", autoDraw=True)
    win.flip()
    stage.hold()

    for color, name in [("red", "red"), ("cyan", "cyan"), ("yellow", "yellow")]:
        stage.show(name)
        tb.color = color
        win.flip()
        stage.hold()

    tb.autoDraw = False
