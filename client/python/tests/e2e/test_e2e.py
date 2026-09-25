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
from .cases._helpers import check_frame_stats

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

    exe = "vstimd_client.exe" if sys.platform == "win32" else "vstimd"
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
@check_frame_stats
def test_capture_frame_shows_what_was_drawn(conn, stage):
    """CaptureFrame returns the presented frame, commands already applied."""
    from vstimd_client.stimuli.color import Color
    from vstimd_client.stimuli.shapes_models import RectParams, ShapeAppearance
    from vstimd_client.stimuli.stimuli_models import Vec2

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
    # Captionless tests have nothing else to dwell on: hold the frame about to be
    # captured, so it is on screen long enough to judge by eye.
    stage.hold()

    shot = conn.system.capture_frame()
    width, height, rgb = decode_rgb(shot.png)
    assert (width, height) == (shot.width_px, shot.height_px)
    assert shot.frame >= before

    cx, cy = width // 2, height // 2
    # Stimulus space is Y-up; image rows run top to bottom.
    assert pixel(width, rgb, cx, cy - 150) == (255, 0, 0)
    assert pixel(width, rgb, cx, cy + 150) == (0, 255, 0)
    assert pixel(width, rgb, cx, cy) == (0, 0, 255)


@pytest.mark.onscreen(
    "3D-05",
    "a grey 2-D circle on the left and a grey unlit 3-D sphere in the middle, "
    "identical in brightness; then the sphere turns Phong-lit from the left",
    deferred=True,  # no caption: it would be in the captured pixels
)
@check_frame_stats
def test_unlit_3d_matches_2d_luminance_and_phong_lights_one_side(conn, stage):
    """An unlit sphere writes exactly the pixel values a 2-D circle of the same
    colour does; a Phong sphere lit from -X is bright on its left, dark on its right."""
    from vstimd_client.stimuli import (
        CircleParams, Color, Material3D, Shading, ShapeAppearance, Sphere3DParams,
        Transform3D, Vec2, Vec3,
    )
    from vstimd_client.system import Lighting3D

    from .png_pixels import decode_rgb, pixel

    grey = Color(0.6, 0.6, 0.6)
    conn.system.set_background(0.0, 0.0, 0.0)
    conn.stimuli.shapes.create_circle(
        position_px=Vec2(-400.0, 0.0),
        params=CircleParams(diameter_px=120.0, appearance=ShapeAppearance(fill_color=grey)),
    )
    ball = conn.stimuli.shapes3d.create_sphere(
        transform=Transform3D(position_cm=Vec3(0.0, 0.0, -60.0)),
        params=Sphere3DParams(diameter_cm=20.0, material=Material3D(albedo=grey)),
    )

    stage.hold()
    shot = conn.system.capture_frame()
    width, height, rgb = decode_rgb(shot.png)
    cx, cy = width // 2, height // 2
    circle = pixel(width, rgb, cx - 400, cy)
    sphere = pixel(width, rgb, cx, cy)
    assert circle == sphere, f"2-D {circle} vs unlit 3-D {sphere}"
    assert circle[0] in (152, 153), circle  # 0.6 of 255, however the driver rounds

    # Light travelling +X, so from the viewer's left.
    conn.system.set_lighting(Lighting3D(
        ambient_color=Vec3(0.0, 0.0, 0.0),
        sun_direction=Vec3(1.0, 0.0, 0.0),
        sun_color=Vec3(1.0, 1.0, 1.0),
    ))
    conn.stimuli.shapes3d.set_material(ball, Material3D(albedo=grey, shading=Shading.PHONG))
    stage.hold()
    shot = conn.system.capture_frame()
    width, height, rgb = decode_rgb(shot.png)
    # The sphere is ~20 cm at 60 cm with a 60° vertical FOV: about 29% of the
    # height across. Sample well inside its left and right edges.
    offset = int(height * 0.10)
    left = pixel(width, rgb, cx - offset, cy)
    right = pixel(width, rgb, cx + offset, cy)
    assert left[0] > 100, f"lit side too dark: {left}"
    assert right[0] < 20, f"unlit side should fall to the zero ambient: {right}"


@pytest.mark.onscreen(
    "3D-07",
    "a striped grey corridor running away from the viewer with a sphere in every "
    "period; the two captures, one period apart, must be identical",
    deferred=True,  # no caption: it would be in the captured pixels
)
@check_frame_stats
def test_corridor_has_no_seam_one_period_apart(conn, stage):
    """A camera one period further down the corridor sees the same frame, bit for
    bit — which is what lets LinearNav3D wrap the camera without a visible jump."""
    from vstimd_client.stimuli import (
        Corridor3DParams, Material3D, Repeat3D, Shading, Sphere3DParams, Transform3D, Vec3,
    )
    from vstimd_client.system import Camera3D

    period = 100.0
    conn.stimuli.shapes3d.create_corridor(params=Corridor3DParams(
        period_cm=period, periods_ahead=40, periods_behind=2, ceiling=True,
        material=Material3D(shading=Shading.PHONG),
    ))
    conn.stimuli.shapes3d.create_sphere(
        transform=Transform3D(position_cm=Vec3(-15.0, 8.0, -50.0)),
        params=Sphere3DParams(
            diameter_cm=8.0,
            material=Material3D(shading=Shading.PHONG),
            repeat=Repeat3D(period_cm=period, ahead=40, behind=2),
        ),
    )

    def capture_at(z_cm: float) -> bytes:
        conn.system.set_camera(Camera3D(position_cm=Vec3(0.0, 20.0, z_cm)))
        stage.hold()
        return conn.system.capture_frame().png

    near = capture_at(0.5)
    far = capture_at(0.5 + period)
    assert near == far, "the corridor must look the same one period further on"
    # And it is not trivially identical: moving half a period changes the frame.
    assert capture_at(0.5 + period / 2) != near
