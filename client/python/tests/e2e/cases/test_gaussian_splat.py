"""E2E tests for Gaussian splat scenes.

A prototype, kept in a case file of its own so it can be added, disabled or
removed without touching the other 3-D tests.
"""

from __future__ import annotations

import pathlib
import struct

import pytest

from vstimd_client import VstimdClient
from vstimd_client.exceptions import InvalidArgumentError, WrongStimulusTypeError
from vstimd_client.stimuli import (
    GaussianSplat3DParams,
    Material3D,
    Shading,
    StimulusType,
    Vec3,
)
from vstimd_client.system import Camera3D

from ._helpers import Stage

PHONG = Material3D(shading=Shading.PHONG)


def _write_splat_corridor(path: pathlib.Path) -> int:
    """A synthetic corridor of splats in the antimatter15 ``.splat`` layout.

    Two walls and a floor of soft blobs, 60 cm wide and 4 m long, in cm — so it
    needs no scale. Stripes of colour every 50 cm give the walk a motion cue.
    """
    records = bytearray()
    for i in range(160):
        z = -2.5 * i
        rgb = (230, 120, 40) if (i // 20) % 2 == 0 else (40, 140, 230)
        for x, y in [(-30.0, float(h)) for h in range(-10, 30, 5)] + \
                    [(30.0, float(h)) for h in range(-10, 30, 5)] + \
                    [(float(w), -12.0) for w in range(-25, 30, 10)]:
            records += struct.pack(
                "<3f3f4B4B", x, y, z, 2.0, 2.0, 2.0, *rgb, 230, 255, 128, 128, 128,
            )
    path.write_bytes(bytes(records))
    return len(records) // 32


@pytest.mark.onscreen(
    "3D-09",
    "a corridor of soft orange and blue blobs; the camera walks down it, the "
    "view fades to black, and the walk starts again from the beginning",
)
def test_gaussian_splat_on_a_finite_track(
    conn: VstimdClient, stage: Stage, tmp_path: pathlib.Path
) -> None:
    from vstimd_client.animations import AnimationState

    path = tmp_path / "corridor.splat"
    _write_splat_corridor(path)
    splats = conn.stimuli.gaussian_splat.create(str(path), name="corridor")
    info = conn.stimuli.query(splats)
    assert info.stimulus_type == StimulusType.GAUSSIAN_SPLAT_3D
    assert info.params == GaussianSplat3DParams(path=str(path))

    with pytest.raises(WrongStimulusTypeError):
        conn.stimuli.shapes3d.set_material(splats, PHONG)
    with pytest.raises(InvalidArgumentError, match="no such splat file"):
        conn.stimuli.gaussian_splat.create(str(tmp_path / "missing.splat"))
    with pytest.raises(InvalidArgumentError):
        conn.animations.create_linear_nav_3d(60.0, wrap_period_cm=100.0, track_length_cm=100.0)

    conn.system.set_camera(Camera3D(position_cm=Vec3(0, 5, 20)))
    nav = conn.animations.create_linear_nav_3d(
        120.0, track_length_cm=250.0, fade_frames=30, name="walk"
    )
    conn.animations.arm(nav)
    conn.system.wait_for_frames(10)
    stage.hold(5.0)
    assert conn.animations.query(nav).state == AnimationState.RUNNING
    assert conn.system.query_camera().position_cm.z <= 20.0

    conn.animations.cancel(nav)
    conn.animations.delete(nav)
    conn.stimuli.delete(splats)
