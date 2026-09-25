"""corridor.py — Walk down an endless striped corridor.

Usage
-----
    # Server must already be running:
    #   cd server && cargo run

    uv run examples/corridor.py                      # default tcp://localhost:5555
    uv run examples/corridor.py --speed 60 --seconds 20
    uv run examples/corridor.py --wheel wheel:wheel   # a rig-config input device

A corridor repeats every period, and the camera animation wraps its position
into one period, so the walk never runs out of corridor. Every sphere recurs
once per period too. The script prints the true distance walked, which is the
number to record in an experiment — the camera position itself wraps.

With ``--wheel DEVICE:AXIS`` the camera follows a cumulative axis of a
rig-config input device instead of a set speed — e.g. mousewheeld publishing
to ``/vstimd_wheel`` — and the walk lasts until Ctrl-C.
"""

import argparse
import time

from vstimd_client import VstimdClient
from vstimd_client.animations import AxisRef
from vstimd_client.stimuli import (
    Color,
    Corridor3DParams,
    Material3D,
    Repeat3D,
    Shading,
    Sphere3DParams,
    Transform3D,
    Vec3,
)
from vstimd_client.system import Camera3D

PERIOD_CM = 100.0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("address", nargs="?", default="tcp://localhost:5555")
    parser.add_argument("--speed", type=float, default=40.0, help="cm/s (default: 40)")
    parser.add_argument("--seconds", type=float, default=10.0)
    parser.add_argument(
        "--wheel",
        metavar="DEVICE:AXIS",
        help="follow this rig-config input axis instead of --speed, until Ctrl-C",
    )
    args = parser.parse_args()
    source = AxisRef(*args.wheel.split(":", 1)) if args.wheel else None

    print(f"Connecting to {args.address} …")
    with VstimdClient(args.address) as conn:
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
        walk = conn.animations.create_linear_nav_3d(
            0.0 if source else args.speed, wrap_period_cm=PERIOD_CM, source=source
        )
        conn.animations.arm(walk)
        try:
            end = float("inf") if source else time.monotonic() + args.seconds
            while time.monotonic() < end:
                time.sleep(1.0)
                d = conn.animations.query(walk).distance_travelled_cm
                print(f"walked {d:7.1f} cm")
        except KeyboardInterrupt:
            pass
        finally:
            conn.animations.delete(walk)
            conn.stimuli.delete(spheres)
            conn.stimuli.delete(corridor)
            conn.system.set_camera(Camera3D())


if __name__ == "__main__":
    main()
