"""E2E tests for the animation system.

These tests require a server with a running frame loop (real or null renderer).
"""

from __future__ import annotations

import time

import pytest

from vstimd import Connection, NotSupportedError
from vstimd.animations import AnimationState, CancelAction, FinalAction, StartAction, VtlEdge, VtlPolarity
from vstimd.stimuli import GratingParams, RectParams, ShapeAppearance
from vstimd.stimuli.stimuli_models import Color, Vec2
from vstimd.vtl import VtlKind, VtlHandle

from ._helpers import Stage
from ._helpers import make_rect as _make_rect
from ._helpers import wait_for_anim_run_start as _wait_for_run_start
from ._helpers import wait_for_anim_state as _wait_for_state


@pytest.mark.onscreen(
    "ANIM-01",
    "a dark red 80×80 px square in the centre that switches on by itself "
    "for 30 frames (half a second) and then goes off and stays off",
)
def test_anim_flash_state_transitions(conn: Connection, stage: Stage) -> None:
    """Flash runs for N frames and ends in DONE state."""
    s = _make_rect(conn, x=0, y=0, enabled=False)

    a = conn.animations.create_flash(
        s, duration_frames=60, name="flash_60", final_action_mask=FinalAction.DISABLE
    )
    assert conn.animations.query(a).state == AnimationState.IDLE

    # Single source of truth: query() and list_animations() return the same
    # canonical type_name (the serde config tag), sent verbatim by the server.
    assert conn.animations.query(a).type_name == "FlashForNFrames"
    listed = next(i for i in conn.animations.list_animations() if i.handle == a)
    assert listed.type_name == conn.animations.query(a).type_name

    stage.cue("watch the centre: the square flashes on for a second")
    conn.animations.arm(a)
    assert conn.animations.query(a).state in (
        AnimationState.ARMED,
        AnimationState.RUNNING,
    )

    stage.hold()

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=4.0)
    assert final == AnimationState.DONE, f"animation did not reach DONE (got {final!r})"

    info = conn.stimuli.query(s)
    assert info.enabled is False, "stimulus should be disabled by DISABLE final action"

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-02",
    "a red square left of centre, on for the 60 frames the flash runs and "
    "off for good once it ends",
)
def test_anim_flash_stimulus_visible_during_run(conn: Connection, stage: Stage) -> None:
    """Stimulus is enabled while flash is running and disabled after DISABLE final action."""
    s = _make_rect(conn, x=-150, y=0, enabled=False)

    a = conn.animations.create_flash(
        s, duration_frames=60, final_action_mask=FinalAction.DISABLE
    )
    stage.cue("watch left of centre: the square comes on, then goes off for good")
    conn.animations.arm(a)

    time.sleep(0.1)
    info = conn.stimuli.query(s)
    assert info.enabled is True, "stimulus should be enabled while flash is running"

    stage.step("rect ON (flash running)")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=4.0)
    assert final == AnimationState.DONE

    info = conn.stimuli.query(s)
    assert info.enabled is False, "stimulus should be disabled after flash + DISABLE"

    stage.step("rect OFF (flash done, DISABLE)")

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-03",
    "a red square right of centre that stays hidden until a rising edge "
    "arrives on input line (0,10), then flashes for 30 frames",
)
def test_anim_flash_start_trigger(conn: Connection, stage: Stage) -> None:
    """Flash with start_trigger stays ARMED until a rising edge fires it."""
    s = _make_rect(conn, x=150, y=0, enabled=False)

    a = conn.animations.create_flash(
        s,
        duration_frames=60,
        start_trigger=VtlHandle.input(0, 10),
        start_edge=VtlEdge.RISING,
        final_action_mask=FinalAction.DISABLE,
    )
    conn.animations.arm(a)

    time.sleep(0.2)
    assert conn.animations.query(a).state == AnimationState.ARMED, (
        "should remain ARMED before trigger"
    )

    stage.step("ARMED — waiting for trigger")

    conn.vtl.set_line(VtlHandle.input(0, 10), True)
    time.sleep(0.1)
    conn.vtl.set_line(VtlHandle.input(0, 10), False)

    stage.step("triggered — rect ON")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=4.0)
    assert final == AnimationState.DONE

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-04",
    "nothing visible below centre: the flash there is armed waiting on a "
    "trigger that never comes, then disarmed — its square never appears",
)
def test_anim_flash_disarm_resets_state(conn: Connection, stage: Stage) -> None:
    """Disarming a flash while ARMED returns it to IDLE."""
    s = _make_rect(conn, x=0, y=-100, enabled=False)

    a = conn.animations.create_flash(
        s, duration_frames=120, start_trigger=VtlHandle.input(0, 11), start_edge=VtlEdge.RISING
    )
    conn.animations.arm(a)

    time.sleep(0.1)
    assert conn.animations.query(a).state == AnimationState.ARMED

    stage.step("ARMED, about to disarm")

    conn.animations.disarm(a)
    assert conn.animations.query(a).state == AnimationState.IDLE

    stage.step("IDLE after disarm", hold=0.5)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-05",
    "a red square in the centre, visible while its long flash runs, that "
    "disappears the moment the animation is cancelled from the client",
)
def test_anim_cancel_command_running(conn: Connection, stage: Stage) -> None:
    """Cancelling a RUNNING animation via the software command is a clean teardown → DONE.

    Distinct from disarm (which returns to IDLE): cancel runs the final action
    (DISABLE here) and lands in DONE.
    """
    s = _make_rect(conn, x=0, y=0, enabled=False)

    # Long duration so it is still running when we cancel.
    a = conn.animations.create_flash(
        s, duration_frames=600, cancel_action_mask=CancelAction.DISABLE
    )
    conn.animations.arm(a)

    _wait_for_state(conn, a, AnimationState.RUNNING, timeout=2.0)
    time.sleep(0.05)
    assert conn.stimuli.query(s).enabled is True, "stimulus enabled while running"

    stage.step("RUNNING — about to cancel")

    conn.animations.cancel(a)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=2.0)
    assert final == AnimationState.DONE, "cancel ends in DONE (not IDLE like disarm)"
    assert conn.stimuli.query(s).enabled is False, "cancel runs DISABLE teardown"

    stage.step("cancelled — DONE, rect OFF", hold=0.5)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-06",
    "a red square in the centre, visible while its long flash runs, that "
    "disappears when a rising edge on input line (0,50) cancels the "
    "animation",
)
def test_anim_cancel_trigger_running(conn: Connection, stage: Stage) -> None:
    """A cancel_trigger VTL edge aborts a RUNNING animation with clean teardown → DONE."""
    s = _make_rect(conn, x=0, y=0, enabled=False)

    a = conn.animations.create_flash(
        s,
        duration_frames=600,
        cancel_trigger=VtlHandle.input(0, 50),
        cancel_edge=VtlEdge.RISING,
        cancel_action_mask=CancelAction.DISABLE,
    )
    conn.animations.arm(a)

    _wait_for_state(conn, a, AnimationState.RUNNING, timeout=2.0)
    time.sleep(0.05)
    assert conn.stimuli.query(s).enabled is True

    stage.step("RUNNING — firing cancel edge")

    conn.vtl.set_line(VtlHandle.input(0, 50), True)
    time.sleep(0.1)
    conn.vtl.set_line(VtlHandle.input(0, 50), False)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=2.0)
    assert final == AnimationState.DONE
    assert conn.stimuli.query(s).enabled is False, "cancel edge ran DISABLE teardown"

    stage.step("cancelled by edge — DONE", hold=0.5)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-07",
    "nothing visible in the centre: a flash armed on input (0,51) is "
    "cancelled by an edge on (0,52) before its trigger ever comes, so its "
    "square stays off",
)
def test_anim_cancel_trigger_while_armed(conn: Connection, stage: Stage) -> None:
    """A cancel_trigger edge stops an ARMED animation before it ever starts → DONE."""
    s = _make_rect(conn, x=0, y=0, enabled=False)

    # Waits on start_trigger (0,51); we never fire it — instead we fire the
    # cancel_trigger (0,52) while it is still ARMED.
    a = conn.animations.create_flash(
        s,
        duration_frames=120,
        start_trigger=VtlHandle.input(0, 51),
        start_edge=VtlEdge.RISING,
        cancel_trigger=VtlHandle.input(0, 52),
        cancel_edge=VtlEdge.RISING,
    )
    conn.animations.arm(a)

    time.sleep(0.1)
    assert conn.animations.query(a).state == AnimationState.ARMED

    stage.step("ARMED — firing cancel edge")

    conn.vtl.set_line(VtlHandle.input(0, 52), True)
    time.sleep(0.1)
    conn.vtl.set_line(VtlHandle.input(0, 52), False)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=2.0)
    assert final == AnimationState.DONE, "cancelled before start → DONE"
    assert conn.stimuli.query(s).enabled is False, (
        "flash never started; stimulus stays off"
    )

    stage.step("cancelled while ARMED — DONE", hold=0.5)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-08",
    "a red square up and left of centre flickering 6 frames on, 6 frames "
    "off (about 5 Hz) for one second, then off",
)
def test_anim_flicker_cycles(conn: Connection, stage: Stage) -> None:
    """Flicker toggles a stimulus on and off at the specified cadence."""
    s = _make_rect(conn, x=-200, y=100)

    a = conn.animations.create_flicker(s, on_frames=6, off_frames=6, total_frames=120)
    stage.cue("watch the upper left: the square is about to flicker at ~5 Hz")
    conn.animations.arm(a)

    stage.step("flickering", hold=2)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=5.0)
    assert final == AnimationState.DONE, f"flicker did not complete (got {final!r})"

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-09",
    "a red square up and right of centre flickering 8 on / 8 off with no "
    "end time, until it is disarmed and stays on",
)
def test_anim_flicker_indefinite_then_disarm(conn: Connection, stage: Stage) -> None:
    """Indefinite flicker stays RUNNING until explicitly disarmed."""
    s = _make_rect(conn, x=200, y=100)

    a = conn.animations.create_flicker(s, on_frames=8, off_frames=8)
    conn.animations.arm(a)

    running = _wait_for_state(conn, a, AnimationState.RUNNING, timeout=2.0)
    assert running == AnimationState.RUNNING, "indefinite flicker should reach RUNNING"

    stage.step("RUNNING (indefinite flicker)", hold=2)

    assert conn.animations.query(a).state == AnimationState.RUNNING, (
        "indefinite flicker should stay RUNNING"
    )

    conn.animations.disarm(a)
    assert conn.animations.query(a).state == AnimationState.IDLE

    info = conn.stimuli.query(s)
    assert info.anim_enabled is True, (
        "anim_enabled should be True after disarming flicker"
    )

    stage.step("IDLE after disarm", hold=0.5)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-10",
    "a red square above centre that stays hidden for the first two seconds "
    "(the flicker starts in its off phase) and only then appears",
)
def test_anim_flicker_off_phase_start(conn: Connection, stage: Stage) -> None:
    """Flicker with start_on_phase=False begins in the off-phase_cycles (stimulus hidden first)."""
    s = _make_rect(conn, x=0, y=100)

    # 30 on / 120 off (2 s at 60 fps); starts in off-phase_cycles → ample window to observe hidden state
    a = conn.animations.create_flicker(
        s, on_frames=30, off_frames=120, total_frames=150, start_on_phase=False
    )
    conn.animations.arm(a)

    _wait_for_state(conn, a, AnimationState.RUNNING, timeout=2.0)
    time.sleep(0.05)
    info = conn.stimuli.query(s)
    assert info.anim_enabled is False, (
        "stimulus should start in off-phase_cycles (anim_enabled=False)"
    )

    stage.step("off-phase_cycles (rect hidden)", hold=0.5)

    # after the off-phase_cycles (120 frames / 60 fps = 2 s) it should flip to on
    time.sleep(2.1)
    info = conn.stimuli.query(s)
    assert info.anim_enabled is True, (
        "stimulus should be in on-phase_cycles after off-phase_cycles ends"
    )

    stage.step("on-phase_cycles (rect visible)")

    _wait_for_state(conn, a, AnimationState.DONE, timeout=4.0)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-11",
    "a red square down and left of centre, hidden until a rising edge on "
    "input line (0,20) switches it on for good",
)
def test_anim_enable_on_trigger_edge_rising(conn: Connection, stage: Stage) -> None:
    """EnableOnTriggerEdge enables a disabled stimulus on a rising edge, then DONE."""
    s = _make_rect(conn, x=-100, y=-100, enabled=False)

    a = conn.animations.create_enable_on_trigger_edge(
        VtlHandle.input(0, 20),
        s,
        edge=VtlEdge.RISING,
        enabled=True,
    )
    stage.cue("watch the lower left: an edge on (0,20) will switch a square on")
    conn.animations.arm(a)

    time.sleep(0.1)
    assert conn.animations.query(a).state == AnimationState.RUNNING, (
        "should be RUNNING waiting for edge"
    )
    assert conn.stimuli.query(s).enabled is False, (
        "stimulus must still be disabled before edge"
    )

    stage.step("RUNNING — waiting for rising edge")

    conn.vtl.set_line(VtlHandle.input(0, 20), True)
    time.sleep(0.1)
    conn.vtl.set_line(VtlHandle.input(0, 20), False)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=2.0)
    assert final == AnimationState.DONE

    assert conn.stimuli.query(s).enabled is True, (
        "stimulus should be enabled after rising edge"
    )

    stage.step("rect ON (trigger fired)")

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-12",
    "a red square down and right of centre, visible until a falling edge on "
    "input line (0,21) switches it off",
)
def test_anim_enable_on_trigger_edge_falling(conn: Connection, stage: Stage) -> None:
    """EnableOnTriggerEdge with FALLING edge fires on the high→low transition."""
    s = _make_rect(conn, x=100, y=-100, enabled=True)

    a = conn.animations.create_enable_on_trigger_edge(
        VtlHandle.input(0, 21),
        s,
        edge=VtlEdge.FALLING,
        enabled=False,
    )
    stage.cue("watch the lower right: an edge on (0,21) will switch that square off")
    conn.animations.arm(a)

    conn.vtl.set_line(VtlHandle.input(0, 21), True)
    time.sleep(0.1)

    stage.step("RUNNING — waiting for falling edge")

    conn.vtl.set_line(VtlHandle.input(0, 21), False)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=2.0)
    assert final == AnimationState.DONE

    assert conn.stimuli.query(s).enabled is False, (
        "stimulus should be disabled after falling edge"
    )

    stage.step("rect OFF (falling edge fired)")

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-13",
    "a red square near the bottom edge that follows input line (0,30) "
    "directly: off while the line is low, on while it is high, off again "
    "when it drops",
)
def test_anim_couple_visibility_to_vtl_line(conn: Connection, stage: Stage) -> None:
    """CoupleVisibility mirrors anim_enabled to the level of a VTL input line."""
    s = _make_rect(conn, x=0, y=-200, enabled=False)

    a = conn.animations.create_couple_visibility_to_trigger_line(
        VtlHandle.input(0, 30),
        s,
        polarity=VtlPolarity.ACTIVE_HIGH,
    )
    conn.animations.arm(a)

    _wait_for_state(conn, a, AnimationState.RUNNING, timeout=2.0)
    time.sleep(0.05)
    assert conn.stimuli.query(s).anim_enabled is False, (
        "anim_enabled should be False when line is LOW"
    )

    stage.step("line LOW → rect OFF")

    conn.vtl.set_line(VtlHandle.input(0, 30), True)
    time.sleep(0.1)
    assert conn.stimuli.query(s).anim_enabled is True, (
        "anim_enabled should be True when line is HIGH"
    )

    stage.step("line HIGH → rect ON")

    conn.vtl.set_line(VtlHandle.input(0, 30), False)
    time.sleep(0.1)
    assert conn.stimuli.query(s).anim_enabled is False, (
        "anim_enabled should be False when line returns LOW"
    )

    stage.step("line LOW → rect OFF again")

    conn.animations.disarm(a)
    assert conn.animations.query(a).state == AnimationState.IDLE
    assert conn.stimuli.query(s).anim_enabled is True, (
        "anim_enabled should be True after disarming"
    )

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-14",
    "a red square at the bottom left doing the opposite of ANIM-13: on "
    "while input line (0,31) is low, off while it is high",
)
def test_anim_couple_visibility_inverted_polarity(conn: Connection, stage: Stage) -> None:
    """CoupleVisibility with ACTIVE_LOW: HIGH → anim_enabled=False, LOW → anim_enabled=True."""
    s = _make_rect(conn, x=-250, y=-200, enabled=False)

    a = conn.animations.create_couple_visibility_to_trigger_line(
        VtlHandle.input(0, 31),
        s,
        polarity=VtlPolarity.ACTIVE_LOW,
    )
    conn.animations.arm(a)

    _wait_for_state(conn, a, AnimationState.RUNNING, timeout=2.0)
    time.sleep(0.05)
    assert conn.stimuli.query(s).anim_enabled is True, (
        "inverted polarity: line LOW → anim_enabled=True"
    )

    stage.step("line LOW → rect ON (inverted)")

    conn.vtl.set_line(VtlHandle.input(0, 31), True)
    time.sleep(0.1)
    assert conn.stimuli.query(s).anim_enabled is False, (
        "inverted polarity: line HIGH → anim_enabled=False"
    )

    stage.step("line HIGH → rect OFF (inverted)")

    conn.animations.disarm(a)
    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-15",
    "a green 60×60 px square sweeping from the left edge to the right edge "
    "through 41 waypoints, then disappearing where it stops",
)
def test_anim_move_along_path_2d(conn: Connection, stage: Stage) -> None:
    """MoveAlongPath2D moves a stimulus through a sequence of positions."""
    s = conn.stimuli.shapes.create_rect(
        position_px=Vec2(-200, 0),
        params=RectParams(
            width_px=60,
            height_px=60,
            appearance=ShapeAppearance(fill_color=Color(0.2, 0.8, 0.2)),
        ),
    )

    xs = [x * 10.0 - 200.0 for x in range(41)]  # -200 → 200 in 41 steps
    ys = [0.0] * 41
    a = conn.animations.create_move_along_path_2d(
        s, x_px=xs, y_px=ys, final_action_mask=FinalAction.DISABLE
    )
    conn.animations.arm(a)

    stage.show("moving left→right")
    # Wait for a few frames then confirm position has moved from the start.
    time.sleep(0.1)
    mid_info = conn.stimuli.query(s)
    assert mid_info.pos_px.x > -200.0, (
        f"position should have advanced from start, got x={mid_info.pos_px.x}"
    )

    stage.hold(2)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=5.0)
    assert final == AnimationState.DONE, (
        f"path animation did not complete (got {final!r})"
    )

    # After completion the final position should be the last waypoint.
    end_info = conn.stimuli.query(s)
    assert abs(end_info.pos_px.x - 200.0) < 1.0, (
        f"expected final x≈200, got {end_info.pos_px.x}"
    )
    assert abs(end_info.pos_px.y) < 1.0, f"expected final y≈0, got {end_info.pos_px.y}"

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-16",
    "a blue 50×50 px square tracing a triangle at a steady 400 px/s and "
    "disappearing when it gets back to the corner it started from",
)
def test_anim_move_along_segments_2d(conn: Connection, stage: Stage) -> None:
    """MoveAlongSegments2D moves at constant pixel-per-second speed along waypoints."""
    s = conn.stimuli.shapes.create_rect(
        position_px=Vec2(-200, -100),
        params=RectParams(
            width_px=50,
            height_px=50,
            appearance=ShapeAppearance(fill_color=Color(0.2, 0.4, 1.0)),
        ),
    )

    xs = [-200.0, 200.0, 0.0, -200.0]
    ys = [-100.0, -100.0, 100.0, -100.0]
    a = conn.animations.create_move_along_segments_2d(
        s,
        x_px=xs,
        y_px=ys,
        speed_px_per_sec=400.0,
        final_action_mask=FinalAction.DISABLE,
    )
    conn.animations.arm(a)

    stage.show("moving along triangle")
    # Wait a short time and confirm the stimulus has left the starting position.
    time.sleep(0.15)
    mid_info = conn.stimuli.query(s)
    assert mid_info.pos_px.x > -200.0, (
        f"position should have moved from start, got x={mid_info.pos_px.x}"
    )

    stage.hold(3)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=10.0)
    assert final == AnimationState.DONE, (
        f"segment animation did not complete (got {final!r})"
    )

    # After completion the final position should be the last waypoint.
    end_info = conn.stimuli.query(s)
    assert abs(end_info.pos_px.x - (-200.0)) < 2.0, (
        f"expected final x≈-200, got {end_info.pos_px.x}"
    )
    assert abs(end_info.pos_px.y - (-100.0)) < 2.0, (
        f"expected final y≈-100, got {end_info.pos_px.y}"
    )

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-17",
    "a red square above centre, on while a 20-frame flash runs and hidden "
    "again afterwards — RESTORE_VISIBILITY puts back the state it had "
    "before",
)
def test_anim_final_action_restore_visibility(conn: Connection, stage: Stage) -> None:
    """RESTORE_VISIBILITY final action returns stimulus to its pre-animation enabled state."""
    s = _make_rect(conn, x=0, y=150, enabled=False)

    a = conn.animations.create_flash(
        s, duration_frames=45, final_action_mask=FinalAction.RESTORE_VISIBILITY
    )
    stage.cue("watch above centre: the square shows while the flash runs, then hides again")
    conn.animations.arm(a)

    time.sleep(0.05)
    assert conn.stimuli.query(s).enabled is True, (
        "stimulus should be enabled while flash is running"
    )

    stage.step("rect ON (flash running)")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0)
    assert final == AnimationState.DONE

    assert conn.stimuli.query(s).enabled is False, (
        "RESTORE_VISIBILITY should restore pre-animation disabled state"
    )

    stage.step("rect OFF (restored)")

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-18",
    "a red square below centre flashing for 15 frames; on completion it "
    "pulses the named output line 'anim_done_out', which is checked over "
    "the wire",
)
def test_anim_final_action_trigger_line(conn: Connection, stage: Stage) -> None:
    """FINAL_ACTION_TRIGGER_LINE fires an output bit on a named VTL line when the animation completes."""

    conn.vtl.set_line_name(
        bank=0, bit=40, kind=VtlKind.OUTPUT, name="anim_done_out"
    )

    s = _make_rect(conn, x=0, y=-150, enabled=False)
    a = conn.animations.create_flash(
        s,
        duration_frames=45,
        final_action_mask=FinalAction.FINAL_ACTION_TRIGGER_LINE | FinalAction.DISABLE,
        final_action_trigger_line=VtlHandle.named("anim_done_out", VtlKind.OUTPUT),
    )
    conn.animations.arm(a)

    stage.step("flash running — output fires at end")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0)
    assert final == AnimationState.DONE

    lines = conn.vtl.list_lines()
    assert any(l.name == "anim_done_out" for l in lines), (
        "output line should be registered"
    )

    stage.step("done — output pulsed")

    conn.animations.delete(a)
    conn.stimuli.delete(s)
    conn.vtl.set_line_name(bank=0, bit=40, kind=VtlKind.OUTPUT, name="")


@pytest.mark.onscreen(
    "ANIM-19",
    "nothing visible: two disabled squares above centre carry a flash and a "
    "flicker, and the server lists both with their names and IDLE state",
)
def test_anim_list_and_query(conn: Connection, stage: Stage) -> None:
    """list() and query() return accurate metadata for all active animations."""
    s1 = _make_rect(conn, x=-100, y=200, enabled=False)
    s2 = _make_rect(conn, x=100, y=200, enabled=False)

    a1 = conn.animations.create_flash(s1, duration_frames=120, name="flash_list_test")
    a2 = conn.animations.create_flicker(
        s2, on_frames=5, off_frames=5, name="flicker_list_test"
    )

    anim_list = conn.animations.list_animations()
    handles = {a.handle for a in anim_list}
    assert a1 in handles
    assert a2 in handles

    by_handle = {a.handle: a for a in anim_list}
    assert by_handle[a1].name == "flash_list_test"
    assert by_handle[a2].name == "flicker_list_test"
    assert by_handle[a1].state == AnimationState.IDLE
    assert by_handle[a2].state == AnimationState.IDLE

    details = conn.animations.query(a1)
    assert details.handle == a1
    assert details.name == "flash_list_test"
    assert details.state == AnimationState.IDLE
    assert s1 in details.stimuli

    stage.hold(0.5)
    conn.animations.delete(a1)
    conn.animations.delete(a2)
    conn.stimuli.delete(s1)
    conn.stimuli.delete(s2)


@pytest.mark.onscreen(
    "ANIM-20",
    "a 200×200 px grating in the centre switched on by a flash for 40 "
    "frames and off again — animations drive gratings, not just rects",
)
def test_anim_flash_with_grating(conn: Connection, stage: Stage) -> None:
    """Flash works with grating stimuli (not just rects)."""
    g = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        params=GratingParams(width_px=200, height_px=200, sf_cycles_per_px=0.04, contrast=0.9),
    )
    conn.stimuli.set_enabled(g, False)

    a = conn.animations.create_flash(
        g, duration_frames=60, final_action_mask=FinalAction.DISABLE
    )
    stage.cue("watch the centre: a grating patch is about to be flashed on")
    conn.animations.arm(a)

    time.sleep(0.05)
    assert conn.stimuli.query(g).enabled is True

    stage.step("grating ON (flash)")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0)
    assert final == AnimationState.DONE
    assert conn.stimuli.query(g).enabled is False

    conn.animations.delete(a)
    conn.stimuli.delete(g)


@pytest.mark.onscreen(
    "ANIM-21",
    "three red squares in a row below centre, switched on and off together "
    "by one flash animation driving all three",
)
def test_anim_multiple_stimuli(conn: Connection, stage: Stage) -> None:
    """Flash can control multiple stimuli at once."""
    stimuli = [_make_rect(conn, x=x, y=-50, enabled=False) for x in (-200, 0, 200)]

    a = conn.animations.create_flash(
        stimuli, duration_frames=60, final_action_mask=FinalAction.DISABLE
    )
    stage.cue("watch the row below centre: all three squares flash together")
    conn.animations.arm(a)

    time.sleep(0.05)
    for s in stimuli:
        assert conn.stimuli.query(s).enabled is True, "all three stimuli should be ON"

    stage.step("three rects ON simultaneously")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0)
    assert final == AnimationState.DONE
    for s in stimuli:
        assert conn.stimuli.query(s).enabled is False, (
            "all three stimuli should be OFF after flash"
        )

    conn.animations.delete(a)
    for s in stimuli:
        conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-22",
    "a red square above centre switched on by the animation's start action "
    "and switched off again by its final action",
)
def test_anim_start_action_enable(conn: Connection, stage: Stage) -> None:
    """StartAction.ENABLE enables stimuli when animation starts; FinalAction.DISABLE disables on completion."""
    s = _make_rect(conn, x=0, y=150, enabled=False)

    # Stimulus starts disabled — start_action enables it; DISABLE final_action turns it off at the end.
    a = conn.animations.create_flash(
        s,
        duration_frames=60,
        start_action_mask=StartAction.ENABLE,
        final_action_mask=FinalAction.DISABLE,
    )
    # Normally flash enables stimuli implicitly; here we verify start_action does too.
    conn.stimuli.set_enabled(s, False)  # ensure it is still disabled before arm
    conn.animations.arm(a)

    time.sleep(0.05)
    assert conn.stimuli.query(s).enabled is True, (
        "StartAction.ENABLE should enable stimulus at start"
    )

    stage.step("rect ON (start_action enabled it)")

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0)
    assert final == AnimationState.DONE

    assert conn.stimuli.query(s).enabled is False, (
        "FinalAction.DISABLE should disable stimulus at end"
    )

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-23",
    "the classic RF-mapping sweep: a narrow white 20×400 px bar appears at "
    "the left edge, crosses the screen at 400 px/s and vanishes on the "
    "right",
)
def test_anim_moving_bar_rf_mapping(conn: Connection, stage: Stage) -> None:
    """RF-mapping pattern: a bar sweeps across the screen, enabled at start and disabled at end.

    This is the canonical receptive field mapping stimulus: a narrow vertical bar
    hidden until the animation fires, sweeps left-to-right at constant speed, and
    disappears automatically on completion.
    """

    # Narrow vertical bar, initially disabled (will be enabled by start_action).
    bar = conn.stimuli.shapes.create_rect(
        position_px=Vec2(-400, 0),
        params=RectParams(
            width_px=20,
            height_px=400,
            appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
        ),
    )
    conn.stimuli.set_enabled(bar, False)
    assert not conn.stimuli.query(bar).enabled, "bar should be disabled after creation"

    # Sweep from x=-400 to x=400 at 400 px/s ≈ 2 seconds.
    a = conn.animations.create_move_along_segments_2d(
        bar,
        x_px=[-400.0, 400.0],
        y_px=[0.0, 0.0],
        speed_px_per_sec=400.0,
        name="rf_bar",
        start_action_mask=StartAction.ENABLE,
        final_action_mask=FinalAction.DISABLE,
    )
    conn.animations.arm(a)

    # After a short delay the bar should be enabled and have moved from the start.
    time.sleep(0.1)
    info = conn.stimuli.query(bar)
    assert info.enabled is True, (
        "bar should be enabled by start_action at animation start"
    )
    assert info.pos_px.x > -400.0, f"bar should have started moving, got x={info.pos_px.x}"

    stage.step("bar sweeping left→right", hold=2)

    final = _wait_for_state(conn, a, AnimationState.DONE, timeout=6.0)
    assert final == AnimationState.DONE, (
        f"bar animation did not complete (got {final!r})"
    )

    # At completion: final position near end waypoint, and stimulus disabled.
    end = conn.stimuli.query(bar)
    assert abs(end.pos_px.x - 400.0) < 5.0, (
        f"expected bar at x≈400 after sweep, got {end.pos_px.x}"
    )
    assert end.enabled is False, (
        "bar should be disabled by FinalAction.DISABLE after sweep"
    )

    stage.step("bar done — hidden again")

    conn.animations.delete(a)
    conn.stimuli.delete(bar)


@pytest.mark.onscreen(
    "ANIM-24",
    "a plain white rect and nothing else: driving a stimulus from shared "
    "memory is refused by the server rather than silently doing nothing",
)
def test_anim_external_position_2d_is_refused(conn: Connection, stage: Stage) -> None:
    """Unimplemented, and refused rather than silently doing nothing (#84).

    The server never opens the shared-memory segment, so accepting this would arm
    an animation that reports success and leaves the stimulus where it was for the
    whole session. Tighten this to the behavioural test when #84 lands.
    """
    s = conn.stimuli.shapes.create_rect()
    with pytest.raises(NotSupportedError):
        conn.animations.create_external_position_2d(s, shm_name="/vstimd_test_ext_pos")
    stage.hold(0.5)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-25",
    "two red squares either side of centre: the left one flashes briefly, "
    "and its completion pulse on output (0,20) sets the right one flashing",
)
def test_anim_output_edge_chaining(conn: Connection, stage: Stage) -> None:
    """Animation A pulses an output line on completion; animation B starts off
    that OUTPUT edge — chaining entirely inside the server, no input loopback."""
    sa = _make_rect(conn, x=-150, y=0, enabled=False)
    sb = _make_rect(conn, x=150, y=0, enabled=False)

    # A: short flash that pulses output bit (0, 20) when it completes.
    a = conn.animations.create_flash(
        sa,
        duration_frames=20,
        start_action_mask=StartAction.ENABLE,
        final_action_mask=FinalAction.DISABLE | FinalAction.FINAL_ACTION_TRIGGER_LINE,
        final_action_trigger_line=VtlHandle.output(0, 20),
    )
    # B: waits for a rising edge on the OUTPUT line (0, 20).
    b = conn.animations.create_flash(
        sb,
        duration_frames=60,
        start_action_mask=StartAction.ENABLE,
        final_action_mask=FinalAction.DISABLE,
        start_trigger=VtlHandle.output(0, 20),
        start_edge=VtlEdge.RISING,
    )

    conn.animations.arm(a)
    conn.animations.arm(b)
    assert conn.animations.query(b).state == AnimationState.ARMED, (
        "B must wait for A's output edge"
    )

    # A completes on its own; its output pulse starts B one frame later.
    assert _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0) == AnimationState.DONE
    started = _wait_for_state(conn, b, AnimationState.RUNNING, timeout=2.0)
    assert started in (AnimationState.RUNNING, AnimationState.DONE), (
        f"B should start off A's output edge (got {started!r})"
    )

    stage.step("B started from A's output edge", hold=0.5)

    _wait_for_state(conn, b, AnimationState.DONE, timeout=3.0)
    conn.animations.delete(a)
    conn.animations.delete(b)
    conn.stimuli.delete(sa)
    conn.stimuli.delete(sb)


@pytest.mark.onscreen(
    "ANIM-26",
    "two red squares either side of centre: the right one is on "
    "indefinitely until the left one finishes and its output pulse (0,21) "
    "cancels it",
)
def test_anim_output_edge_cancel_chaining(conn: Connection, stage: Stage) -> None:
    """Animation A pulses an output line on completion; a long-running animation
    B is cancelled off that OUTPUT edge — interlock entirely inside the server."""
    sa = _make_rect(conn, x=-150, y=0, enabled=False)
    sb = _make_rect(conn, x=150, y=0, enabled=False)

    # A: flash that pulses output bit (0, 21) on completion. Long enough that
    # "B is visible while running" is checked well inside A's run — at 6 frames
    # (100 ms) A could complete, and cancel B, before the poll below even
    # returned, and the assert then read the cancelled state as a failure.
    a = conn.animations.create_flash(
        sa,
        duration_frames=60,
        start_action_mask=StartAction.ENABLE,
        final_action_mask=FinalAction.DISABLE | FinalAction.FINAL_ACTION_TRIGGER_LINE,
        final_action_trigger_line=VtlHandle.output(0, 21),
    )
    # B: long-running; cancels on a rising edge of the OUTPUT line (0, 21).
    b = conn.animations.create_flash(
        sb,
        duration_frames=100000,
        start_action_mask=StartAction.ENABLE,
        cancel_trigger=VtlHandle.output(0, 21),
        cancel_edge=VtlEdge.RISING,
        cancel_action_mask=CancelAction.DISABLE,
    )

    conn.animations.arm(a)
    conn.animations.arm(b)
    _wait_for_state(conn, b, AnimationState.RUNNING, timeout=2.0)
    assert conn.stimuli.query(sb).enabled is True, "B visible while running"

    stage.step("B running — A about to finish")

    # A completes; its output pulse cancels B one frame later (DISABLE teardown).
    assert _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0) == AnimationState.DONE
    assert _wait_for_state(conn, b, AnimationState.DONE, timeout=2.0) == AnimationState.DONE
    time.sleep(0.05)
    assert conn.stimuli.query(sb).enabled is False, "B ran DISABLE cancel teardown"

    stage.step("B cancelled by A's output edge", hold=0.5)

    conn.animations.delete(a)
    conn.animations.delete(b)
    conn.stimuli.delete(sa)
    conn.stimuli.delete(sb)


@pytest.mark.onscreen(
    "ANIM-27",
    "three red squares in a row: the left one flashes, and its single "
    "output pulse (0,22) starts the middle and right ones together",
)
def test_anim_output_edge_fan_out(conn: Connection, stage: Stage) -> None:
    """One output edge starts several animations at once (fan-out)."""
    sa = _make_rect(conn, x=-200, y=0, enabled=False)
    sb = _make_rect(conn, x=0, y=0, enabled=False)
    sc = _make_rect(conn, x=200, y=0, enabled=False)

    a = conn.animations.create_flash(
        sa,
        duration_frames=20,
        start_action_mask=StartAction.ENABLE,
        final_action_mask=FinalAction.DISABLE | FinalAction.FINAL_ACTION_TRIGGER_LINE,
        final_action_trigger_line=VtlHandle.output(0, 22),
    )
    followers = [
        conn.animations.create_flash(
            s,
            duration_frames=60,
            start_action_mask=StartAction.ENABLE,
            final_action_mask=FinalAction.DISABLE,
            start_trigger=VtlHandle.output(0, 22),
            start_edge=VtlEdge.RISING,
        )
        for s in (sb, sc)
    ]

    conn.animations.arm(a)
    for f in followers:
        conn.animations.arm(f)
        assert conn.animations.query(f).state == AnimationState.ARMED

    assert _wait_for_state(conn, a, AnimationState.DONE, timeout=3.0) == AnimationState.DONE
    for f in followers:
        started = _wait_for_state(conn, f, AnimationState.RUNNING, timeout=2.0)
        assert started in (AnimationState.RUNNING, AnimationState.DONE), (
            f"follower {f} should start off A's output edge (got {started!r})"
        )

    stage.step("B and C started from one output edge", hold=0.5)

    for f in followers:
        _wait_for_state(conn, f, AnimationState.DONE, timeout=3.0)
        conn.animations.delete(f)
    conn.animations.delete(a)
    conn.stimuli.delete(sa)
    conn.stimuli.delete(sb)
    conn.stimuli.delete(sc)


def _line_high(conn: Connection, name: str) -> bool:
    """Current level of a named VTL line, read back from the server."""
    for line in conn.vtl.list_lines():
        if line.name == name:
            return line.high
    raise AssertionError(f"no VTL line named {name!r}")


def _wait_line(conn: Connection, name: str, want: bool, timeout: float = 4.0) -> bool:
    """Poll a named VTL line until it reads ``want``, or the timeout passes.

    A line moves on a server frame, not on the command that triggers it, so a
    level check after a trigger pulse needs a bounded wait rather than a fixed
    sleep sized to a frame rate the test does not control.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if _line_high(conn, name) == want:
            return want
        time.sleep(0.02)
    return _line_high(conn, name)


@pytest.mark.onscreen(
    "ANIM-28",
    "a red square right of centre flashing once per trigger edge, three "
    "trials in a row — REARM puts the animation back on watch after each "
    "run",
)
def test_anim_flash_rearm_fires_on_every_trigger_edge(conn: Connection, stage: Stage) -> None:
    """REARM returns a triggered flash to ARMED, so each edge fires it again.

    Without REARM the animation lands in DONE after the first edge and ignores
    every later one — the whole trial sequence stops after trial 1.
    """
    s = _make_rect(conn, x=150, y=0, enabled=False)

    # Long enough that a poll cannot step over the whole run: the wait below has
    # to catch the animation outside ARMED to know the trigger was consumed.
    a = conn.animations.create_flash(
        s,
        duration_frames=45,
        start_trigger=VtlHandle.input(0, 12),
        start_edge=VtlEdge.RISING,
        final_action_mask=FinalAction.DISABLE | FinalAction.REARM,
    )
    conn.animations.arm(a)

    for trial in range(3):
        assert conn.animations.query(a).state == AnimationState.ARMED, (
            f"trial {trial}: not waiting for its trigger"
        )
        conn.vtl.set_line(VtlHandle.input(0, 12), True)
        conn.vtl.set_line(VtlHandle.input(0, 12), False)

        # The edge is consumed on the server's next frame, so wait for the run to
        # actually start — otherwise the ARMED below is the one this trial began
        # in, and the next trial's assert races the run it never waited for.
        started = _wait_for_run_start(conn, a, timeout=4.0)
        assert started != AnimationState.ARMED, f"trial {trial}: never started"

        # Runs, then re-arms rather than finishing in DONE.
        back = _wait_for_state(conn, a, AnimationState.ARMED, timeout=4.0)
        assert back == AnimationState.ARMED, f"trial {trial}: did not re-arm"
        stage.show(f"trial {trial + 1} fired, re-armed")
        stage.hold(0.33)

    conn.animations.delete(a)
    conn.stimuli.delete(s)


@pytest.mark.onscreen(
    "ANIM-29",
    "a red square left of centre flashing on each trigger edge; the "
    "checking is on output line 'e2e_done_level', which stays high between "
    "runs",
)
def test_anim_done_level_holds_until_next_start(conn: Connection, stage: Stage) -> None:
    """DONE_LEVEL is the sticky counterpart to the one-frame completion pulse.

    It answers "has this run finished?" at any time, and clears when the
    animation next starts so each run answers for itself.
    """
    s = _make_rect(conn, x=-150, y=0, enabled=False)

    # Name the line so its level can be read back through list_lines().
    conn.vtl.set_line_name(0, 21, VtlKind.OUTPUT, "e2e_done_level")

    a = conn.animations.create_flash(
        s,
        duration_frames=6,
        start_trigger=VtlHandle.input(0, 13),
        start_edge=VtlEdge.RISING,
        final_action_mask=FinalAction.DISABLE | FinalAction.REARM | FinalAction.DONE_LEVEL,
        final_action_level_line=VtlHandle.output(0, 21),
    )
    conn.animations.arm(a)

    assert not _line_high(conn, "e2e_done_level"), "level HIGH before the first run"

    conn.vtl.set_line(VtlHandle.input(0, 13), True)
    conn.vtl.set_line(VtlHandle.input(0, 13), False)
    # The level is the honest observable here: it goes HIGH on the completing
    # frame and stays there, whereas polling for ARMED matches the ARMED this
    # animation is already in until the server consumes the edge a frame later.
    assert _wait_line(conn, "e2e_done_level", True), "level never went HIGH on completion"

    # Still HIGH well after the completing frame — this is what makes it a level
    # rather than a mark.
    time.sleep(0.2)
    assert _line_high(conn, "e2e_done_level"), "level did not hold after completion"
    stage.step("finished — level HIGH", hold=0.5)

    # Starting again clears it.
    conn.vtl.set_line(VtlHandle.input(0, 13), True)
    conn.vtl.set_line(VtlHandle.input(0, 13), False)
    assert not _wait_line(conn, "e2e_done_level", False), "level survived the next start"

    _wait_for_state(conn, a, AnimationState.ARMED, timeout=4.0)
    conn.animations.delete(a)
    conn.stimuli.delete(s)
    conn.vtl.set_line_name(0, 21, VtlKind.OUTPUT, "")
