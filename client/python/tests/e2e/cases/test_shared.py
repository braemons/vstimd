"""E2E tests for shared stimulus mutations (shared_set_requests.proto)."""
from __future__ import annotations

import pytest

from vstimd import Connection, NotSupportedError
from vstimd.response import ErrorCode, ServerResponse
from vstimd.stimuli import RectParams, ShapeAppearance, TextParams
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "SHARED-01",
    "a default white rect in the centre that vanishes when disabled and comes "
    "straight back when enabled again",
)
def test_set_enabled(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    resp = conn.stimuli.set_enabled(handle, False)
    assert isinstance(resp, ServerResponse)
    assert resp.code == ErrorCode.OK
    assert resp.error == ""
    assert resp.frame_count >= 0
    assert conn.stimuli.query(handle).enabled is False
    stage.step("disabled — the rect is gone", hold=0.5)

    conn.stimuli.set_enabled(handle, True)
    assert conn.stimuli.query(handle).enabled is True

    stage.step("enabled again — the rect is back")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-02",
    "a default white rect that is deleted: it leaves the screen and its handle "
    "stops answering queries",
)
def test_delete(conn: Connection, stage: Stage) -> None:
    from vstimd import HandleNotFoundError
    handle = conn.stimuli.shapes.create_rect()
    stage.step("rect on screen, about to be deleted", hold=0.5)

    conn.stimuli.delete(handle)
    stage.step("deleted — screen empty, the handle is unknown to the server", hold=0.5)
    with pytest.raises(HandleNotFoundError):
        conn.stimuli.query(handle)


@pytest.mark.onscreen(
    "SHARED-03",
    "a rect being renamed 'original' → 'renamed' → nameless. Names are "
    "bookkeeping, so the rect on screen never changes",
)
def test_set_name(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(name="original")
    assert conn.stimuli.query(handle).name == "original"
    conn.stimuli.set_name(handle, "renamed")
    assert conn.stimuli.query(handle).name == "renamed"
    conn.stimuli.set_name(handle, "")
    assert conn.stimuli.query(handle).name == ""

    stage.hold(0.5)
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-04",
    "a rect created with the name 'fix_cross' — an ordinary white rect on "
    "screen; the name and generated id are checked over the wire",
)
def test_create_with_name(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(name="fix_cross")
    info = conn.stimuli.query(handle)
    assert info.name == "fix_cross"
    assert len(info.id) > 0

    stage.hold(0.5)
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-05",
    "a rect that starts centred and jumps to the lower right (+200, −100 px) "
    "— one instant move, no animation",
)
def test_set_position(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(position_px=Vec2(0, 0))
    stage.step("rect at the centre", hold=0.5)

    conn.stimuli.set_position(handle, Vec2(200, -100))
    info = conn.stimuli.query(handle)
    assert info.pos_px.x == pytest.approx(200.0, abs=0.5)
    assert info.pos_px.y == pytest.approx(-100.0, abs=0.5)

    stage.step("jumped to (+200, −100) px — right of centre, below the middle")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-06",
    "a default white rect tilted 30° anticlockwise from upright",
)
def test_set_orientation(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    conn.stimuli.set_rotation(handle, 30.0)
    assert conn.stimuli.query(handle).rotation_deg == pytest.approx(30.0, abs=0.1)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-07",
    "a white rect that turns a muted blue (0.2, 0.4, 0.8) — same size and "
    "place, colour only",
)
def test_set_fill_color(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(
        params=RectParams(appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0))),
    )
    stage.step("white rect", hold=0.5)

    conn.stimuli.set_fill_color(handle, Color(0.2, 0.4, 0.8))
    info = conn.stimuli.query(handle)
    assert info.fill_color.r == pytest.approx(0.2, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.4, abs=0.01)
    assert info.fill_color.b == pytest.approx(0.8, abs=0.01)

    stage.step("the same rect, now muted blue")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-08",
    "a default white rect dimmed to 60 % opacity — a mid-grey square against "
    "the black background",
)
def test_set_alpha(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    conn.stimuli.set_alpha(handle, 0.6)
    assert conn.stimuli.query(handle).opacity == pytest.approx(0.6, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-09",
    "rect, circle, ellipse, grating and text in turn, each dimmed to 35 % "
    "opacity and then deleted — every stimulus type takes the same command",
)
def test_set_alpha_on_every_stimulus_type(conn: Connection, stage: Stage) -> None:
    """Opacity is shared state — set_alpha is not a shapes-only command."""
    stimuli = [
        ("rect", lambda: conn.stimuli.shapes.create_rect()),
        ("circle", lambda: conn.stimuli.shapes.create_circle()),
        ("ellipse", lambda: conn.stimuli.shapes.create_ellipse()),
        ("grating", lambda: conn.stimuli.grating.create_grating()),
        ("text", lambda: conn.stimuli.text.create_text(params=TextParams(text="opacity"))),
    ]
    for name, create in stimuli:
        handle = create()
        conn.stimuli.set_alpha(handle, 0.35)
        assert conn.stimuli.query(handle).opacity == pytest.approx(0.35, abs=0.01)
        stage.step(f"{name} at 35 % opacity", hold=0.5)
        conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-10",
    "a rect with a half-transparent red fill under an opaque blue outline, all "
    "of it dimmed to 50 %. Draw mode is FILLED, so a dim red square is what "
    "shows; the point is that the two alphas are multiplied, not overwritten",
)
def test_set_alpha_leaves_fill_alpha_alone(conn: Connection, stage: Stage) -> None:
    """A half-transparent fill under an opaque outline keeps that relationship:
    the shared opacity multiplies both rather than overwriting either."""
    handle = conn.stimuli.shapes.create_rect()
    conn.stimuli.set_fill_color(handle, Color(1.0, 0.0, 0.0, 0.5))
    conn.stimuli.shapes.set_outline_color(handle, Color(0.0, 0.0, 1.0, 1.0))
    conn.stimuli.set_alpha(handle, 0.5)

    info = conn.stimuli.query(handle)
    assert info.fill_color.a == pytest.approx(0.5, abs=0.01)
    assert info.outline_color.a == pytest.approx(1.0, abs=0.01)
    assert info.opacity == pytest.approx(0.5, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-11",
    "a rect asked for opacity 5.0, which clamps to fully opaque, then for "
    "−2.0, which clamps to fully transparent — the rect disappears",
)
def test_set_alpha_clamps(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect()
    conn.stimuli.set_alpha(handle, 5.0)
    assert conn.stimuli.query(handle).opacity == pytest.approx(1.0, abs=0.01)
    stage.step("asked for 5.0 → clamped to 1.0, fully opaque", hold=0.5)

    conn.stimuli.set_alpha(handle, -2.0)
    assert conn.stimuli.query(handle).opacity == pytest.approx(0.0, abs=0.01)

    stage.step("asked for −2.0 → clamped to 0.0, invisible")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHARED-12",
    "nothing to see: bring_to_front is refused by the server (xfail). Two "
    "overlapping white rects would swap which one is on top",
)
@pytest.mark.xfail(raises=NotSupportedError, strict=True, reason="not yet implemented")
def test_bring_to_front(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect()
    h2 = conn.stimuli.shapes.create_rect()
    conn.stimuli.bring_to_front(h1)
    assert conn.stimuli.query(h1).draw_order > conn.stimuli.query(h2).draw_order
    stage.hold(0.5)
    conn.stimuli.delete(h1)
    conn.stimuli.delete(h2)


@pytest.mark.onscreen(
    "SHARED-13",
    "nothing to see: send_to_back is refused by the server (xfail). The second "
    "of two overlapping rects would drop behind the first",
)
@pytest.mark.xfail(raises=NotSupportedError, strict=True, reason="not yet implemented")
def test_send_to_back(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect()
    h2 = conn.stimuli.shapes.create_rect()
    conn.stimuli.send_to_back(h2)
    assert conn.stimuli.query(h2).draw_order < conn.stimuli.query(h1).draw_order
    stage.hold(0.5)
    conn.stimuli.delete(h1)
    conn.stimuli.delete(h2)


@pytest.mark.onscreen(
    "SHARED-14",
    "nothing to see: swap_draw_order is refused by the server (xfail). Two "
    "rects would exchange their places in the draw order",
)
@pytest.mark.xfail(raises=NotSupportedError, strict=True, reason="not yet implemented")
def test_swap_draw_order(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect()
    h2 = conn.stimuli.shapes.create_rect()
    order1_before = conn.stimuli.query(h1).draw_order
    order2_before = conn.stimuli.query(h2).draw_order
    conn.stimuli.swap_draw_order(h1, h2)
    assert conn.stimuli.query(h1).draw_order == order2_before
    assert conn.stimuli.query(h2).draw_order == order1_before
    stage.hold(0.5)
    conn.stimuli.delete(h1)
    conn.stimuli.delete(h2)
