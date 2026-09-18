"""gaussian_splat_room.py — A trained Gaussian splat scene, walked through.

Usage
-----
    # Server must already be running (needs a real display — splats do not
    # render on the null renderer):
    #   cargo run --release --windowed 1280x720

    uv run examples/demos/gaussian_splat_room.py ~/.cache/vstimd-samples/room-7k.splat
    uv run examples/demos/gaussian_splat_room.py scene.ply --scale 100 --speed 40

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

Placement is the one thing a trained scene needs and a primitive does not. A
scene has no units and, straight out of COLMAP, is upside down in vstimd's Y-up
world — hence `--scale` (centimetres per scene unit) and the 180° pitch.
"""

import sys

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection
from vstimd.stimuli import Transform3D, Vec3
from vstimd.system import Camera3D

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
    args = parser.parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
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
        scene = conn.stimuli.gaussian_splat.create(
            args.path,
            name="splat_scene",
            transform=Transform3D(
                position_cm=Vec3(0, 0, 0),
                rotation_deg=Vec3(0, 180, 0),
                scale=Vec3(args.scale, args.scale, args.scale),
            ),
        )

        # ── The camera ────────────────────────────────────────────────────────
        # Eye height above the scene origin, looking down -Z like the default.
        conn.system.set_camera(Camera3D(position_cm=Vec3(0, 5, 20)))

        # ── The walk ──────────────────────────────────────────────────────────
        # A finite track, not a wrap: a captured room does not repeat, so at the
        # end the 3-D view fades to the background over 30 frames, the camera
        # jumps back to its start, and it fades in again.
        walk = conn.animations.create_linear_nav_3d(
            args.speed,
            track_length_cm=args.track_cm,
            fade_frames=30,
            name="walk",
        )

        add_explanation(conn, EXPLANATION)

        conn.animations.arm(walk)
        print(
            f"Walking: handle={scene}, {args.speed:g} cm/s over {args.track_cm:g} cm.\n"
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
