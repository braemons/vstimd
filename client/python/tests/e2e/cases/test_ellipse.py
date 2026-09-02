"""E2E tests for ellipse stimuli (ellipse.proto: CreateEllipseRequest, SetEllipseSizeRequest)."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import EllipseParams, ShapeAppearance, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "ELLI-01",
    "a pure green ellipse in the centre, 200 px wide and 80 px tall — "
    "clearly wider than it is high, axis-aligned",
)
def test_create_ellipse(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_ellipse(
        position_px=Vec2(0, 0),
        params=EllipseParams(
            width_px=200,
            height_px=80,
            appearance=ShapeAppearance(fill_color=Color(0.0, 1.0, 0.0)),
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.ELLIPSE
    assert isinstance(info.params, EllipseParams)
    assert info.params.width_px == pytest.approx(200.0, abs=0.5)
    assert info.params.height_px == pytest.approx(80.0, abs=0.5)
    assert info.fill_color.g == pytest.approx(1.0, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "ELLI-02",
    "a white 150×50 px ellipse in the centre, tilted 45° — long axis running "
    "from bottom-left up to top-right",
)
def test_create_ellipse_with_angle(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_ellipse(
        rotation_deg=45.0,
        params=EllipseParams(width_px=150, height_px=50),
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, EllipseParams)
    assert info.rotation_deg == pytest.approx(45.0, abs=0.1)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "ELLI-03",
    "a white 100×50 px ellipse that jumps to 300×120 px — three times as wide, "
    "still centred and axis-aligned",
)
def test_set_ellipse_size(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_ellipse(params=EllipseParams(width_px=100, height_px=50))
    stage.step("before: a 100×50 px ellipse", hold=0.5)

    conn.stimuli.shapes.set_ellipse_size(handle, 300, 120)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, EllipseParams)
    assert info.params.width_px == pytest.approx(300.0, abs=0.5)
    assert info.params.height_px == pytest.approx(120.0, abs=0.5)

    stage.step("after: the same ellipse resized to 300×120 px")
    conn.stimuli.delete(handle)
