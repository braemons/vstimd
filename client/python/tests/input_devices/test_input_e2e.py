"""Input devices end to end: a Python producer, a null server that declares the
device in its rig-config, and animations that follow it.

Starts its own server on free ports with its own rig-config, so it never shares
state with another suite.

    make test-e2e-null
"""

from __future__ import annotations

import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import time

import pytest

from vstimd import Connection, InvalidArgumentError
from vstimd.animations import AxisMap, AxisRef, TransformChannel
from vstimd.shm import InputDevice, Semantic
from vstimd.stimuli import Vec2

_REPO = pathlib.Path(__file__).resolve().parents[4]
SHM = f"/vstimd_e2e_input_{os.getpid()}"


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


@pytest.fixture(scope="module")
def conn():
    exe = _REPO / "target" / "release" / ("vstimd.exe" if sys.platform == "win32" else "vstimd")
    if not exe.exists():
        subprocess.run(["cargo", "build", "--release"], cwd=_REPO, check=True)
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="vstimd_input_e2e_"))
    rig = tmp / "rig.toml"
    rig.write_text(f"""
[[input.device]]
name = "gaze"
shm = "{SHM}"
stale_after_ms = 150
  [[input.device.axis]]
  name = "x"
  semantic = "absolute"
  [[input.device.axis]]
  name = "y"
  semantic = "absolute"
  [[input.device.axis]]
  name = "wheel"
  semantic = "cumulative"
  scale = 0.5
""")
    port = _free_port()
    log = (tmp / "server.log").open("w")
    proc = subprocess.Popen(
        [str(exe), "--null", "--no-web", "--no-events", "--zmq-port", str(port),
         "--rig-config", str(rig), "--storage-dir", str(tmp / "store")],
        stdout=log, stderr=log,
    )
    try:
        c = Connection(f"tcp://localhost:{port}")
        c.wait_until_ready(timeout_s=20)
        yield c
        c.close()
    finally:
        proc.terminate()
        proc.wait(timeout=10)
        log.close()


def _until(predicate, timeout_s: float = 3.0) -> bool:
    end = time.monotonic() + timeout_s
    while time.monotonic() < end:
        if predicate():
            return True
        time.sleep(0.02)
    return False


def test_a_python_producer_drives_a_stimulus_through_the_rig(conn: Connection) -> None:
    dev = InputDevice.create(SHM, [("x", Semantic.ABSOLUTE), ("y", Semantic.ABSOLUTE), ("wheel", Semantic.CUMULATIVE)])
    try:
        dev.write([0.0, 0.0, 0.0])
        assert _until(lambda: not conn.system.list_input_devices()[0].stale), "device never connected"
        info = conn.system.list_input_devices()[0]
        assert info.name == "gaze" and info.backend == f"shm {SHM}" and info.connected
        assert [a.semantic for a in info.axes] == ["absolute", "absolute", "cumulative"]

        follower = conn.stimuli.shapes.create_rect()
        conn.animations.arm(conn.animations.create_external_position_2d(
            follower, "gaze", x_offset_px=5.0))
        dev.write([100.0, -40.0, 0.0])
        assert _until(lambda: conn.stimuli.query(follower).pos_px == Vec2(105.0, -40.0))

        slider = conn.stimuli.shapes.create_rect()
        anim = conn.animations.create_device_driven_transform(
            slider, "gaze", [AxisMap("wheel", TransformChannel.POS_X, gain=2.0)])
        conn.animations.arm(anim)
        conn.system.wait_for_frames(3)
        dev.write([100.0, -40.0, 50.0])  # 25 after scale, 50 px after gain
        assert _until(lambda: conn.stimuli.query(slider).pos_px == Vec2(50.0, 0.0))

        # The producer stops: the device goes stale, and query says so.
        assert _until(lambda: conn.animations.query(anim).device_stale, timeout_s=2.0)
        assert conn.system.list_input_devices()[0].stale
    finally:
        dev.close()


def test_device_animations_are_validated_against_the_rig(conn: Connection) -> None:
    r = conn.stimuli.shapes.create_rect()
    with pytest.raises(InvalidArgumentError, match="no input device"):
        conn.animations.create_device_driven_transform(r, "nope", [AxisMap("x", TransformChannel.POS_X)])
    with pytest.raises(InvalidArgumentError, match="absolute"):
        conn.animations.create_linear_nav_3d(0.0, source=AxisRef("gaze", "x"))
    with pytest.raises(InvalidArgumentError, match="camera"):
        conn.animations.create_device_driven_transform(r, "gaze", [AxisMap("wheel", TransformChannel.FORWARD)])
    nav = conn.animations.create_linear_nav_3d(0.0, source=AxisRef("gaze", "wheel"), wrap_period_cm=100)
    assert conn.animations.query(nav).type_name == "LinearNav3D"
