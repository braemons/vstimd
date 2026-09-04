"""figure_ground_rdk.py — Build the `demos/figure_ground_rdk` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/figure_ground_rdk.py
    uv run examples/demos/figure_ground_rdk.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/figure_ground_rdk.py --save-as my_figure_ground_rdk -f

Reproduces the shipped `demos/figure_ground_rdk` config: two full-screen dot
fields, one masked to a circle and the other to its exact complement
(`Aperture(invert=True)`), that differ only in `direction_deg`. Nothing but
motion separates the figure from the ground — no luminance, density or texture
cue distinguishes them in any single frame.

This is a direct port of the Psychtoolbox
`stimulusStageFigureRDBackgroundComponentsCLaser_BW.m` reference; see
`dev/design/RDK_PLAN.md` for the full port notes, including the two places a
Psychtoolbox port goes silently wrong (radius vs. diameter, and the angle
convention).
"""

import sys
from dataclasses import replace

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection
from vstimd.stimuli import Aperture, ApertureClip, ApertureShape, Color, DotsParams
from vstimd.stimuli.stimuli_models import Vec2

EXPLANATION = (
    "demos/figure_ground_rdk — a figure defined by motion alone\n"
    "\n"
    "Two full-screen dot fields: one inside a 500 px circle, one outside it —\n"
    "the exact complement, so together they cover the whole screen with no gap\n"
    "or overlap in where each field is *allowed* to be seen. The figure drifts up,\n"
    "the ground drifts right. Freeze either field alone and there is nothing to see:\n"
    "the circle exists only in how the dots move, never in their density or colour.\n"
    "Runs on load — no trigger."
)

FIELD_PX = Vec2(1920.0, 1080.0)
FIGURE_DIAMETER_PX = 500.0
DOT_SIZE_PX = 8.0
DOT_COUNT = 900  # split evenly between the two fields by the aperture test
SPEED_PX_PER_S = 200.0


def main() -> None:
    args = demo_parser(__doc__.splitlines()[0], "my_figure_ground_rdk").parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)

        # Mid grey, as the original's backgroundCol = 128 on a 0-255 scale.
        conn.system.set_background(0.5, 0.5, 0.5)

        # ── The shared field and the figure circle ───────────────────────────
        figure_circle = Aperture(
            shape=ApertureShape.CIRCLE,
            # height_px is ignored for a circle, but 0 means "the field" — pass
            # the diameter on both axes so the aperture round-trips as the circle
            # it is, rather than as a circle-shaped stand-in for the field height.
            width_px=FIGURE_DIAMETER_PX,
            height_px=FIGURE_DIAMETER_PX,
            # Dots overhang the boundary uncut, as the MATLAB's centre-pixel test
            # does — cutting them at the edge would draw a crisp circle, a static
            # form cue that a motion-defined figure must not have.
            clip=ApertureClip.DOT_CENTER,
        )
        common = DotsParams(
            field_width_px=FIELD_PX.x,
            field_height_px=FIELD_PX.y,
            dot_count=DOT_COUNT,
            dot_size_px=DOT_SIZE_PX,
            dot_color=Color(1.0, 1.0, 1.0),
            speed_px_per_s=SPEED_PX_PER_S,
            coherence=1.0,
        )

        # invert=True is the exact complement of figure_circle — together the two
        # apertures partition the whole field with no gap and no double cover.
        ground = conn.stimuli.dots.create_dots(
            name="ground",
            params=replace(
                common, aperture=replace(figure_circle, invert=True), direction_deg=0.0, seed=1,
            ),
        )
        figure = conn.stimuli.dots.create_dots(
            name="figure",
            params=replace(common, aperture=figure_circle, direction_deg=90.0, seed=2),
        )

        add_explanation(conn, EXPLANATION)

        conn.scene_config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}' — the figure is moving the moment it loads.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
