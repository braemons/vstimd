"""Protocol driver: sends flash commands to the vstim_server and records timestamps.

This module drives the visual stimulus server to display a series of full-field
flash stimuli, recording when each command was sent so the test tool can
compute ZMQ command-to-photon latency.

Phase 3 (ZMQ PUB events) is scaffolded here but not yet active — it requires
the server to emit FrameFlip events on tcp://*:5556.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field


@dataclass
class FlashRecord:
    """Metadata for a single flash command."""
    index: int
    t_cmd_send_s: float    # when the ZMQ command was sent
    t_flip_s: float = 0.0  # server flip time (Phase 3: from ZMQ PUB events)


def run_flash_protocol(
    n_flashes: int,
    hz: float,
    server_address: str = "tcp://localhost:5555",
    use_zmq_events: bool = False,
    verbose: bool = False,
) -> list[FlashRecord]:
    """Drive the stimulus server through N on/off cycles at the given rate.

    Returns a list of FlashRecord (one per flash) with cmd timestamps.

    If ``use_zmq_events=True`` and the server supports it, also fills in
    ``t_flip_s`` from the FrameFlip ZMQ PUB messages.

    Parameters
    ----------
    n_flashes:
        Number of on/off flash cycles to send.
    hz:
        Flash rate in Hz.
    server_address:
        ZMQ REQ address of the vstim server.
    use_zmq_events:
        If True, subscribe to tcp://*:5556 for FrameFlip events (Phase 3).
    verbose:
        Print progress.
    """
    try:
        import zmq  # type: ignore[import]
    except ImportError as e:
        raise ImportError("pyzmq not installed — run: uv pip install pyzmq") from e

    ctx = zmq.Context()
    sock = ctx.socket(zmq.REQ)
    sock.connect(server_address)
    sock.setsockopt(zmq.RCVTIMEO, 2000)  # 2 s timeout

    period_s = 1.0 / hz
    records: list[FlashRecord] = []

    try:
        for i in range(n_flashes):
            # Send ON command
            t_send = time.perf_counter()
            sock.send_json({"cmd": "set_background", "r": 1.0, "g": 1.0, "b": 1.0})
            try:
                sock.recv_json()
            except zmq.Again:
                if verbose:
                    print(f"  [protocol] Flash {i}: no ACK (server timeout)")

            records.append(FlashRecord(index=i, t_cmd_send_s=t_send))

            # Wait half period
            time.sleep(period_s / 2)

            # Send OFF command
            sock.send_json({"cmd": "set_background", "r": 0.0, "g": 0.0, "b": 0.0})
            try:
                sock.recv_json()
            except zmq.Again:
                pass

            # Wait remaining half period
            time.sleep(period_s / 2)

            if verbose and i % 30 == 0:
                print(f"  [protocol] Flash {i + 1}/{n_flashes}")

    finally:
        sock.close()
        ctx.term()

    return records


def calibrate_clock(
    server_address: str = "tcp://localhost:5555",
    n_samples: int = 10,
) -> float:
    """Measure Python↔server clock offset in milliseconds.

    Sends n_samples ``get_time`` commands and averages the estimated offset.
    Returns ``clock_offset_ms`` such that::

        server_time_ns ≈ time.time_ns() + clock_offset_ms * 1e6

    Requires the server to implement the ``get_time`` command (Phase 3).
    Returns 0.0 if the server does not support it.
    """
    try:
        import zmq  # type: ignore[import]
    except ImportError:
        return 0.0

    ctx = zmq.Context()
    sock = ctx.socket(zmq.REQ)
    sock.connect(server_address)
    sock.setsockopt(zmq.RCVTIMEO, 500)

    offsets_ns: list[float] = []
    try:
        for _ in range(n_samples):
            t_before = time.time_ns()
            sock.send_json({"cmd": "get_time"})
            try:
                reply = sock.recv_json()
                t_after = time.time_ns()
                server_ns = reply.get("server_ns")
                if server_ns is not None:
                    mid_ns = (t_before + t_after) / 2
                    offsets_ns.append(server_ns - mid_ns)
            except zmq.Again:
                break
    finally:
        sock.close()
        ctx.term()

    if not offsets_ns:
        return 0.0
    # Average, ignoring outliers (discard min and max if enough samples)
    if len(offsets_ns) >= 4:
        offsets_ns = sorted(offsets_ns)[1:-1]
    return sum(offsets_ns) / len(offsets_ns) / 1e6  # → ms
