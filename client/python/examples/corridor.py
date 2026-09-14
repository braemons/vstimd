"""corridor.py — Walk down an endless striped corridor.

Usage
-----
    # Server must already be running:
    #   cd server && cargo run

    uv run examples/corridor.py                      # default tcp://localhost:5555
    uv run examples/corridor.py --speed 60 --seconds 20

A corridor repeats every period, and the camera animation wraps its position
into one period, so the walk never runs out of corridor. Every sphere recurs
once per period too. The script prints the true distance walked, which is the
number to record in an experiment — the camera position itself wraps.
"""

import argparse
import time

from vstimd import Connection
from vstimd.stimuli import (
    Color,
    Corridor3DParams,
    Material3D,
    Repeat3D,
    Shading,
    Sphere3DParams,
    Transform3D,
    Vec3,
)
from vstimd.system import Camera3D

PERIOD_CM = 100.0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("address", nargs="?", default="tcp://localhost:5555")
    parser.add_argument("--speed", type=float, default=40.0, help="cm/s (default: 40)")
    parser.add_argument("--seconds", type=float, default=10.0)
    args = parser.parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        corridor = conn.stimuli.shapes3d.create_corridor(
            name="corridor",
            params=Corridor3DParams(
                period_cm=PERIOD_CM, periods_ahead=40, periods_behind=1, ceiling=True,
                material=Material3D(shading=Shading.PHONG),
            ),
        )
        spheres = conn.stimuli.shapes3d.create_sphere(
            name="landmark",
            transform=Transform3D(position_cm=Vec3(-18.0, 8.0, -50.0)),
            params=Sphere3DParams(
                diameter_cm=8.0,
                material=Material3D(albedo=Color(0.2, 0.6, 0.9), shading=Shading.PHONG),
                repeat=Repeat3D(period_cm=PERIOD_CM, ahead=40, behind=1),
            ),
        )
        conn.system.set_camera(Camera3D(position_cm=Vec3(0.0, 20.0, 0.0)))
        walk = conn.animations.create_linear_nav_3d(args.speed, wrap_period_cm=PERIOD_CM)
        conn.animations.arm(walk)
        try:
            end = time.monotonic() + args.seconds
            while time.monotonic() < end:
                time.sleep(1.0)
                d = conn.animations.query(walk).distance_travelled_cm
                print(f"walked {d:7.1f} cm")
        finally:
            conn.animations.delete(walk)
            conn.stimuli.delete(spheres)
            conn.stimuli.delete(corridor)
            conn.system.set_camera(Camera3D())


if __name__ == "__main__":
    main()
