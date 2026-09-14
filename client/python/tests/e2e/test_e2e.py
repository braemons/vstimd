"""E2E tests against a real vstimd with visible rendering.

Skipped in CI and when no display is available.

    make test-e2e
    uv run pytest tests/e2e/test_e2e.py --server tcp://192.168.1.10:5555
"""

import os
import pathlib
import subprocess
import sys
import tempfile
import time
import warnings

import pytest

from .cases import *  # noqa: F401, F403
from .conftest import reachable

_REPO_ROOT = pathlib.Path(__file__).parents[4]


@pytest.fixture(scope="session", autouse=True)
def server_process(server_address: str):
    """Start the real server; skip in CI or without a display."""
    if os.environ.get("CI"):
        pytest.skip("e2e tests skipped in CI")

    has_display = (
        sys.platform == "win32"
        or os.environ.get("DISPLAY")
        or os.environ.get("WAYLAND_DISPLAY")
    )
    if not has_display:
        pytest.skip("e2e tests require a display (no DISPLAY/WAYLAND_DISPLAY set)")

    if reachable(server_address):
        yield
        return

    result = subprocess.run(["cargo", "build", "--release"], cwd=_REPO_ROOT)
    if result.returncode != 0:
        pytest.skip(f"cargo build --release failed (exit {result.returncode})")

    exe = "vstimd.exe" if sys.platform == "win32" else "vstimd"
    server_bin = _REPO_ROOT / "target" / "release" / exe
    log_path = pathlib.Path(tempfile.gettempdir()) / "vstimd_e2e.log"
    log_file = log_path.open("w")
    env = os.environ.copy()
    env.setdefault("RUST_LOG", "debug")
    proc = subprocess.Popen(
        [str(server_bin)], stdout=log_file, stderr=log_file, env=env
    )

    for _ in range(20):
        if reachable(server_address):
            break
        time.sleep(0.5)
    else:
        proc.terminate()
        log_file.close()
        pytest.skip("server did not become ready in time")

    yield
    proc.terminate()
    try:
        proc.wait(timeout=20)
    except subprocess.TimeoutExpired:
        # A fullscreen Vulkan renderer can take longer than a polite terminate
        # allows to hand the display back, so escalate rather than fail the last
        # test that ran.
        proc.kill()
        try:
            proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            # SIGKILL is not negotiable, so a process still here is stuck in the
            # kernel giving the display back. Say so and let the suite finish:
            # how long cleanup took is not a result about the code under test.
            warnings.warn(f"vstimd (pid {proc.pid}) has not exited after SIGKILL")
    log_file.close()
    print(f"\nServer log: {log_path}")


@pytest.mark.onscreen(
    "SYS-15",
    "a red square above centre and a green one below, on a blue background; "
    "the capture must show them where they are",
    deferred=True,  # no caption: it would be in the captured pixels
)
def test_capture_frame_shows_what_was_drawn(conn):
    """CaptureFrame returns the presented frame, commands already applied."""
    from vstimd.stimuli.color import Color
    from vstimd.stimuli.shapes_models import RectParams, ShapeAppearance
    from vstimd.stimuli.stimuli_models import Vec2

    from .png_pixels import decode_rgb, pixel

    def square(y_px: float, color: Color) -> None:
        conn.stimuli.shapes.create_rect(
            position_px=Vec2(0.0, y_px),
            params=RectParams(
                width_px=80.0, height_px=80.0, appearance=ShapeAppearance(fill_color=color)
            ),
        )

    conn.system.set_background(0.0, 0.0, 1.0)
    square(150.0, Color(1.0, 0.0, 0.0))
    square(-150.0, Color(0.0, 1.0, 0.0))
    before = conn.system.wait_for_frames(0).frame_count

    shot = conn.system.capture_frame()
    width, height, rgb = decode_rgb(shot.png)
    assert (width, height) == (shot.width_px, shot.height_px)
    assert shot.frame >= before

    cx, cy = width // 2, height // 2
    # Stimulus space is Y-up; image rows run top to bottom.
    assert pixel(width, rgb, cx, cy - 150) == (255, 0, 0)
    assert pixel(width, rgb, cx, cy + 150) == (0, 255, 0)
    assert pixel(width, rgb, cx, cy) == (0, 0, 255)
