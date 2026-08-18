"""E2E tests for ellipse stimuli (ellipse.proto: CreateEllipseRequest, SetEllipseSizeRequest)."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import EllipseParams, ShapeAppearance, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2


def test_create_ellipse(conn: Connection) -> None:
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
    conn.stimuli.delete(handle)


def test_create_ellipse_with_angle(conn: Connection) -> None:
    handle = conn.stimuli.shapes.create_ellipse(
        rotation_deg=45.0,
        params=EllipseParams(width_px=150, height_px=50),
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, EllipseParams)
    assert info.rotation_deg == pytest.approx(45.0, abs=0.1)
    conn.stimuli.delete(handle)


def test_set_ellipse_size(conn: Connection) -> None:
    handle = conn.stimuli.shapes.create_ellipse(params=EllipseParams(width_px=100, height_px=50))
    conn.stimuli.shapes.set_ellipse_size(handle, 300, 120)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, EllipseParams)
    assert info.params.width_px == pytest.approx(300.0, abs=0.5)
    assert info.params.height_px == pytest.approx(120.0, abs=0.5)
    conn.stimuli.delete(handle)
