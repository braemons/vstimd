"""vstimd_client.shm.input against itself and against the Rust ``vinput`` crate.

The cross-language tests are the ones that keep two definitions of one layout
honest. They need the Rust ``segment_tool`` example, built on demand with cargo,
and are skipped where there is no cargo.
"""

from __future__ import annotations

import multiprocessing
import os
import pathlib
import shutil
import subprocess
import time

import pytest

from vstimd_client.shm import AxisSpec, InputDevice, InputDeviceReader, Semantic

_REPO = pathlib.Path(__file__).resolve().parents[4]
_TOOL = _REPO / "target" / "debug" / "examples" / "segment_tool"


def _name(tag: str) -> str:
    return f"/vstimd_pyshm_{os.getpid()}_{tag}"


@pytest.fixture(scope="module")
def segment_tool() -> pathlib.Path:
    if not _TOOL.exists():
        if shutil.which("cargo") is None:
            pytest.skip("cargo is not available to build the Rust segment_tool")
        subprocess.run(
            ["cargo", "build", "-q", "-p", "vinput", "--example", "segment_tool"],
            cwd=_REPO,
            check=True,
        )
    return _TOOL


def test_round_trip_in_python() -> None:
    with InputDevice.create(_name("rt"), [("x", Semantic.ABSOLUTE), AxisSpec("y", Semantic.RATE, 2.5, 0.25)]) as dev:
        dev.write([1.5, -3.0])
        r = InputDeviceReader(dev.name)
        assert r.read() == [1.5, -3.0]
        assert r.axes == [AxisSpec("x", Semantic.ABSOLUTE), AxisSpec("y", Semantic.RATE, 2.5, 0.25)]
        assert r.device_name == dev.name
        assert r.write_count == 1
        assert r.age_ns() < 1_000_000_000
        r.close()


def test_names_need_a_leading_slash() -> None:
    with pytest.raises(ValueError):
        InputDevice.create("no_slash", [("x", Semantic.ABSOLUTE)])


def test_close_removes_the_segment() -> None:
    dev = InputDevice.create(_name("gone"), [("x", Semantic.ABSOLUTE)])
    dev.close()
    with pytest.raises(FileNotFoundError):
        InputDeviceReader(dev.name)


def test_rust_reads_what_python_writes(segment_tool: pathlib.Path) -> None:
    with InputDevice.create(_name("py2rs"), [("dist", Semantic.CUMULATIVE, 0.5), ("speed", Semantic.RATE)]) as dev:
        dev.write([1234.5, -2.25])
        out = subprocess.run([segment_tool, "read", dev.name], capture_output=True, text=True, check=True).stdout
    name, axes, _, values, _, stats = out.split(maxsplit=5)
    assert name == dev.name
    assert axes == "dist:Cumulative:0.5,speed:Rate:1"
    assert values == "1234.5,-2.25"
    assert "count=1" in stats


def test_python_reads_what_rust_writes(segment_tool: pathlib.Path) -> None:
    name = _name("rs2py")
    proc = subprocess.Popen([segment_tool, "write", name, "3000", "7.5", "-1"], stdout=subprocess.PIPE, text=True)
    try:
        assert proc.stdout is not None and proc.stdout.readline().strip() == "ready"
        r = InputDeviceReader(name)
        assert r.read() == [7.5, -1.0]
        assert [a.semantic for a in r.axes] == [Semantic.RATE, Semantic.RATE]
        assert r.write_count == 1
        r.close()
    finally:
        proc.kill()
        proc.wait()


def _hammer(name: str, seconds: float) -> None:
    dev = InputDevice.create(name, [("a", Semantic.ABSOLUTE), ("b", Semantic.ABSOLUTE), ("c", Semantic.ABSOLUTE)])
    end = time.monotonic() + seconds
    n = 0.0
    while time.monotonic() < end:
        n += 1.0
        dev.write([n, n, n])
    dev.close()


def test_a_rust_reader_never_sees_a_mixed_python_write(segment_tool: pathlib.Path) -> None:
    name = _name("seqlock")
    writer = multiprocessing.get_context("spawn").Process(target=_hammer, args=(name, 2.0))
    writer.start()
    try:
        for _ in range(100):
            if pathlib.Path(f"/dev/shm/{name[1:]}").exists():
                break
            time.sleep(0.02)
        time.sleep(0.1)
        out = subprocess.run([segment_tool, "check", name, "1000"], capture_output=True, text=True)
        assert out.returncode == 0, out.stdout + out.stderr
        ok = int(out.stdout.split()[0].split("=")[1])
        assert ok > 1000, out.stdout
    finally:
        writer.join()
