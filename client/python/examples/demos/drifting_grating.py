"""drifting_grating.py — Build the `demo_drifting_grating` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/drifting_grating.py
    uv run examples/demos/drifting_grating.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/drifting_grating.py --save-as my_drifting_grating -f

Reproduces the shipped `demo_drifting_grating` config: one full-field sinusoidal
grating that drifts on its own, with no animation and no trigger — the motion
comes from the grating's own `drift_speed_hz`, advanced by the render thread once
per frame.

See docs/tutorials/drifting-grating.md for the walkthrough.
"""

import sys

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection

from vstimd.stimuli.stimuli_models import Vec2
from vstimd.stimuli import GratingMask, GratingParams, GratingTexture

EXPLANATION = (
    "demo_drifting_grating — a moving stimulus every frame\n"
    "\n"
    "Full-field sinusoidal grating: 0.01 cycles/px (100 px per cycle),\n"
    "vertical stripes, drifting at 4 cycles/s perpendicular to the stripes.\n"
    "Runs on load — no trigger. Tearing or stutter here means a frame-timing problem.\n"
    "Edit sf_cycles_per_px / drift_speed_hz / rotation live, then save it under your own name."
)


def main() -> None:
    args = demo_parser(__doc__.splitlines()[0], "my_drifting_grating").parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)

        # Mid grey: a sinusoidal grating modulates around mean luminance, so a
        # grey surround keeps the whole frame at roughly one adaptation level.
        conn.system.set_background(0.5, 0.5, 0.5)

        # ── The grating ───────────────────────────────────────────────────────
        # 2400 x 1400 px overfills a 1920 x 1080 frame, which is what makes it
        # full-field: no edge of the patch is ever on screen. sf_cycles_per_px is in
        # cycles per pixel, so 0.01 is one cycle per 100 px; drift_speed_hz is in
        # cycles per second and is advanced by the render thread, not by us —
        # that is what keeps the motion frame-accurate with no client attached.
        conn.stimuli.grating.create_grating(
            position_px=Vec2(0, 0),
            rotation_deg=0.0,  # vertical stripes
            name="full_field_grating",
            params=GratingParams(
                width_px=2400, height_px=1400, sf_cycles_per_px=0.01, contrast=1.0,
                waveform=GratingTexture.SIN, mask=GratingMask.NONE,
                drift_speed_hz=4.0,  # cycles/s, perpendicular to the stripes
            ),
        )

        add_explanation(conn, EXPLANATION)

        conn.config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}' — it starts drifting the moment it is loaded.")

if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
