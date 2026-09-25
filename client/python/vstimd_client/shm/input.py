"""Publish an input device for vstimd: the producer side of the ``vinput`` segment.

A reader process — your serial wheel reader, treadmill DAQ, eye tracker bridge —
creates the segment and writes samples. vstimd maps a rig-config device onto it
and reads it every frame::

    from vstimd_client.shm import InputDevice, Semantic

    dev = InputDevice.create("/vstimd_wheel", [("distance", Semantic.CUMULATIVE, 1.0)])
    total = 0
    for ticks in read_serial_wheel():
        total += ticks          # a running total: never reset, never a delta
        dev.write([total])

**Publish a cumulative axis as a running total, never as per-write deltas.**
vstimd differences successive totals, so a sample it misses is folded into the
next frame's change instead of being lost, however the write and frame rates
interleave.

**Write at the device's own rate, and keep writing.** Every write refreshes a
heartbeat; a producer silent for longer than the rig-config's ``stale_after_ms``
is treated as gone and the stimulus stops. So a stationary wheel still writes
its unchanged total.

The layout is byte-for-byte the Rust ``vinput`` crate's (``vinput/src/layout.rs``),
including the seqlock that keeps a multi-axis sample coherent; the tests check it
against the Rust implementation in both directions. One writer per segment.
"""

from __future__ import annotations

import importlib
import mmap
import os
import struct
import time
from collections.abc import Sequence
from dataclasses import dataclass
from enum import IntEnum
from typing import Any


def _posix() -> Any:
    """CPython's POSIX shared-memory primitives (``shm_open``, ``shm_unlink``).

    Used directly rather than through ``multiprocessing.shared_memory``, which
    fixes the mode at 0600 — unreadable to vstimd running as its own user — and
    registers the segment with a resource tracker that removes it on exit.
    """
    try:
        return importlib.import_module("_posixshmem")
    except ImportError as e:  # pragma: no cover - non-POSIX
        raise OSError("vstimd_client.shm needs POSIX shared memory (Linux)") from e

MAGIC = 0x5649_4E31  # "VIN1"
VERSION = 1
MAX_AXES = 16
NAME_LEN = 64
AXIS_NAME_LEN = 48
AXES_OFFSET = 128
AXIS_RECORD = 64
STATE_OFFSET = 0x1000
SHM_SIZE = 0x2000
MAX_READ_SPINS = 1024

_SEQ = STATE_OFFSET
_HEARTBEAT = STATE_OFFSET + 8
_WRITE_COUNT = STATE_OFFSET + 16
_VALUES = STATE_OFFSET + 24


class Semantic(IntEnum):
    """What an axis value means — and so how vstimd uses it."""

    #: The current value (a gaze position): used as is.
    ABSOLUTE = 0
    #: A running total (wheel ticks, distance): vstimd uses its change.
    CUMULATIVE = 1
    #: A velocity (a joystick): vstimd integrates it over frame time.
    RATE = 2


@dataclass(frozen=True)
class AxisSpec:
    name: str
    semantic: Semantic
    #: Declared for readers; vstimd applies the rig-config's scale, not this one.
    scale: float = 1.0
    deadzone: float = 0.0


class TornRead(Exception):
    """No coherent sample could be read: the producer is stopped mid-write."""


def _monotonic_ns() -> int:
    return time.clock_gettime_ns(time.CLOCK_MONOTONIC)


def _shm_name(name: str) -> str:
    if not name.startswith("/"):
        raise ValueError(f"segment names start with '/', got {name!r}")
    return name


class InputDevice:
    """The writer side: creates, writes and (on :meth:`close`) removes a segment."""

    def __init__(self, name: str, mm: mmap.mmap, n_axes: int) -> None:
        self._name = name
        self._mm = mm
        self._n_axes = n_axes

    @classmethod
    def create(
        cls,
        name: str,
        axes: Sequence[AxisSpec | tuple[str, Semantic] | tuple[str, Semantic, float]],
    ) -> InputDevice:
        """Create the segment ``name`` (e.g. ``"/vstimd_wheel"``) with ``axes``.

        A segment left by a producer that crashed is replaced. The segment is
        readable by other users (mode 0644), since vstimd usually runs as its own.
        """
        posix = _posix()
        specs = [a if isinstance(a, AxisSpec) else AxisSpec(*a) for a in axes]
        if not 1 <= len(specs) <= MAX_AXES:
            raise ValueError(f"an input device needs 1..{MAX_AXES} axes, got {len(specs)}")
        shm = _shm_name(name)
        try:
            posix.shm_unlink(shm)
        except FileNotFoundError:
            pass
        fd = posix.shm_open(shm, os.O_CREAT | os.O_EXCL | os.O_RDWR, mode=0o644)
        try:
            os.fchmod(fd, 0o644)  # the umask must not make it unreadable
            os.ftruncate(fd, SHM_SIZE)
            mm = mmap.mmap(fd, SHM_SIZE)
        except OSError:
            posix.shm_unlink(shm)
            raise
        finally:
            os.close(fd)

        struct.pack_into("<III", mm, 0, MAGIC, VERSION, len(specs))
        mm[16 : 16 + NAME_LEN] = _fixed(name, NAME_LEN)
        for i, a in enumerate(specs):
            off = AXES_OFFSET + i * AXIS_RECORD
            mm[off : off + AXIS_NAME_LEN] = _fixed(a.name, AXIS_NAME_LEN)
            struct.pack_into("<B3xff", mm, off + AXIS_NAME_LEN, int(a.semantic), a.scale, a.deadzone)
        struct.pack_into("<Q", mm, _HEARTBEAT, _monotonic_ns())
        return cls(name, mm, len(specs))

    @property
    def name(self) -> str:
        return self._name

    def write(self, values: Sequence[float]) -> int:
        """Publish one sample and refresh the heartbeat. Returns values written.

        Follows the seqlock protocol: the sequence goes odd, the values and the
        heartbeat are stored, the sequence goes even again.
        """
        n = min(len(values), self._n_axes)
        now = _monotonic_ns()
        mm = self._mm
        (seq,) = struct.unpack_from("<I", mm, _SEQ)
        struct.pack_into("<I", mm, _SEQ, (seq + 1) & 0xFFFF_FFFF)  # odd: writing
        struct.pack_into(f"<{n}d", mm, _VALUES, *values[:n])
        struct.pack_into("<Q", mm, _HEARTBEAT, now)
        (count,) = struct.unpack_from("<Q", mm, _WRITE_COUNT)
        struct.pack_into("<Q", mm, _WRITE_COUNT, count + 1)
        struct.pack_into("<I", mm, _SEQ, (seq + 2) & 0xFFFF_FFFF)  # even: done
        return n

    def close(self, *, unlink: bool = True) -> None:
        """Unmap, and by default remove the segment so vstimd sees it go."""
        self._mm.close()
        if unlink:
            try:
                _posix().shm_unlink(_shm_name(self._name))
            except FileNotFoundError:
                pass

    def __enter__(self) -> InputDevice:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


class InputDeviceReader:
    """A read-only view of a segment — for diagnostics and tests; vstimd has its own."""

    def __init__(self, name: str) -> None:
        fd = _posix().shm_open(_shm_name(name), os.O_RDONLY, mode=0)
        try:
            self._mm = mmap.mmap(fd, SHM_SIZE, prot=mmap.PROT_READ)
        finally:
            os.close(fd)
        magic, version, n_axes = struct.unpack_from("<III", self._mm, 0)
        if magic != MAGIC or version != VERSION or n_axes > MAX_AXES:
            self._mm.close()
            raise ValueError(f"{name} is not a vinput v{VERSION} segment")
        self.n_axes = n_axes

    @property
    def device_name(self) -> str:
        return _unfixed(self._mm[16 : 16 + NAME_LEN])

    @property
    def axes(self) -> list[AxisSpec]:
        out = []
        for i in range(self.n_axes):
            off = AXES_OFFSET + i * AXIS_RECORD
            name = _unfixed(self._mm[off : off + AXIS_NAME_LEN])
            semantic, scale, deadzone = struct.unpack_from("<B3xff", self._mm, off + AXIS_NAME_LEN)
            out.append(AxisSpec(name, Semantic(semantic), scale, deadzone))
        return out

    def read(self) -> list[float]:
        """A coherent sample, or :class:`TornRead` if none could be taken."""
        for _ in range(MAX_READ_SPINS):
            (before,) = struct.unpack_from("<I", self._mm, _SEQ)
            if before & 1:
                continue
            values = list(struct.unpack_from(f"<{self.n_axes}d", self._mm, _VALUES))
            (after,) = struct.unpack_from("<I", self._mm, _SEQ)
            if after == before:
                return values
        raise TornRead()

    def age_ns(self) -> int:
        (heartbeat,) = struct.unpack_from("<Q", self._mm, _HEARTBEAT)
        return max(0, _monotonic_ns() - heartbeat)

    @property
    def write_count(self) -> int:
        return struct.unpack_from("<Q", self._mm, _WRITE_COUNT)[0]

    def close(self) -> None:
        self._mm.close()


def _fixed(text: str, size: int) -> bytes:
    raw = text.encode()[: size - 1]
    return raw + b"\0" * (size - len(raw))


def _unfixed(raw: bytes) -> str:
    return raw.split(b"\0", 1)[0].decode(errors="replace")
