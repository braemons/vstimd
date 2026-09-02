"""E2E tests for conditions (conditions.proto: SetCondition, DeclareConditions,
ListConditions, SetStimulusConditions, SetAnimationConditions).

A condition selects which stimuli and animations are active; switching is a hard
cut with no cross-fade. The on-screen expectation for each test is therefore a
statement about what appears and disappears *at once*.
"""
from __future__ import annotations

import pytest

from vstimd import Connection, InvalidArgumentError
from vstimd.animations import AnimationState
from vstimd.conditions import ConditionAction
from vstimd.stimuli import RectParams, ShapeAppearance
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


def _rect(conn: Connection, x: float, color: Color):
    return conn.stimuli.shapes.create_rect(
        position_px=Vec2(x, 0),
        params=RectParams(
            width_px=160,
            height_px=160,
            appearance=ShapeAppearance(fill_color=color),
        ),
    )


@pytest.mark.onscreen(
    "COND-01",
    "a red square on the left and a blue one on the right, then the blue one "
    "vanishes as condition 1 becomes active, then comes back at condition 0",
)
def test_membership_gates_visibility(conn: Connection, stage: Stage) -> None:
    always = _rect(conn, -250, Color(1.0, 0.0, 0.0))
    baseline_only = _rect(conn, 250, Color(0.0, 0.4, 1.0))
    conn.conditions.set_stimulus_conditions(baseline_only, [0])
    stage.step("both squares: condition 0, blue belongs to it")

    conn.conditions.set(1)
    assert conn.stimuli.query(baseline_only).condition_enabled is False
    assert conn.stimuli.query(always).condition_enabled is True
    stage.step("condition 1: only the red square is left")

    conn.conditions.set(0)
    assert conn.stimuli.query(baseline_only).condition_enabled is True
    stage.step("back to condition 0: the blue square returns")

    conn.stimuli.delete(always)
    conn.stimuli.delete(baseline_only)
    conn.conditions.set(0)


@pytest.mark.onscreen(
    "COND-02",
    "nothing on screen: a condition switch hides a stimulus and restores it "
    "without ever touching the enabled flag the operator set",
)
def test_the_gate_does_not_touch_enabled(conn: Connection, stage: Stage) -> None:
    handle = _rect(conn, 0, Color(1.0, 1.0, 1.0))
    conn.conditions.set_stimulus_conditions(handle, [0])
    conn.stimuli.set_enabled(handle, False)

    conn.conditions.set(1)
    conn.conditions.set(0)

    info = conn.stimuli.query(handle)
    assert info.enabled is False, "the condition switch re-enabled it"
    assert info.condition_enabled is True
    stage.hold(0.3)

    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "COND-03",
    "nothing on screen: conditions are declared with names, switched to by "
    "name, and listed back with the active one marked",
)
def test_declare_and_switch_by_name(conn: Connection, stage: Stage) -> None:
    conn.conditions.declare([(0, "baseline"), (2, "probe")])

    conn.conditions.set("probe")
    status = conn.conditions.list_conditions()
    assert status.active_index == 2
    assert status.active_name == "probe"
    assert [c.index for c in status.declared] == [0, 2]

    with pytest.raises(InvalidArgumentError):
        conn.conditions.set("no_such_condition")
    assert conn.conditions.active == 2, "a typo must not move the protocol"

    # Any index is a valid condition; declaring is only what gives it a name.
    conn.conditions.set(7)
    assert conn.conditions.list_conditions().active_name == ""

    stage.hold(0.3)
    conn.conditions.declare([])
    conn.conditions.set(0)


@pytest.mark.onscreen(
    "COND-04",
    "nothing on screen: an animation is idled when its condition goes away "
    "and re-armed when it comes back",
)
def test_animation_reset_on_condition_switch(conn: Connection, stage: Stage) -> None:
    handle = _rect(conn, 0, Color(1.0, 1.0, 1.0))
    anim = conn.animations.create_flash(handle, duration_ms=200)
    conn.animations.arm(anim)

    conn.conditions.set_animation_conditions(anim, [1], action=ConditionAction.RESET)
    # Condition 0 is active and does not include it, so it was idled on the spot.
    assert conn.animations.query(anim).state == AnimationState.IDLE

    conn.conditions.set(1)
    assert conn.animations.query(anim).state == AnimationState.ARMED

    conn.conditions.set(0)
    assert conn.animations.query(anim).state == AnimationState.IDLE

    stage.hold(0.3)
    conn.animations.delete(anim)
    conn.stimuli.delete(handle)
    conn.conditions.set(0)


@pytest.mark.onscreen(
    "COND-05",
    "nothing on screen: HOLD leaves an animation armed across a condition "
    "switch, where the default RESET would have idled it",
)
def test_animation_hold_across_a_switch(conn: Connection, stage: Stage) -> None:
    handle = _rect(conn, 0, Color(1.0, 1.0, 1.0))
    anim = conn.animations.create_flash(handle, duration_ms=200)
    conn.animations.arm(anim)

    conn.conditions.set_animation_conditions(anim, [1], action=ConditionAction.HOLD)
    assert conn.animations.query(anim).state == AnimationState.ARMED

    conn.conditions.set(1)
    assert conn.animations.query(anim).state == AnimationState.ARMED

    stage.hold(0.3)
    conn.animations.delete(anim)
    conn.stimuli.delete(handle)
    conn.conditions.set(0)


@pytest.mark.onscreen(
    "COND-06",
    "nothing on screen: the declarations, the memberships and the active "
    "condition all survive a save and reload of the scene-config",
)
def test_conditions_survive_a_scene_config_round_trip(
    conn: Connection, stage: Stage
) -> None:
    conn.system.clear_all()
    handle = _rect(conn, 0, Color(1.0, 1.0, 1.0))
    conn.conditions.declare([(0, "baseline"), (1, "probe")])
    conn.conditions.set_stimulus_conditions(handle, [1])
    conn.conditions.set("probe")

    raw = conn.scene_config.retrieve()
    conn.system.clear_all()
    conn.conditions.declare([])
    conn.conditions.set(0)
    conn.scene_config.upload("e2e_test_conditions", raw, overwrite=True, apply_now=True)

    status = conn.conditions.list_conditions()
    assert status.active_index == 1
    assert status.active_name == "probe"
    restored = conn.system.list_stimuli()
    assert [e.condition_indices for e in restored] == [[1]]

    stage.hold(0.3)
    conn.system.clear_all()
    conn.conditions.declare([])
    conn.conditions.set(0)
    stage.show()
