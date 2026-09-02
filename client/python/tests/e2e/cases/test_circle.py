"""E2E tests for circle stimuli (circle.proto: CreateCircleRequest, SetCircleRadiusRequest)."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import CircleParams, ShapeAppearance, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "CIRC-01",
    "a pure blue disc, 120 px across, in the exact centre of the screen",
)
def test_create_circle(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_circle(
        position_px=Vec2(0, 0),
        params=CircleParams(
            diameter_px=120,
            appearance=ShapeAppearance(fill_color=Color(0.0, 0.0, 1.0)),
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.CIRCLE
    assert isinstance(info.params, CircleParams)
    assert info.params.diameter_px == pytest.approx(120.0, abs=0.5)
    assert info.fill_color.b == pytest.approx(1.0, abs=0.01)
    assert info.fill_color.r == pytest.approx(0.0, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "CIRC-02",
    "a white disc 80 px across, which then grows to 180 px — same centre, "
    "more than twice the diameter",
)
def test_set_circle_diameter(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_circle(params=CircleParams(diameter_px=80))
    stage.step("before: an 80 px disc", hold=0.5)

    conn.stimuli.shapes.set_circle_diameter(handle, 180)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, CircleParams)
    assert info.params.diameter_px == pytest.approx(180.0, abs=0.5)

    stage.step("after: the same disc grown to 180 px across")
    conn.stimuli.delete(handle)
