"""E2E tests for QueryStimulusRequest (query.proto).

These read back what the server thinks a stimulus is. Most of what they check
is invisible by design, so the captions say what little there is to see.
"""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import RectParams, ShapeAppearance
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "QUERY-01",
    "a small white 50×50 px square right of centre and above the middle "
    "(+120, −80 px), whose position is then read back",
)
def test_query_pos(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(
        position_px=Vec2(120, -80),
        params=RectParams(width_px=50, height_px=50),
    )
    info = conn.stimuli.query(handle)
    assert info.pos_px.x == pytest.approx(120.0, abs=0.5)
    assert info.pos_px.y == pytest.approx(-80.0, abs=0.5)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "QUERY-02",
    "a default white rect in the centre that disappears when it is disabled — "
    "query reports enabled=True, then False",
)
def test_query_enabled(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    info = conn.stimuli.query(handle)
    assert info.enabled is True
    stage.step("rect enabled — visible", hold=0.5)

    conn.stimuli.set_enabled(handle, False)
    info = conn.stimuli.query(handle)
    assert info.enabled is False

    stage.step("rect disabled — gone from the screen")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "QUERY-03",
    "a default white rect faded to 30 % opacity — a dim grey square, with "
    "query reporting opacity 0.3",
)
def test_query_opacity(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    conn.stimuli.set_alpha(handle, 0.3)
    info = conn.stimuli.query(handle)
    assert info.opacity == pytest.approx(0.3, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "QUERY-04",
    "a rect created red that turns azure (0, 0.5, 1) — query reports the new "
    "colour, not the one it was created with",
)
def test_query_fill_color(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(
        params=RectParams(appearance=ShapeAppearance(fill_color=Color(1.0, 0.0, 0.0))),
    )
    stage.step("created red", hold=0.5)

    conn.stimuli.set_fill_color(handle, Color(0.0, 0.5, 1.0))
    info = conn.stimuli.query(handle)
    assert info.fill_color.r == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.5, abs=0.01)
    assert info.fill_color.b == pytest.approx(1.0, abs=0.01)

    stage.step("now azure — this is what query reports")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "QUERY-05",
    "a default white rect turned 45° — a diamond standing on one corner, with "
    "query reporting rotation_deg 45",
)
def test_query_orientation(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    conn.stimuli.set_rotation(handle, 45.0)
    info = conn.stimuli.query(handle)
    assert info.rotation_deg == pytest.approx(45.0, abs=0.1)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "QUERY-06",
    "two overlapping default rects in the centre; the second is drawn on top, "
    "and query reports the higher draw order for it",
)
def test_query_draw_order(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect()
    h2 = conn.stimuli.shapes.create_rect()
    info1 = conn.stimuli.query(h1)
    info2 = conn.stimuli.query(h2)
    assert info2.draw_order > info1.draw_order

    stage.hold(0.5)
    conn.stimuli.delete(h1)
    conn.stimuli.delete(h2)


@pytest.mark.onscreen(
    "QUERY-07",
    "a default white rect queried twice: the id it reports is non-empty and "
    "does not change between calls. Nothing moves on screen",
)
def test_query_id_stable(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    id1 = conn.stimuli.query(handle).id
    id2 = conn.stimuli.query(handle).id
    assert id1 == id2
    assert len(id1) > 0

    stage.hold(0.5)
    conn.stimuli.delete(handle)
