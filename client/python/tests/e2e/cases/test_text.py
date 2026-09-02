"""E2E tests for text stimuli."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import TextParams
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "TEXT-01",
    "white 48 px text reading 'Hello vstimd' in the centre of the screen",
)
def test_create_text(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(
            text="Hello vstimd",
            letter_height_px=48,
            text_color=Color(1.0, 1.0, 1.0),
            box_size_px=Vec2(400, 80),
        ),
    )
    assert handle > 0
    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "TEXT-02",
    "centre text reading 'before', which is then replaced in place by 'after'",
)
def test_set_text(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(text="before", letter_height_px=40, box_size_px=Vec2(400, 80)),
    )
    stage.step("centre text reads 'before'", hold=0.5)

    conn.stimuli.text.set_text(handle, "after")
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, TextParams)
    assert info.params.text == "after"

    stage.step("the same stimulus now reads 'after'")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "TEXT-03",
    "centre text reading 'Color test' in white, which then turns pure red "
    "without changing its wording or position",
)
def test_set_text_color(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(
            text="Color test",
            letter_height_px=40,
            text_color=Color(1.0, 1.0, 1.0),
            box_size_px=Vec2(400, 80),
        ),
    )
    stage.step("'Color test' in white", hold=0.5)

    conn.stimuli.text.set_text_color(handle, Color(1.0, 0.0, 0.0))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, TextParams)
    assert info.params.text_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.text_color.g == pytest.approx(0.0, abs=0.01)
    assert info.params.text_color.b == pytest.approx(0.0, abs=0.01)

    stage.step("the same text, now red")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "TEXT-04",
    "on a near-black background, one 56 px line in the centre changes wording "
    "and colour four times: white 'Hello vstimd' → white 'Updated text!' → "
    "yellow → green 'Step 7 works!'",
)
def test_text_visual(conn: Connection, stage: Stage) -> None:
    """Show text stimuli in various states so a human can visually verify rendering."""
    conn.system.set_background(r=0.1, g=0.1, b=0.1)

    h = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(
            text="Hello vstimd",
            letter_height_px=56,
            text_color=Color(1.0, 1.0, 1.0),
            box_size_px=Vec2(600, 100),
        ),
    )
    stage.step("white 'Hello vstimd'")

    conn.stimuli.text.set_text(h, "Updated text!")
    stage.step("same white line, now reading 'Updated text!'")

    conn.stimuli.text.set_text_color(h, Color(1.0, 0.8, 0.0))
    stage.step("same wording, colour changed to warm yellow")

    conn.stimuli.text.set_text(h, "Step 7 works!")
    conn.stimuli.text.set_text_color(h, Color(0.4, 1.0, 0.4))
    stage.step("light green 'Step 7 works!'")

    conn.stimuli.delete(h)
    conn.system.set_background(r=0.0, g=0.0, b=0.0)
