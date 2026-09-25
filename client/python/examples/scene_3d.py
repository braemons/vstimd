"""scene_3d.py — A lit 3-D scene: a spinning cube and a sphere on a floor.

Usage
-----
    # Server must already be running:
    #   cd server && cargo run

    uv run examples/scene_3d.py                      # default tcp://localhost:5555
    uv run examples/scene_3d.py tcp://192.168.1.10:5555
    uv run examples/scene_3d.py --seconds 20

The cube turns once every four seconds while the camera swings slowly from side
to side, so depth, lighting and the camera all move at once. Everything is
deleted and the camera and lighting restored when the script ends.
"""

import argparse
import math
import time

from vstimd_client import VstimdClient
from vstimd_client.stimuli import (
    Color,
    Cube3DParams,
    Material3D,
    Plane3DParams,
    Shading,
    Sphere3DParams,
    Transform3D,
    Vec2,
    Vec3,
)
from vstimd_client.system import Camera3D, Lighting3D


def lit(r: float, g: float, b: float) -> Material3D:
    return Material3D(albedo=Color(r, g, b), shading=Shading.PHONG)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("address", nargs="?", default="tcp://localhost:5555")
    parser.add_argument("--seconds", type=float, default=10.0)
    args = parser.parse_args()

    print(f"Connecting to {args.address} …")
    with VstimdClient(args.address) as conn:
        shapes3d = conn.stimuli.shapes3d
        conn.system.set_lighting(Lighting3D(
            ambient_color=Vec3(0.15, 0.15, 0.2),
            sun_direction=Vec3(-0.4, -1.0, -0.5),
        ))
        floor = shapes3d.create_plane(
            name="floor",
            transform=Transform3D(position_cm=Vec3(0, -15, -100)),
            params=Plane3DParams(size_cm=Vec2(160, 200), material=lit(0.35, 0.35, 0.4)),
        )
        sphere = shapes3d.create_sphere(
            name="sphere",
            transform=Transform3D(position_cm=Vec3(22, -3, -90)),
            params=Sphere3DParams(diameter_cm=24, material=lit(0.2, 0.55, 0.9)),
        )
        cube_placement = Transform3D(position_cm=Vec3(-15, 0, -80), rotation_deg=Vec3(0, 20, 0))
        cube = shapes3d.create_cube(
            name="cube",
            transform=cube_placement,
            params=Cube3DParams(size_cm=Vec3(22, 22, 22), material=lit(0.9, 0.35, 0.2)),
        )

        start = time.monotonic()
        try:
            while (t := time.monotonic() - start) < args.seconds:
                # Stage the cube and the camera together so they change on the
                # same frame.
                conn.system.set_deferred_mode(True)
                cube_placement.rotation_deg = Vec3(90.0 * t, 20.0, 0.0)
                shapes3d.set_transform(cube, cube_placement)
                conn.system.set_camera(Camera3D(
                    position_cm=Vec3(20.0 * math.sin(0.4 * t), 5.0, 0.0),
                    yaw_deg=10.0 * math.sin(0.4 * t),
                    pitch_deg=-5.0,
                ))
                conn.system.set_deferred_mode(False)
                conn.system.wait_for_frames(1)
        finally:
            for handle in (cube, sphere, floor):
                conn.stimuli.delete(handle)
            conn.system.set_camera(Camera3D())
            conn.system.set_lighting(Lighting3D())
    print("Done.")


if __name__ == "__main__":
    main()
