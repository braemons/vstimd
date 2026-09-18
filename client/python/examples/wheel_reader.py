"""wheel_reader.py — Publish a rotary encoder or running wheel to vstimd.

A reference producer for an input device. It reads encoder counts from a
serial port (one signed integer per line — adapt `read_counts` to your
hardware) and publishes them to shared memory, where vstimd reads them every
frame for any animation that names the device.

Usage
-----
    python examples/wheel_reader.py /dev/ttyACM0
    python examples/wheel_reader.py --simulate          # no hardware: a steady run

Declare the device in the rig-config (/etc/braemons/vstimd-rig-config.toml):

    [[input.device]]
    name = "wheel"
    shm  = "/vstimd_wheel"
      [[input.device.axis]]
      name     = "distance"
      semantic = "cumulative"
      scale    = 0.0127      # counts → cm for your wheel

and drive the camera with it:

    conn.animations.create_linear_nav_3d(0, source=AxisRef("wheel", "distance"),
                                         wrap_period_cm=100)
"""

import argparse
import time
from collections.abc import Iterator

from vstimd.shm import InputDevice, Semantic


def read_counts(port: str) -> Iterator[int]:
    """Counts since the last line, from a microcontroller printing one per line."""
    with open(port, "rb", buffering=0) as serial:
        for line in serial:
            try:
                yield int(line.strip() or 0)
            except ValueError:
                continue


def simulated_counts(rate_hz: float = 500.0) -> Iterator[int]:
    while True:
        time.sleep(1.0 / rate_hz)
        yield 2


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("port", nargs="?", help="serial device, e.g. /dev/ttyACM0")
    parser.add_argument("--simulate", action="store_true", help="publish a steady synthetic run")
    parser.add_argument("--shm", default="/vstimd_wheel", help="segment name (default: /vstimd_wheel)")
    args = parser.parse_args()
    if not args.port and not args.simulate:
        parser.error("give a serial port, or --simulate")

    counts = simulated_counts() if args.simulate else read_counts(args.port)
    with InputDevice.create(args.shm, [("distance", Semantic.CUMULATIVE)]) as dev:
        print(f"publishing {args.shm} — Ctrl+C to stop")
        # The running total, never the per-read delta. vstimd subtracts the
        # total it saw last frame from the one it sees now, so a read it misses
        # is folded into the next change instead of vanishing — and this
        # process never has to know when, or how often, vstimd reads.
        total = 0
        try:
            for n in counts:
                total += n
                # Write every sample, even an unchanged total: each write is a
                # heartbeat, and a silent producer is treated as a dead one.
                dev.write([total])
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
