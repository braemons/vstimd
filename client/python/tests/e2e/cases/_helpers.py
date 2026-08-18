"""Shared helpers for e2e test cases."""

from __future__ import annotations

import time
from typing import Callable

from vstimd import Connection, HandleNotFoundError
from vstimd.animations import AnimationHandle, AnimationState
from vstimd.stimuli import RectParams, ShapeAppearance, StimulusHandle, TextParams
from vstimd.stimuli.stimuli_models import Color, Vec2

#: Near the top of the frame, clear of where demos put their own titles.
CAPTION_POS = Vec2(0.0, 420.0)
CAPTION_BOX = Vec2(1600.0, 80.0)


class Stage:
    """The on-screen caption for one e2e test, plus the dwells that make it readable.

    Every test writes ``[GRT-04] what should be on screen right now`` in yellow
    near the top of the display. The id in front is the point: an operator
    watching the suite run can note down the id of anything that looks wrong and
    find the test again with ``grep -rn GRT-04 tests/e2e``.

    All dwelling goes through here, and every dwell is a multiple of the
    ``--step-delay`` option — which the null suites pin to 0, so none of this
    costs a headless run any time.
    """

    def __init__(
        self,
        conn: Connection,
        test_id: str,
        description: str,
        step_delay: float,
        pause: Callable[[str, str], None] = lambda where, prompt: None,
    ) -> None:
        self.conn = conn
        self.test_id = test_id
        self.description = description
        self.step_delay = step_delay
        self.pause = pause
        self._handle: StimulusHandle | None = None

    # ── caption ──────────────────────────────────────────────────────────────

    def _caption(self, description: str) -> str:
        return f"[{self.test_id}] {description}".rstrip()

    def show(self, description: str | None = None) -> None:
        """Put the caption on screen, replacing whatever was there.

        The caption is rebuilt rather than rewritten because draw order is
        creation order: a caption made once at the start of the test ends up
        *under* every stimulus the test creates afterwards, and a test that
        draws near the top of the frame can bury it. Creating the new one before
        deleting the old also means there is no frame without a caption.

        Tests that clear the whole scene take the caption with them, so a
        missing handle is a normal state to recover from rather than a failure.
        """
        if description is not None:
            self.description = description
        previous = self._handle
        self._handle = self.conn.stimuli.text.create_text(
            position_px=CAPTION_POS,
            name="_label",
            params=TextParams(
                text=self._caption(self.description),
                letter_height_px=28,
                text_color=Color(1.0, 1.0, 0.0),
                # A dim backing box: the caption has to stay readable over a
                # mid-grey background and over any stimulus it lands on.
                fill_color=Color(0.0, 0.0, 0.0, 0.7),
                anchor="center",
                box_size_px=CAPTION_BOX,
            ),
        )
        if previous is not None:
            try:
                self.conn.stimuli.delete(previous)
            except HandleNotFoundError:
                pass

    def cue(self, description: str, hold: float = 0.6) -> None:
        """Say what is *about* to happen, and pause long enough to read it.

        Several animations are over in well under a second — a caption that only
        goes up once they have started is a caption nobody gets to read.
        """
        self.step(description, hold=hold)

    def step(self, description: str, hold: float = 1.0) -> None:
        """Describe what is on screen *now*, then hold it there to be looked at."""
        self.show(description)
        self.hold(hold)

    def hold(self, factor: float = 1.0) -> None:
        """Leave the current frame up for ``factor`` step delays.

        With ``--pause=step`` the frame stays up until someone says otherwise,
        which is the only way to study something that a dwell always cuts short.
        """
        if self.step_delay > 0:
            time.sleep(self.step_delay * factor)
        self.pause("step", self._caption(self.description))

    def close(self) -> None:
        if self._handle is None:
            return
        try:
            self.conn.stimuli.delete(self._handle)
        except HandleNotFoundError:
            pass  # a scene-clearing test already took it
        self._handle = None


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
