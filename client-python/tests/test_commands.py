"""Unit tests: verify correct JSON is sent per operation."""

from __future__ import annotations

import pytest

from wonderlamp_client import visual
from wonderlamp_client._units import apply_operation


# ---------------------------------------------------------------------------
# Circle creation
# ---------------------------------------------------------------------------

class TestCircleCreate:
    def test_sends_create_circle(self, mock_win, mock_socket):
        visual.Circle(mock_win, radius=50, pos=(10, 20), fillColor="red")
        mock_win.flip()
        cmds = mock_socket.sent_commands()
        create = next(c for c in cmds if c.get("cmd") == "create_circle")
        assert create["radius"] == 50.0
        assert create["pos"] == [10.0, 20.0]
        assert create["fill_color"][0] == pytest.approx(1.0)
        assert create["fill_color"][1] == pytest.approx(0.0)
        assert create["fill_color"][2] == pytest.approx(0.0)

    def test_handle_is_integer(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_win.flip()
        create = mock_socket.last_cmd("create_circle")
        assert isinstance(create["handle"], int)
        assert create["handle"] == c._handle

    def test_enabled_false_by_default(self, mock_win, mock_socket):
        visual.Circle(mock_win)
        mock_win.flip()
        create = mock_socket.last_cmd("create_circle")
        assert create["enabled"] is False

    def test_enabled_true_when_autodraw(self, mock_win, mock_socket):
        visual.Circle(mock_win, autoDraw=True)
        mock_win.flip()
        create = mock_socket.last_cmd("create_circle")
        assert create["enabled"] is True

    def test_no_fill_color_sent_as_null(self, mock_win, mock_socket):
        visual.Circle(mock_win, fillColor=None)
        mock_win.flip()
        create = mock_socket.last_cmd("create_circle")
        assert create["fill_color"] is None


# ---------------------------------------------------------------------------
# Rect creation
# ---------------------------------------------------------------------------

class TestRectCreate:
    def test_sends_create_rect(self, mock_win, mock_socket):
        visual.Rect(mock_win, width=100, height=50)
        mock_win.flip()
        create = mock_socket.last_cmd("create_rect")
        assert create is not None
        assert create["width"] == pytest.approx(100.0)
        assert create["height"] == pytest.approx(50.0)


# ---------------------------------------------------------------------------
# Property setters
# ---------------------------------------------------------------------------

class TestSetPos:
    def test_set_pos_absolute(self, mock_win, mock_socket):
        c = visual.Circle(mock_win, pos=(0, 0))
        mock_socket.clear()
        c.pos = (100, 200)
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_pos")
        assert cmd["pos"] == [100.0, 200.0]

    def test_set_pos_via_setter_method(self, mock_win, mock_socket):
        c = visual.Circle(mock_win, pos=(10, 10))
        mock_socket.clear()
        c.setPos((5, 5), "+")
        assert c.pos == (15.0, 15.0)
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_pos")
        assert cmd["pos"] == [15.0, 15.0]

    def test_set_pos_subtract(self, mock_win, mock_socket):
        c = visual.Circle(mock_win, pos=(10, 10))
        mock_socket.clear()
        c.setPos((3, 4), "-")
        assert c.pos == (7.0, 6.0)


class TestSetOri:
    def test_set_ori_property(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_socket.clear()
        c.ori = 45.0
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_ori")
        assert cmd["ori"] == pytest.approx(45.0)

    def test_set_ori_add(self, mock_win, mock_socket):
        c = visual.Circle(mock_win, ori=10.0)
        mock_socket.clear()
        c.setOri(5.0, "+")
        assert c.ori == pytest.approx(15.0)


class TestSetColor:
    def test_fill_color_named(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_socket.clear()
        c.fillColor = "blue"
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_fill_color")
        assert cmd["fill_color"] == pytest.approx([0.0, 0.0, 1.0, 1.0])

    def test_line_color_hex(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_socket.clear()
        c.lineColor = "#ff8000"
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_line_color")
        r, g, b, a = cmd["line_color"]
        assert r == pytest.approx(1.0)
        assert g == pytest.approx(0x80 / 255.0, abs=0.01)
        assert b == pytest.approx(0.0)

    def test_opacity_setter(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_socket.clear()
        c.opacity = 0.5
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_opacity")
        assert cmd["opacity"] == pytest.approx(0.5)


# ---------------------------------------------------------------------------
# autoDraw / draw()
# ---------------------------------------------------------------------------

class TestAutoDraw:
    def test_autodraw_true_sends_set_enabled(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_socket.clear()
        c.autoDraw = True
        mock_win.flip()
        cmd = mock_socket.last_cmd("set_enabled")
        assert cmd["enabled"] is True

    def test_draw_one_shot_enables_then_disables(self, mock_win, mock_socket):
        c = visual.Circle(mock_win)
        mock_win.flip()
        mock_socket.clear()
        c.draw()
        mock_win.flip()
        cmds = mock_socket.sent_commands()
        enable_cmds = [x for x in cmds if x.get("cmd") == "set_enabled"]
        # Should have enabled=True (before flip) and enabled=False (after flip)
        assert any(x["enabled"] is True for x in enable_cmds)
        assert any(x["enabled"] is False for x in enable_cmds)


# ---------------------------------------------------------------------------
# Deferred mode: deferred_flip sent at end of batch
# ---------------------------------------------------------------------------

class TestDeferredMode:
    def test_deferred_flip_sent(self, mock_win, mock_socket):
        visual.Circle(mock_win)
        mock_win.flip()
        cmds = mock_socket.sent_commands()
        assert any(c.get("cmd") == "deferred_flip" for c in cmds)

    def test_no_deferred_flip_in_immediate_mode(self):
        from tests.conftest import _make_mock_window
        win = _make_mock_window(deferred=False)
        mock_socket = win._connection._socket
        c = visual.Circle(win)
        win.flip()  # no-op
        cmds = mock_socket.sent_commands()
        assert not any(c.get("cmd") == "deferred_flip" for c in cmds)


# ---------------------------------------------------------------------------
# apply_operation helper
# ---------------------------------------------------------------------------

class TestApplyOperation:
    def test_assign(self):
        assert apply_operation((1.0, 2.0), (5.0, 6.0), "") == (5.0, 6.0)
        assert apply_operation((1.0, 2.0), (5.0, 6.0), "=") == (5.0, 6.0)

    def test_add(self):
        assert apply_operation((1.0, 2.0), (3.0, 4.0), "+") == (4.0, 6.0)

    def test_subtract(self):
        assert apply_operation((10.0, 8.0), (3.0, 4.0), "-") == (7.0, 4.0)

    def test_multiply(self):
        assert apply_operation((2.0, 3.0), (4.0, 5.0), "*") == (8.0, 15.0)

    def test_divide(self):
        assert apply_operation((10.0, 20.0), (2.0, 4.0), "/") == (5.0, 5.0)
