"""Command-line interface for vstimd_timing_test."""

from __future__ import annotations

import argparse
import sys
import time


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="vstimd-timing-test",
        description="Measure visual stimulus frame timing using a photodiode + DAQ.",
    )
    p.add_argument(
        "--backend",
        choices=["auto", "ni", "t4", "u3", "simulated"],
        default="auto",
        help="DAQ backend (default: auto-detect)",
    )
    p.add_argument(
        "--server",
        default="tcp://localhost:5555",
        metavar="ADDR",
        help="vstimd ZMQ address (default: tcp://localhost:5555)",
    )
    p.add_argument(
        "--hz",
        type=float,
        default=60.0,
        help="Flash rate in Hz (default: 60)",
    )
    p.add_argument(
        "--duration",
        type=float,
        default=5.0,
        metavar="SEC",
        help="Acquisition duration in seconds (default: 5)",
    )
    p.add_argument(
        "--out",
        default=None,
        metavar="PATH",
        help="Output CSV path (also writes <PATH>.json). Skipped if not given.",
    )
    p.add_argument(
        "--threshold-v",
        type=float,
        default=1.5,
        metavar="V",
        help="Photodiode edge detection threshold voltage (default: 1.5 V)",
    )
    p.add_argument(
        "--use-zmq-events",
        action="store_true",
        help="Subscribe to server FrameFlip ZMQ PUB events (Phase 3; requires server support)",
    )
    p.add_argument(
        "--no-server",
        action="store_true",
        help="Skip ZMQ server connection (use with --backend simulated for offline testing)",
    )
    p.add_argument(
        "--verbose",
        action="store_true",
        help="Print progress during acquisition",
    )
    return p


def main(argv: list[str] | None = None) -> int:
    """Entry point.  Returns 0 on PASS, 1 on WARN, 2 on FAIL."""
    from .analysis import analyze, Thresholds
    from .backends import auto_detect
    from .report import print_report

    p = build_parser()
    args = p.parse_args(argv)

    n_flashes = max(1, int(args.duration * args.hz))

    # --- Select backend ---
    print(f"[vstimd-timing-test] Backend: {args.backend}")
    if args.backend == "simulated":
        from .backends.simulation import SimulatedBackend
        backend = SimulatedBackend(hz=args.hz, duration_s=args.duration)
    else:
        backend = auto_detect(prefer=args.backend)
    print(f"  Using: {backend.name}  ({backend.sample_rate_hz} Hz)")

    # --- Clock calibration (Phase 3) ---
    clock_offset_ms = 0.0
    if not args.no_server and args.use_zmq_events:
        from .protocol import calibrate_clock
        print("[vstimd-timing-test] Calibrating clock offset...")
        clock_offset_ms = calibrate_clock(args.server)
        print(f"  Clock offset: {clock_offset_ms:.2f} ms")

    # --- Drive protocol & acquire simultaneously ---
    import numpy as np
    from .protocol import run_flash_protocol

    # Start acquisition
    backend.start_acquisition()

    # Drive flashes (unless --no-server)
    records = []
    if not args.no_server:
        try:
            records = run_flash_protocol(
                n_flashes=n_flashes,
                hz=args.hz,
                server_address=args.server,
                use_zmq_events=args.use_zmq_events,
                verbose=args.verbose,
            )
        except Exception as e:
            print(f"[vstimd-timing-test] WARNING: Could not connect to server: {e}")
            print("  Running in offline mode (no protocol commands sent).")
    else:
        # Simulated: just wait for the acquisition duration
        if args.verbose:
            print(f"  --no-server: sleeping {args.duration:.1f} s")
        time.sleep(args.duration)

    # Stop acquisition
    timestamps_s, voltages_v = backend.stop_acquisition()
    backend.close()

    # Build cmd timestamps
    cmd_timestamps_s = (
        np.array([r.t_cmd_send_s for r in records], dtype=np.float64)
        if records else None
    )
    flip_timestamps_s = (
        np.array([r.t_flip_s for r in records if r.t_flip_s > 0], dtype=np.float64)
        if records else None
    )
    if flip_timestamps_s is not None and len(flip_timestamps_s) == 0:
        flip_timestamps_s = None

    # --- Analyze ---
    result = analyze(
        timestamps_s=timestamps_s,
        voltages_v=voltages_v,
        n_flashes_expected=n_flashes,
        expected_hz=args.hz,
        cmd_timestamps_s=cmd_timestamps_s,
        flip_timestamps_s=flip_timestamps_s,
        clock_offset_ms=clock_offset_ms,
        threshold_v=args.threshold_v,
    )

    # --- Report ---
    print_report(result)

    # --- Export ---
    if args.out:
        from .export import export_csv, export_json
        csv_path = export_csv(result, args.out)
        json_path = export_json(
            result,
            str(args.out) + ".json",
            metadata=backend.get_device_info(),
        )
        print(f"  Results written to {csv_path} and {json_path}")

    # Return code
    verdict_code = {"PASS": 0, "WARN": 1, "FAIL": 2}
    return verdict_code.get(result.verdict, 2)


if __name__ == "__main__":
    sys.exit(main())
