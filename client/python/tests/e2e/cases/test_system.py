"""E2E tests for scene-wide system commands (system.proto)."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.response import ErrorCode, ServerResponse
from vstimd.stimuli.stimuli_models import Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "SYS-01",
    "nothing new on screen: the server reports its resolution, frame rate and "
    "version, and all of them look sane",
)
def test_query_server_info(conn: Connection, stage: Stage) -> None:
    info = conn.system.query_server_info()
    assert info.width_px >= 0
    assert info.height_px >= 0
    assert info.frame_rate_hz > 0.0
    assert info.version.major >= 0
    stage.hold(0.3)


@pytest.mark.onscreen(
    "SYS-02",
    "the whole background turns steel blue (0.2, 0.4, 0.6) for a moment and "
    "then goes back to black",
)
def test_set_background(conn: Connection, stage: Stage) -> None:
    conn.system.set_background(r=0.2, g=0.4, b=0.6)
    info = conn.system.query_server_info()
    assert info.background_color.r == pytest.approx(0.2, abs=0.01)
    assert info.background_color.g == pytest.approx(0.4, abs=0.01)
    assert info.background_color.b == pytest.approx(0.6, abs=0.01)

    stage.step("background steel blue, no stimuli")
    conn.system.set_background(r=0.0, g=0.0, b=0.0)
    stage.step("background back to black", hold=0.5)


@pytest.mark.onscreen(
    "SYS-03",
    "a white rect and a white disc, both centred and overlapping; the server "
    "lists them with the names 'stim_a' and 'stim_b'",
)
def test_list_stimuli(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect(name="stim_a")
    h2 = conn.stimuli.shapes.create_circle(name="stim_b")

    entries = {e.handle: e for e in conn.system.list_stimuli()}

    assert h1 in entries and h2 in entries
    assert entries[h1].name == "stim_a"
    assert entries[h2].name == "stim_b"
    assert len(entries[h1].id) > 0

    stage.hold()
    conn.stimuli.delete(h1)
    conn.stimuli.delete(h2)


@pytest.mark.onscreen(
    "SYS-04",
    "a rect and a disc appear, then clear_stimuli wipes the screen completely "
    "— the caption goes with them, as it is a stimulus too",
)
def test_clear_stimuli(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect()
    h2 = conn.stimuli.shapes.create_circle()
    stage.step("rect and disc on screen, about to be cleared", hold=0.5)

    conn.system.clear_stimuli()
    handles = {e.handle for e in conn.system.list_stimuli()}
    assert h1 not in handles
    assert h2 not in handles

    stage.step("cleared — every stimulus is gone", hold=0.5)


@pytest.mark.onscreen(
    "SYS-05",
    "a rect with a flash animation attached; clear_stimuli empties the screen "
    "but the animation stays registered on the server",
)
def test_clear_stimuli_leaves_animations(conn: Connection, stage: Stage) -> None:
    """The three clear commands are separable: this one takes stimuli only."""
    h = conn.stimuli.shapes.create_rect()
    a = conn.animations.create_flash(h, duration_frames=60)
    conn.system.clear_stimuli()

    assert h not in {e.handle for e in conn.system.list_stimuli()}
    assert a in {e.handle for e in conn.animations.list_animations()}

    stage.step("stimuli cleared, the flash animation survives", hold=0.5)
    conn.system.clear_animations()


@pytest.mark.onscreen(
    "SYS-06",
    "a white rect that stays on screen while clear_animations removes the "
    "flash attached to it — the mirror image of SYS-05",
)
def test_clear_animations_leaves_stimuli(conn: Connection, stage: Stage) -> None:
    h = conn.stimuli.shapes.create_rect()
    a = conn.animations.create_flash(h, duration_frames=60)
    conn.system.clear_animations()

    assert a not in {e.handle for e in conn.animations.list_animations()}
    assert h in {e.handle for e in conn.system.list_stimuli()}

    stage.step("animations cleared, the rect is still there", hold=0.5)
    conn.system.clear_stimuli()


@pytest.mark.onscreen(
    "SYS-07",
    "a rect with a flash animation, both swept away by clear_all: the screen "
    "ends up blank and no animations remain",
)
def test_clear_all_takes_both(conn: Connection, stage: Stage) -> None:
    h = conn.stimuli.shapes.create_rect()
    a = conn.animations.create_flash(h, duration_frames=60)
    stage.step("rect and animation in place, about to clear_all", hold=0.5)

    conn.system.clear_all()
    assert h not in {e.handle for e in conn.system.list_stimuli()}
    assert a not in {e.handle for e in conn.animations.list_animations()}

    stage.step("clear_all done — nothing left in the scene", hold=0.5)


@pytest.mark.onscreen(
    "SYS-08",
    "a rect and a disc that blank out together when everything is disabled, "
    "then reappear together when everything is enabled again",
)
def test_set_all_enabled(conn: Connection, stage: Stage) -> None:
    h1 = conn.stimuli.shapes.create_rect()
    h2 = conn.stimuli.shapes.create_circle()
    stage.step("rect and disc visible", hold=0.5)

    conn.system.set_all_enabled(False)
    assert conn.stimuli.query(h1).enabled is False
    assert conn.stimuli.query(h2).enabled is False
    stage.hold(0.5)  # the caption is disabled too — the screen is blank

    conn.system.set_all_enabled(True)
    assert conn.stimuli.query(h1).enabled is True
    assert conn.stimuli.query(h2).enabled is True

    stage.step("all enabled again — both shapes and this caption are back", hold=0.5)
    conn.stimuli.delete(h1)
    conn.stimuli.delete(h2)


@pytest.mark.onscreen(
    "SYS-09",
    "nothing on screen: every mutation answers with an OK response carrying a "
    "frame count and a server timestamp, and the frame count keeps advancing",
)
def test_server_response_fields(conn: Connection, stage: Stage) -> None:
    """Every mutation returns a ServerResponse with sensible metadata."""
    resp = conn.system.clear_all()
    assert isinstance(resp, ServerResponse)
    assert resp.code == ErrorCode.OK
    assert resp.error == ""
    assert resp.frame_count >= 0
    assert resp.server_time_ns > 0

    # frame_count must advance across successive RPCs
    r1 = conn.system.wait_for_frames(1)
    r2 = conn.system.wait_for_frames(1)
    assert r2.frame_count > r1.frame_count
    stage.hold(0.3)


@pytest.mark.onscreen(
    "SYS-10",
    "nothing on screen: wait_until(timestamp) returns OK for a server time "
    "that has already passed",
)
def test_wait_until(conn: Connection, stage: Stage) -> None:
    r1 = conn.system.wait_for_frames(1)
    r2 = conn.system.wait_until(r1.server_time_ns)
    assert r2.code == ErrorCode.OK
    stage.hold(0.3)


@pytest.mark.onscreen(
    "SYS-11",
    "nothing on screen: a second connection to the already-running server "
    "becomes ready immediately",
)
def test_wait_until_ready_already_running(server_address: str, stage: Stage) -> None:
    """wait_until_ready returns immediately when the server is already up."""
    with Connection(server_address) as c:
        c.wait_until_ready(timeout_s=5.0)
    stage.hold(0.3)


@pytest.mark.onscreen(
    "SYS-12",
    "nothing on screen: Connection(wait_ready=True) is usable as soon as the "
    "constructor returns",
)
def test_wait_ready_constructor_flag(server_address: str, stage: Stage) -> None:
    """Connection(wait_ready=True) connects and becomes ready without extra calls."""
    with Connection(server_address, wait_ready=True, ready_timeout_s=5.0) as c:
        info = c.system.query_server_info()
        assert info.frame_rate_hz > 0.0
    stage.hold(0.3)


@pytest.mark.onscreen(
    "SYS-13",
    "nothing on screen: waiting on a port with no server behind it gives up "
    "with TimeoutError after a second",
)
def test_wait_until_ready_timeout(stage: Stage) -> None:
    """wait_until_ready raises TimeoutError when nothing is listening."""
    with Connection("tcp://localhost:19876") as c:
        with pytest.raises(TimeoutError):
            c.wait_until_ready(timeout_s=1.0, retry_interval_s=0.2)
    stage.hold(0.3)


@pytest.mark.onscreen(
    "SYS-14",
    "a white rect that only moves to (100, 50) px once deferred mode is turned "
    "off — the move is staged, then applied on the next frame",
)
def test_set_deferred_mode(conn: Connection, stage: Stage) -> None:
    h = conn.stimuli.shapes.create_rect(position_px=Vec2(0, 0))
    stage.step("rect centred, deferred mode about to be turned on", hold=0.5)

    # The caption is a stimulus like any other, so it has to be written before
    # deferred mode swallows the change along with the move under test.
    stage.show("move sent while deferred — the rect should not move yet")
    begun = conn.system.set_deferred_mode(True)
    assert begun.deferred and not begun.was_deferred
    conn.stimuli.set_position(h, Vec2(100, 50))
    stage.hold(0.5)

    ended = conn.system.set_deferred_mode(False)
    assert ended.was_deferred, "it was on, so ending it has something to do"
    assert ended.flip_scheduled
    # The flip lands on the frame after the call — or on the call's own frame,
    # if the render thread got there while the reply was being built.
    assert ended.flip_frame > 0
    assert ended.flip_frame <= ended.frame_count + 1
    assert begun.frame_count <= ended.frame_count
    # The reported frame is the first one drawn from the staged state, so
    # waiting for exactly it is enough — no sleeping on a guessed vsync.
    conn.system.wait_for_frame(ended.flip_frame)
    info = conn.stimuli.query(h)
    assert info.pos_px.x == pytest.approx(100.0, abs=0.5)

    stage.step("deferred mode off — the staged move has landed")

    # Ending it again has nothing to end, and says so rather than scheduling a
    # flip of stale copies over the live scene.
    again = conn.system.set_deferred_mode(False)
    assert again.was_a_no_op
    assert not again.flip_scheduled
    assert again.flip_frame == 0

    conn.stimuli.delete(h)
