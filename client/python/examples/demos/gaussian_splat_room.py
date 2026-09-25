"""gaussian_splat_room.py — A trained Gaussian splat scene, walked through.

Usage
-----
    # Server must already be running (needs a real display — splats do not
    # render on the null renderer):
    #   cargo run --release --windowed 1280x720

    uv run examples/demos/gaussian_splat_room.py ~/.cache/vstimd-samples/room-7k.splat
    uv run examples/demos/gaussian_splat_room.py scene.ply --scale 100 --speed 40
    uv run examples/demos/gaussian_splat_room.py room-7k.splat --wheel wheel:wheel
    uv run examples/demos/gaussian_splat_room.py office.ply --placed \
        --camera-height-cm 16 --track-cm 300 --wheel wheel:wheel

Unlike the other scripts here this one builds **no shipped config**: Gaussian
splatting is a prototype (dev/design/GAUSSIAN_SPLAT_PLAN.md), and the sample
scenes it wants are large research-licence captures that cannot be committed.
Point it at a `.ply` or `.splat` on the *server's* filesystem instead.

What it shows
-------------
A splat scene is an ordinary 3-D stimulus: placed by a `Transform3D`, seen
through the scene camera, driven by the same `LinearNav3D` animation a geometric
corridor uses, and captioned by an ordinary 2-D text box drawn over the top.
Only `conn.stimuli.gaussian_splat.create(path)` is splat-specific.

With ``--wheel DEVICE:AXIS`` the camera follows a cumulative axis of a
rig-config input device instead of ``--speed`` — e.g. mousewheeld publishing to
``/vstimd_wheel`` — so the scene is walked by turning the wheel.

Placement is the one thing a trained scene needs and a primitive does not. A
scene has no units and, straight out of COLMAP, is upside down in vstimd's Y-up
world — hence `--scale` (centimetres per scene unit) and the 180° pitch.

A scene from ``vstimd-scene-from-capture`` is already placed — centimetres,
floor at y = 0, the walk starting at the origin and running down −Z — so
``--placed`` loads it as it is and starts the camera at the origin. Give it the
camera height and walked path that the tool's report prints.
"""

import sys

from _common import add_explanation, clean_slate, demo_parser

from vstimd_client import VstimdClient
from vstimd_client.animations import AxisRef
from vstimd_client.stimuli import Transform3D, Vec3
from vstimd_client.system import Camera3D

EXPLANATION = (
    "Gaussian splat scene (prototype)\n"
    "\n"
    "A trained 3DGS capture, drawn back-to-front in the 3-D pass and walked\n"
    "with the same LinearNav3D animation a geometric corridor uses. The camera\n"
    "walks the track, the view fades out at its end, and the walk restarts.\n"
    "Colours are baked in by training — not a calibrated luminance."
)


def main() -> None:
    parser = demo_parser(__doc__.splitlines()[0], "")
    parser.add_argument(
        "path",
        help="server-side path to a .ply (3DGS layout) or antimatter15 .splat file",
    )
    parser.add_argument(
        "--scale", type=float, default=100.0,
        help="centimetres per scene unit (default: 100, right for the MipNeRF-360 captures)",
    )
    parser.add_argument(
        "--speed", type=float, default=60.0,
        help="walking speed in cm/s (default: 60)",
    )
    parser.add_argument(
        "--track-cm", type=float, default=400.0,
        help="track length in cm before the view fades and restarts (default: 400)",
    )
    parser.add_argument(
        "--camera-height-cm", type=float, default=5.0,
        help="the camera's height in the placed scene (default: 5, about a person's eye "
        "height in room-7k; its floor is at -142, so -139 is a mouse's)",
    )
    parser.add_argument(
        "--placed", action="store_true",
        help="a vstimd-scene-from-capture scene: no transform, camera starting at the origin",
    )
    parser.add_argument(
        "--wheel",
        metavar="DEVICE:AXIS",
        help="follow this rig-config input axis instead of --speed",
    )
    args = parser.parse_args()
    source = AxisRef(*args.wheel.split(":", 1)) if args.wheel else None

    print(f"Connecting to {args.address} …")
    with VstimdClient(args.address) as conn:
        clean_slate(conn)
        conn.system.set_background(0.0, 0.0, 0.0)

        # ── The scene ─────────────────────────────────────────────────────────
        # The path is checked here (a bad one raises InvalidArgumentError) and
        # the file is then read on a worker thread: the stimulus draws nothing
        # for the first second or so of a million-splat scene.
        #
        # rotation_deg=(0, 180, 0) is the COLMAP fix — a Y-down training frame
        # turned upright. scale is centimetres per scene unit.
        print(f"Loading {args.path} …")
        placement = Transform3D() if args.placed else Transform3D(
            position_cm=Vec3(0, 0, 0),
            rotation_deg=Vec3(0, 180, 0),
            scale=Vec3(args.scale, args.scale, args.scale),
        )
        scene = conn.stimuli.gaussian_splat.create(args.path, name="splat_scene", transform=placement)

        # ── The camera ────────────────────────────────────────────────────────
        # Eye height above the scene origin, looking down -Z like the default.
        # A placed scene's walk starts at the origin; the sample rooms are
        # entered from a little behind theirs.
        start_z = 0 if args.placed else 20
        conn.system.set_camera(Camera3D(position_cm=Vec3(0, args.camera_height_cm, start_z)))

        # ── The walk ──────────────────────────────────────────────────────────
        # A finite track, not a wrap: a captured room does not repeat, so at the
        # end the 3-D view fades to the background over 30 frames, the camera
        # jumps back to its start, and it fades in again.
        walk = conn.animations.create_linear_nav_3d(
            args.speed,
            track_length_cm=args.track_cm,
            fade_frames=30,
            source=source,
            name="walk",
        )

        add_explanation(conn, EXPLANATION)

        conn.animations.arm(walk)
        pace = f"following {args.wheel}" if source else f"{args.speed:g} cm/s"
        print(
            f"Walking: handle={scene}, {pace} over {args.track_cm:g} cm.\n"
            "Ctrl-C to stop."
        )
        try:
            while True:
                conn.system.wait_for_frames(120)
                nav = conn.animations.query(walk)
                print(f"  travelled {nav.distance_travelled_cm:8.1f} cm", flush=True)
        except KeyboardInterrupt:
            conn.animations.cancel(walk)
            print("\nStopped.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
