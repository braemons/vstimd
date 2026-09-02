"""E2E tests for rect stimuli (rect.proto: CreateRectRequest, SetRectSizeRequest)."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import RectParams, ShapeAppearance, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "RECT-01",
    "a pure red square, 100×100 px, in the exact centre of the screen",
)
def test_create_rect(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(
        position_px=Vec2(0, 0),
        params=RectParams(
            width_px=100,
            height_px=100,
            appearance=ShapeAppearance(fill_color=Color(1.0, 0.0, 0.0)),
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.RECT
    assert isinstance(info.params, RectParams)
    assert info.params.width_px == pytest.approx(100.0, abs=0.5)
    assert info.params.height_px == pytest.approx(100.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(1.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(0.0, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "RECT-02",
    "a white 100×50 px rect in the centre, which then grows to 200×80 px "
    "— wider and a little taller, still centred",
)
def test_set_rect_size(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(params=RectParams(width_px=100, height_px=50))
    stage.step("before: a 100×50 px rect", hold=0.5)

    conn.stimuli.shapes.set_rect_size(handle, 200, 80)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, RectParams)
    assert info.params.width_px == pytest.approx(200.0, abs=0.5)
    assert info.params.height_px == pytest.approx(80.0, abs=0.5)

    stage.step("after: the same rect resized to 200×80 px")
    conn.stimuli.delete(handle)
