"""Shared helpers for e2e test cases."""

from __future__ import annotations

import time
from typing import Callable

from vstimd import Connection, HandleNotFoundError
from vstimd.animations import AnimationHandle, AnimationState
from vstimd.stimuli import RectParams, ShapeAppearance, StimulusHandle, TextParams
from vstimd.stimuli.stimuli_models import Color, Vec2

#: Screen size per connection, so the caption can be placed relative to it
#: rather than at a pixel offset that falls off a small window.
_SCREEN: dict[int, tuple[int, int]] = {}


def _caption_geometry(conn: Connection) -> tuple[Vec2, Vec2, float]:
    """Where the caption goes on *this* display: position, box, letter height.

    A fixed offset does not travel: y=420 is near the top of a 1080-line screen
    and off the bottom of a 720-line window, which is exactly what the browser
    opens. Everything here is a fraction of the real frame instead.
    """
    size = _SCREEN.get(id(conn))
    if size is None:
        info = conn.system.query_server_info()
        size = _SCREEN[id(conn)] = (info.width_px, info.height_px)
    width_px, height_px = size

    letter_height_px = min(32.0, max(16.0, height_px / 30.0))
    box = Vec2(min(width_px - 40.0, 1600.0), letter_height_px * 2.6)
    # A hair below the top edge, measuring from the centre the server places by.
    top = height_px / 2.0 - box.y / 2.0 - height_px * 0.02
    return Vec2(0.0, top), box, letter_height_px


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
        pause: Callable[["Stage"], None] = lambda stage: None,
        node_id: str = "",
    ) -> None:
        self.conn = conn
        self.test_id = test_id
        #: What the test as a whole should show — the marker's description. The
        #: caption below it changes as the test walks through its states.
        self.summary = description
        self.description = description
        self.step_delay = step_delay
        self.pause = pause
        #: pytest's own name for the test, so a flagged one can be re-run.
        self.node_id = node_id
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
        position_px, box_size_px, letter_height_px = _caption_geometry(self.conn)
        self._handle = self.conn.stimuli.text.create_text(
            position_px=position_px,
            name="_label",
            params=TextParams(
                text=self._caption(self.description),
                letter_height_px=letter_height_px,
                text_color=Color(1.0, 1.0, 0.0),
                # A dim backing box: the caption has to stay readable over a
                # mid-grey background and over any stimulus it lands on.
                fill_color=Color(0.0, 0.0, 0.0, 0.7),
                anchor="center",
                box_size_px=box_size_px,
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
        self.pause(self)

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
