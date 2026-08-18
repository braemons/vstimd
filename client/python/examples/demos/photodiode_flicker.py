"""photodiode_flicker.py — Build the `demo_photodiode_flicker` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/photodiode_flicker.py
    uv run examples/demos/photodiode_flicker.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/photodiode_flicker.py --save-as my_timing_check -f

Reproduces the shipped `demo_photodiode_flicker` config: the corner photodiode
patch inverting on every single frame, plus a large patch flickering at 5 Hz as
a visible cross-check. This is the scene you load to measure what the display is
actually doing, rather than what it claims to do.

See docs/tutorials/photodiode-flicker.md for the walkthrough.
"""

import json
import sys

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection, StartAction
from vstimd.stimuli.stimuli_models import Color, Vec2
from vstimd.stimuli import RectParams, ShapeAppearance

EXPLANATION = (
    "demo_photodiode_flicker — frame timing\n"
    "\n"
    "The photodiode patch (bottom-left corner) inverts on every single frame,\n"
    "so a photodiode on that corner reports the real refresh rate.\n"
    "The big white patch flickers at 5 Hz (6 frames on / 6 off at 60 Hz) as a\n"
    "visible cross-check. Both run on load and never stop; no triggers used."
)


def main() -> None:
    args = demo_parser(__doc__.splitlines()[0], "my_photodiode_flicker").parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)
        conn.system.set_background(0.05, 0.05, 0.05)

        # ── The visible patch ─────────────────────────────────────────────────
        # Big and white: you want to be able to see the flicker from across the
        # room, and a photometer wants a lot of it.
        patch = conn.stimuli.shapes.create_rect(
            position=Vec2(0, 100),
            name="flicker_patch",
            params=RectParams(
                width=1400, height=600,
                appearance=ShapeAppearance(fill_color=Color(1.0, 1.0, 1.0)),
            ),
        )

        add_explanation(conn, EXPLANATION)

        # ── The 5 Hz flicker ──────────────────────────────────────────────────
        # Counted in frames, not milliseconds: 6 on + 6 off is one 12-frame
        # period, which is 5 Hz at 60 Hz and something else at another rate.
        # Omitting `total_frames` means it never stops. StartAction.ENABLE makes
        # the first on-phase actually show the patch rather than assuming it is
        # already visible.
        flicker = conn.animations.create_flicker(
            patch,
            on_frames=6, off_frames=6,
            start_on_phase=True,
            name="field_flicker_5hz",
            start_action_mask=StartAction.ENABLE,
        )
        conn.animations.arm(flicker)

        conn.config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}'.")

        # ── The corner photodiode patch ───────────────────────────────────────
        # The per-frame inverting patch is a scene setting, not a stimulus, and
        # it has no command of its own yet — so set it by editing the retrieved
        # JSON and uploading it back with `apply_now`. `flicker` is what makes
        # it invert every frame; without it the patch is a static square.
        scene = json.loads(conn.config.retrieve())
        scene["scene"]["photodiode"]["enabled"] = True
        scene["scene"]["photodiode"]["flicker"] = True
        conn.config.upload(args.save_as, json.dumps(scene),
                           overwrite=True, apply_now=True)
        print("Photodiode patch on and inverting every frame.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
