"""E2E tests for polygon stimuli (polygon.proto: CreatePolygonRequest, SetPolygonVerticesRequest)."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.exceptions import NotSupportedError
from vstimd.stimuli import PolygonParams, ShapeAppearance, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2


@pytest.mark.xfail(raises=NotSupportedError, strict=True, reason="not yet implemented")
def test_create_polygon(conn: Connection) -> None:
    vertices_px = [Vec2(-50, -50), Vec2(50, -50), Vec2(0, 50)]
    handle = conn.stimuli.shapes.create_polygon(
        params=PolygonParams(
            vertices_px=vertices_px,
            close_shape=True,
            appearance=ShapeAppearance(fill_color=Color(1.0, 0.5, 0.0)),
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.POLYGON
    assert isinstance(info.params, PolygonParams)
    assert len(info.params.vertices_px) == 3
    assert info.params.close_shape is True
    assert info.params.vertices_px[0].x == pytest.approx(-50.0, abs=0.5)
    assert info.params.vertices_px[1].x == pytest.approx(50.0, abs=0.5)
    assert info.params.vertices_px[2].y == pytest.approx(50.0, abs=0.5)
    conn.stimuli.delete(handle)


@pytest.mark.xfail(raises=NotSupportedError, strict=True, reason="not yet implemented")
def test_create_polygon_open(conn: Connection) -> None:
    vertices_px = [Vec2(-100, 0), Vec2(0, 80), Vec2(100, 0)]
    handle = conn.stimuli.shapes.create_polygon(
        params=PolygonParams(vertices_px=vertices_px, close_shape=False),
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, PolygonParams)
    assert info.params.close_shape is False
    conn.stimuli.delete(handle)


@pytest.mark.xfail(raises=NotSupportedError, strict=True, reason="not yet implemented")
def test_set_polygon_vertices(conn: Connection) -> None:
    vertices_px = [Vec2(-50, -50), Vec2(50, -50), Vec2(0, 50)]
    handle = conn.stimuli.shapes.create_polygon(params=PolygonParams(vertices_px=vertices_px))

    new_vertices = [Vec2(-80, 0), Vec2(80, 0), Vec2(0, 80), Vec2(-40, -60)]
    conn.stimuli.shapes.set_polygon_vertices(handle, new_vertices)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, PolygonParams)
    assert len(info.params.vertices_px) == 4
    assert info.params.vertices_px[1].x == pytest.approx(80.0, abs=0.5)
    conn.stimuli.delete(handle)
