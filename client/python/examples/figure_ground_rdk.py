"""figure_ground_rdk.py — The figure-ground random dot kinematogram.

A direct port of the Psychtoolbox stimulus in
``stimulusStageFigureRDBackgroundComponentsCLaser_BW.m``, which is the reference
this implementation was built to reproduce (see ``dev/design/RDK_PLAN.md``).

Two dot fields cover the whole screen. They are identical in dot size, density and
speed, and differ only in **direction**:

  * the **background** is visible only *outside* a 45° circle on the receptive
    field — an inverted circular aperture;
  * the **figure** is visible only *inside* the same circle.

Nothing but motion separates figure from ground in any single frame: no luminance,
density or texture cue. The four conditions below are ``params{1..4}`` of the
original, which differ only in which field moves coherently and which is drawn.

The two places a Psychtoolbox port goes silently wrong are both handled here, at
the boundary and nowhere else:

  * every MATLAB size is a **radius**, every vstimd size a **diameter** —
    ``dotSize = 1.5`` is a dot 3° across, ``R = 45/2`` a circle 45° across;
  * MATLAB adds ``sin(angle)`` to a row index, which grows *downward*, so its
    angles run clockwise — ``3*pi/2`` is **up**, which is 90° here, not 270°.

Usage
-----
    uv run examples/figure_ground_rdk.py
    uv run examples/figure_ground_rdk.py --duration 10 tcp://192.168.1.10:5555
"""

from __future__ import annotations

import argparse
import math
import time
from dataclasses import replace

from vstimd import Connection
from vstimd.stimuli import (
    Aperture,
    ApertureClip,
    ApertureShape,
    Color,
    DotsParams,
    NoiseRule,
    Reinsertion,
    SignalRule,
    Vec2,
    diameter_from_radius,
    direction_from_ptb_rad,
    dots_for_density,
    px_per_deg,
)

# ── The MATLAB's numbers, in its own units ────────────────────────────────────

N_DOTS_SPACING_DEG = 5.0        # dotSpacing — one dot per (5 deg)²
DOT_RADIUS_DEG = 1.5            # dotSize, which is a RADIUS
FIGURE_RADIUS_DEG = 45.0 / 2.0  # R = [45]/2, likewise a radius
SPEED_DEG_PER_S = 50.0          # vel
DIR_BACKGROUND_RAD = 0.0                # dirAngleBackground = 0
DIR_FIGURE_RAD = 3.0 * math.pi / 2.0    # dirAngleFigure = 3*pi/2, i.e. UP

# The rig. Replace with your own geometry — this is the only place it appears.
SCREEN_PX = (1920, 1080)
SCREEN_WIDTH_CM = 52.0
VIEWING_DISTANCE_CM = 57.0
RF_CENTER_DEG = (10.0, -7.5)  # screenInfo.RFcenter


def build_params() -> tuple[DotsParams, Aperture]:
    """The parameters shared by both fields, and the figure aperture."""
    ppd = px_per_deg(SCREEN_PX[0], SCREEN_WIDTH_CM, VIEWING_DISTANCE_CM)
    field_w_deg = SCREEN_PX[0] / ppd
    field_h_deg = SCREEN_PX[1] / ppd

    common = DotsParams(
        field_width_px=float(SCREEN_PX[0]),
        field_height_px=float(SCREEN_PX[1]),
        # The MATLAB generates a 161 × 161 lattice, jittered by about twice its own
        # spacing — which is a uniform random field at this density, not a lattice.
        dot_count=dots_for_density(
            1.0 / (N_DOTS_SPACING_DEG**2), field_w_deg, field_h_deg
        ),
        dot_size_px=diameter_from_radius(DOT_RADIUS_DEG) * ppd,
        dot_color=Color(1.0, 1.0, 1.0, 1.0),  # figureDotIntensity = 255
        speed_px_per_s=SPEED_DEG_PER_S * ppd,
        coherence=1.0,                        # cohPropBackground = cohPropFigure = 1
        signal_rule=SignalRule.SAME,
        noise_rule=NoiseRule.DIRECTION,
        # The original's 805°-wide lattice is how it supplies dots streaming in for
        # a 2 s trial without ever wrapping. A one-screen field that wraps is
        # statistically identical and seamless — the wrap boundary is outside the
        # aperture, so it leaks no edge cue.
        reinsertion=Reinsertion.WRAP,
        dot_lifetime_frames=0,  # the original never reborns a dot
    )
    figure_circle = Aperture(
        shape=ApertureShape.CIRCLE,
        width_px=diameter_from_radius(FIGURE_RADIUS_DEG) * ppd,
        offset_px=Vec2(RF_CENTER_DEG[0] * ppd, RF_CENTER_DEG[1] * ppd),
        # Dots overhang the boundary uncut, as the MATLAB's centre-pixel test does.
        # Cutting them would draw a crisp circle — a static form cue, which is
        # exactly what a motion-defined figure must not have.
        clip=ApertureClip.DOT_CENTER,
    )
    return common, figure_circle


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("address", nargs="?", default="tcp://127.0.0.1:5555")
    ap.add_argument("--duration", type=float, default=4.0,
                    help="seconds to show each condition")
    args = ap.parse_args()

    common, figure_circle = build_params()
    background_dir = direction_from_ptb_rad(DIR_BACKGROUND_RAD)  # 0°
    figure_dir = direction_from_ptb_rad(DIR_FIGURE_RAD)          # 90°, i.e. up

    with Connection(args.address) as conn:
        conn.system.set_background(0.5, 0.5, 0.5)  # backgroundCol = 128 on 0-255

        background = conn.stimuli.dots.create_dots(
            name="background",
            params=replace(
                common,
                aperture=replace(figure_circle, invert=True),
                direction_deg=background_dir,
                seed=1,
            ),
        )
        figure = conn.stimuli.dots.create_dots(
            name="figure",
            params=replace(common, aperture=figure_circle,
                           direction_deg=figure_dir, seed=2),
        )

        # params{1..4}: which field moves coherently, and which is drawn at all.
        # `*DotIntensity = 0` in the original means "omit this field", which here
        # is the shared enable rather than a colour of black.
        conditions = [
            ("both fields move — the figure is defined by direction alone",
             (True, figure_dir), (True, background_dir)),
            ("figure only — the background is static",
             (True, figure_dir), (True, background_dir * 0.0)),
            ("background only — a moving 'hole' where the figure was",
             (True, 0.0), (True, background_dir)),
            ("neither — a static field, the blank control",
             (True, 0.0), (True, 0.0)),
        ]
        try:
            for label, (fig_on, fig_dir), (bg_on, bg_dir) in conditions:
                print(f"→ {label}")
                conn.stimuli.set_enabled(figure, fig_on)
                conn.stimuli.set_enabled(background, bg_on)
                conn.stimuli.dots.set_direction(figure, fig_dir)
                conn.stimuli.dots.set_direction(background, bg_dir)
                # A fresh sample per condition, recorded in the config. Replaying
                # these seeds replays these exact dots.
                conn.stimuli.dots.set_seed(figure, 2)
                conn.stimuli.dots.set_seed(background, 1)
                time.sleep(args.duration)
        finally:
            conn.stimuli.delete(figure)
            conn.stimuli.delete(background)


if __name__ == "__main__":
    main()
