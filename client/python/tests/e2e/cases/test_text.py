"""E2E tests for text stimuli."""
from __future__ import annotations

import time

import pytest

from vstimd import Connection
from ._helpers import label as _label, update_label as _update_label


def test_create_text(conn: Connection) -> None:
    handle = conn.stimuli.create_text(
        text="Hello vstimd",
        x=0, y=0,
        box_width=400, box_height=80,
        letter_height=48,
        r=1.0, g=1.0, b=1.0, a=1.0,
    )
    assert handle > 0
    conn.stimuli.delete(handle)


def test_set_text(conn: Connection) -> None:
    handle = conn.stimuli.create_text(text="before", x=0, y=0,
                                      box_width=400, box_height=80, letter_height=40)
    conn.stimuli.set_text(handle, "after")
    conn.stimuli.delete(handle)


def test_set_text_color(conn: Connection) -> None:
    handle = conn.stimuli.create_text(text="Color test", x=0, y=0,
                                      box_width=400, box_height=80, letter_height=40,
                                      r=1.0, g=1.0, b=1.0, a=1.0)
    conn.stimuli.set_text_color(handle, r=1.0, g=0.0, b=0.0)
    conn.stimuli.delete(handle)


def test_text_visual(conn: Connection, step_delay: float, request: pytest.FixtureRequest) -> None:
    """Show text stimuli in various states so a human can visually verify rendering."""
    tid = request.node.name
    conn.system.set_background(r=0.1, g=0.1, b=0.1)

    lbl = _label(conn, tid, "white text")
    h = conn.stimuli.create_text(
        text="Hello vstimd",
        x=0, y=0,
        box_width=600, box_height=100,
        letter_height=56,
        r=1.0, g=1.0, b=1.0, a=1.0,
    )
    time.sleep(step_delay)

    conn.stimuli.set_text(h, "Updated text!")
    _update_label(conn, lbl, tid, "text updated")
    time.sleep(step_delay)

    conn.stimuli.set_text_color(h, r=1.0, g=0.8, b=0.0)
    _update_label(conn, lbl, tid, "yellow colour")
    time.sleep(step_delay)

    conn.stimuli.set_text(h, "Step 7 works!")
    conn.stimuli.set_text_color(h, r=0.4, g=1.0, b=0.4)
    _update_label(conn, lbl, tid, "green, new content")
    time.sleep(step_delay)

    conn.stimuli.delete(h)
    conn.stimuli.delete(lbl)
    conn.system.set_background(r=0.0, g=0.0, b=0.0)
