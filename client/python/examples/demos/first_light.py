"""first_light.py — Build the `demo_first_light` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/first_light.py
    uv run examples/demos/first_light.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/first_light.py --save-as my_first_light -f

Reproduces the shipped `demo_first_light` config: a title, a centre dot, and a
40 px square in each corner, so one glance tells you the whole display is being
driven. No animations, no triggers — the smallest complete scene there is.

See docs/tutorials/first-light.md for the walkthrough.
"""

import sys

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection
from vstimd.stimuli.stimuli_models import Color, Vec2

EXPLANATION = (
    "demo_first_light — the display works\n"
    "\n"
    "Static scene: a centre dot and four corner squares (1800 x 960 px apart),\n"
    "so you can see at a glance that the whole display is being driven.\n"
    "No triggers, no animations.\n"
    "Next: load demo_drifting_grating, or list all demos with config list."
)

#: Corner squares, as (name, x, y): 80 x 80 px each, their centres 1800 x 960 px
#: apart. That leaves a comfortable margin inside a 1920 x 1080 frame, so a square
#: that is clipped means the display is not showing you the whole frame.
CORNERS = [
    ("corner_tl", -900.0,  480.0),
    ("corner_tr",  900.0,  480.0),
    ("corner_bl", -900.0, -480.0),
    ("corner_br",  900.0, -480.0),
]


def main() -> None:
    args = demo_parser(__doc__.splitlines()[0], "my_first_light").parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)

        # ── Background ────────────────────────────────────────────────────────
        # Near-black, so the white marks carry all the contrast.
        conn.system.set_background(0.05, 0.05, 0.05)

        # ── Title ─────────────────────────────────────────────────────────────
        conn.stimuli.text.create_text(
            text="vstimd — first light",
            pos=Vec2(0, 220),
            box_width=1600, box_height=120,
            letter_height=80,
            color=Color(1.0, 1.0, 1.0),
            name="title",
        )

        # ── Centre dot ────────────────────────────────────────────────────────
        # Slightly above centre so it does not collide with the title's descenders.
        conn.stimuli.shapes.create_circle(
            pos=Vec2(0, 60), radius=8,
            color=Color(1.0, 1.0, 1.0),
            name="fixation_dot",
        )

        # ── Corner squares ────────────────────────────────────────────────────
        for name, x, y in CORNERS:
            conn.stimuli.shapes.create_rect(
                pos=Vec2(x, y), width=80, height=80,
                color=Color(1.0, 1.0, 1.0),
                name=name,
            )

        add_explanation(conn, EXPLANATION)

        # ── Persist ───────────────────────────────────────────────────────────
        conn.config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}' — load it again with "
              f"conn.config.load('{args.save_as}')")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
