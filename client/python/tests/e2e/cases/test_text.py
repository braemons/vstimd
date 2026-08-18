"""E2E tests for text stimuli."""
from __future__ import annotations

import time

import pytest

from vstimd import Connection
from vstimd.stimuli import TextParams
from vstimd.stimuli.stimuli_models import Color, Vec2
from ._helpers import label as _label, update_label as _update_label


def test_create_text(conn: Connection) -> None:
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
    conn.stimuli.delete(handle)


def test_set_text(conn: Connection) -> None:
    handle = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(text="before", letter_height_px=40, box_size_px=Vec2(400, 80)),
    )
    conn.stimuli.text.set_text(handle, "after")
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, TextParams)
    assert info.params.text == "after"
    conn.stimuli.delete(handle)


def test_set_text_color(conn: Connection) -> None:
    handle = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(
            text="Color test",
            letter_height_px=40,
            text_color=Color(1.0, 1.0, 1.0),
            box_size_px=Vec2(400, 80),
        ),
    )
    conn.stimuli.text.set_text_color(handle, Color(1.0, 0.0, 0.0))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, TextParams)
    assert info.params.text_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.text_color.g == pytest.approx(0.0, abs=0.01)
    assert info.params.text_color.b == pytest.approx(0.0, abs=0.01)
    conn.stimuli.delete(handle)


def test_text_visual(conn: Connection, step_delay: float, request: pytest.FixtureRequest) -> None:
    """Show text stimuli in various states so a human can visually verify rendering."""
    tid = request.node.name
    conn.system.set_background(r=0.1, g=0.1, b=0.1)

    lbl = _label(conn, tid, "white text")
    h = conn.stimuli.text.create_text(
        position_px=Vec2(0, 0),
        params=TextParams(
            text="Hello vstimd",
            letter_height_px=56,
            text_color=Color(1.0, 1.0, 1.0),
            box_size_px=Vec2(600, 100),
        ),
    )
    time.sleep(step_delay)

    conn.stimuli.text.set_text(h, "Updated text!")
    _update_label(conn, lbl, tid, "text updated")
    time.sleep(step_delay)

    conn.stimuli.text.set_text_color(h, Color(1.0, 0.8, 0.0))
    _update_label(conn, lbl, tid, "yellow colour")
    time.sleep(step_delay)

    conn.stimuli.text.set_text(h, "Step 7 works!")
    conn.stimuli.text.set_text_color(h, Color(0.4, 1.0, 0.4))
    _update_label(conn, lbl, tid, "green, new content")
    time.sleep(step_delay)

    conn.stimuli.delete(h)
    conn.stimuli.delete(lbl)
    conn.system.set_background(r=0.0, g=0.0, b=0.0)
