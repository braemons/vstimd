"""moving_target.py — Build the `demo_moving_target` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/moving_target.py
    uv run examples/demos/moving_target.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/moving_target.py --save-as my_moving_target -f

Reproduces the shipped `demo_moving_target` config: a target sweeping left to
right at a constant 600 px/s, restarting forever, and pulsing an output line at
the end of every sweep. The two ideas here are motion the server owns (rather
than a Python loop setting positions) and an animation that both repeats itself
and reports each repeat on a pin.

See docs/tutorials/moving-target.md for the walkthrough.
"""

import sys

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection, FinalAction, VtlHandle, VtlKind
from vstimd.stimuli.stimuli_models import Color, Vec2

EXPLANATION = (
    "demo_moving_target — motion plus an output pulse\n"
    "\n"
    "A 30 px target sweeps from x = -800 to x = +800 at 600 px/s, then restarts.\n"
    "Each completed sweep pulses out_pin36 (header pin 36) for one frame —\n"
    "scope that pin against the display to measure end-to-end output latency.\n"
    "Runs on load; no input trigger needed."
)


def main() -> None:
    args = demo_parser(__doc__.splitlines()[0], "my_moving_target").parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)
        conn.system.set_background(0.05, 0.05, 0.05)

        # ── The output line ───────────────────────────────────────────────────
        conn.vtl.set_line_name(0, 36, VtlKind.OUTPUT, name="out_pin36")
        sweep_done = VtlHandle.output(0, 36)

        # ── The target ────────────────────────────────────────────────────────
        # Created at the start of the sweep, so the scene looks right even
        # before the animation runs.
        target = conn.stimuli.shapes.create_circle(
            pos=Vec2(-800, 0), radius=30,
            color=Color(1.0, 1.0, 1.0),
            name="target",
        )

        add_explanation(conn, EXPLANATION)

        # ── The sweep ─────────────────────────────────────────────────────────
        # `move_along_segments_2d` takes waypoints and a speed, and the server
        # converts that into a per-frame step using the *measured* frame rate —
        # so 600 px/s is 600 px/s on a 60 Hz and on a 144 Hz display alike.
        #
        #   RESTART                    — begin the sweep again on completion,
        #                                which is what makes it loop forever
        #   FINAL_ACTION_TRIGGER_LINE  — pulse out_pin36 for one frame at the end
        #                                of each sweep, right after the vblank
        sweep = conn.animations.create_move_along_segments_2d(
            target,
            x=[-800.0, 800.0],
            y=[0.0, 0.0],
            speed_px_per_sec=600.0,
            name="sweep_left_to_right",
            final_action_mask=FinalAction.RESTART | FinalAction.FINAL_ACTION_TRIGGER_LINE,
            final_action_trigger_line=sweep_done,
        )
        # No start_trigger, so arming starts it immediately.
        conn.animations.arm(sweep)

        conn.config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}' — sweeping now, and again on every load.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
