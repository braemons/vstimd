"""E2E tests for rect stimuli (rect.proto: CreateRectRequest, SetRectSizeRequest)."""
from __future__ import annotations

import time

import pytest

from vstimd import Connection
from vstimd.stimuli import RectParams, ShapeAppearance, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2
from ._helpers import label as _label


def test_create_rect(conn: Connection, request: pytest.FixtureRequest) -> None:
    tid = request.node.name
    lbl = _label(conn, tid, "red 100×100 rect")
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

    time.sleep(1.0)
    conn.stimuli.delete(handle)
    conn.stimuli.delete(lbl)


def test_set_rect_size(conn: Connection) -> None:
    handle = conn.stimuli.shapes.create_rect(params=RectParams(width_px=100, height_px=50))
    conn.stimuli.shapes.set_rect_size(handle, 200, 80)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, RectParams)
    assert info.params.width_px == pytest.approx(200.0, abs=0.5)
    assert info.params.height_px == pytest.approx(80.0, abs=0.5)
    conn.stimuli.delete(handle)
