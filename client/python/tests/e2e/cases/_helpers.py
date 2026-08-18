"""Shared helpers for e2e test cases."""

from __future__ import annotations

import time

from vstimd import Connection
from vstimd.animations import AnimationHandle, AnimationState
from vstimd.stimuli import RectParams, ShapeAppearance, StimulusHandle, TextParams
from vstimd.stimuli.stimuli_models import Color, Vec2


def label(conn: Connection, test_id: str, description: str = "") -> StimulusHandle:
    """Yellow label near top of screen: '[test_id] description'."""
    text = f"[{test_id}] {description}".rstrip()
    return conn.stimuli.text.create_text(
        position_px=Vec2(0, 250),
        name="_label",
        params=TextParams(
            text=text,
            letter_height_px=28,
            text_color=Color(1.0, 1.0, 0.0),
            anchor="center",
            box_size_px=Vec2(900, 200),
        ),
    )


def update_label(
    conn: Connection, handle: StimulusHandle, test_id: str, description: str
) -> None:
    conn.stimuli.text.set_text(handle, f"[{test_id}] {description}")


def wait_for_anim_state(
    conn: Connection,
    handle: AnimationHandle,
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


def make_rect(
    conn: Connection, *, x: float = 0, y: float = 0, enabled: bool = True
) -> StimulusHandle:
    h = conn.stimuli.shapes.create_rect(
        position_px=Vec2(x, y),
        params=RectParams(
            width_px=80,
            height_px=80,
            appearance=ShapeAppearance(fill_color=Color(0.8, 0.2, 0.2)),
        ),
    )
    if not enabled:
        conn.stimuli.set_enabled(h, False)
    return h


def wait_for_anim_run_start(
    conn: Connection,
    handle: AnimationHandle,
    timeout: float = 4.0,
    poll_interval: float = 0.02,
) -> AnimationState:
    """Poll until a triggered animation has left ARMED, i.e. its run began.

    The server consumes a trigger edge on its next frame, so an animation stays
    ARMED for a moment after the pulse that fires it. Polling straight for the
    state a REARM animation returns to would match the ARMED it started from and
    return before the run ever began — the wait has to see it leave first.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = conn.animations.query(handle).state
        if state != AnimationState.ARMED:
            return state
        time.sleep(poll_interval)
    return conn.animations.query(handle).state
