"""Shared helpers for e2e test cases."""
from __future__ import annotations

import time

from vstimd import Connection
from vstimd.animations import AnimationState


def label(conn: Connection, test_id: str, description: str = "") -> int:
    """Yellow label near top of screen: '[test_id] description'."""
    text = f"[{test_id}] {description}".rstrip()
    return conn.stimuli.create_text(
        text=text, x=0, y=260,
        box_width=900, box_height=50,
        letter_height=28,
        r=1.0, g=1.0, b=0.0, a=1.0,
        anchor="center",
        name="_label",
    )


def update_label(conn: Connection, handle: int, test_id: str, description: str) -> None:
    conn.stimuli.set_text(handle, f"[{test_id}] {description}")


def wait_for_anim_state(
    conn: Connection,
    handle: int,
    target: AnimationState,
    timeout: float = 3.0,
    poll_interval: float = 0.05,
) -> AnimationState:
    """Poll until the animation reaches ``target`` or ``timeout`` seconds pass."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = conn.animations.query(handle).state
        if state == target:
            return state
        time.sleep(poll_interval)
    return conn.animations.query(handle).state


def make_rect(conn: Connection, *, x: float = 0, y: float = 0, enabled: bool = True) -> int:
    h = conn.stimuli.create_rect(x=x, y=y, width=80, height=80, r=0.8, g=0.2, b=0.2)
    if not enabled:
        conn.stimuli.set_enabled(h, False)
    return h
