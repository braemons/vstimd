"""trigger_gate.py — Build the `demos/trigger_gate` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/trigger_gate.py
    uv run examples/demos/trigger_gate.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/trigger_gate.py --save-as my_trigger_gate -f
    uv run examples/demos/trigger_gate.py --toggle       # drive the gate in software

Reproduces the shipped `demos/trigger_gate` config: a square-wave patch visible
exactly while an input line is HIGH. Level-coupled, not edge-triggered — there
is no duration, no re-arming, and no state to get stuck in, which is what makes
it the scene to load when you are debugging input wiring.

See docs/tutorials/trigger-gate.md for the walkthrough.
"""

import sys
import time

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection, VtlHandle, VtlKind, VtlPolarity

from vstimd.stimuli.stimuli_models import Vec2
from vstimd.stimuli import GratingMask, GratingParams, GratingTexture

EXPLANATION = (
    "demos/trigger_gate — visibility follows an input level\n"
    "\n"
    "The square-wave patch is visible exactly while in_pin7 (header pin 7) is HIGH,\n"
    "and hidden while it is LOW — level-coupled, not edge-triggered, so it needs\n"
    "no re-arming and shows input wiring problems immediately.\n"
    "No output pins are driven."
)


def main() -> None:
    parser = demo_parser(__doc__.splitlines()[0], "my_trigger_gate")
    parser.add_argument(
        "--toggle",
        action="store_true",
        help="toggle the input line a few times from software once the scene is built",
    )
    args = parser.parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)
        conn.system.set_background(0.5, 0.5, 0.5)

        # ── The gate line ─────────────────────────────────────────────────────
        conn.vtl.set_line_name(0, 7, VtlKind.INPUT, name="in_pin7")
        gate = VtlHandle.input(0, 7)

        # ── The patch ─────────────────────────────────────────────────────────
        # Square wave through a hard circular mask: maximum contrast at a sharp
        # edge, so a single frame of it is unmistakable on a photodiode trace.
        patch = conn.stimuli.grating.create_grating(
            position_px=Vec2(0, 0),
            rotation_deg=90.0,  # horizontal stripes
            name="gated_grating",
            params=GratingParams(
                width_px=700, height_px=700, sf_cycles_per_px=0.015, contrast=1.0,
                waveform=GratingTexture.SQR, mask=GratingMask.CIRCLE,
            ),
        )

        add_explanation(conn, EXPLANATION)

        # ── Couple visibility to the line's level ─────────────────────────────
        # Unlike a flash, this animation never completes: every frame it copies
        # the line's level onto the stimulus. `ACTIVE_HIGH` means HIGH shows
        # the patch; pass `ACTIVE_LOW` for an active-low input.
        gated = conn.animations.create_couple_visibility_to_trigger_line(
            gate, patch,
            polarity=VtlPolarity.ACTIVE_HIGH,
            name="gate_on_pin7",
        )
        conn.animations.arm(gated)

        conn.scene_config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}'.")

        # ── Optional: drive the gate from software ────────────────────────────
        # `set_line` on an INPUT handle writes the same bit a DAQ edge would, so
        # the demo is testable with nothing wired up.
        if args.toggle:
            for _ in range(3):
                print("  gate HIGH — patch visible")
                conn.vtl.set_line(gate, True)
                time.sleep(0.7)
                print("  gate LOW  — patch hidden")
                conn.vtl.set_line(gate, False)
                time.sleep(0.7)

if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
