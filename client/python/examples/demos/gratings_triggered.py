"""gratings_triggered.py — Build the `demo_gratings_triggered` scene from scratch.

Usage
-----
    # Server must already be running:
    #   cargo run --release            (or --null for a headless check)

    uv run examples/demos/gratings_triggered.py
    uv run examples/demos/gratings_triggered.py tcp://vstimd-ab12.local:5555
    uv run examples/demos/gratings_triggered.py --save-as my_experiment -f
    uv run examples/demos/gratings_triggered.py --fire           # pulse both inputs

Reproduces the shipped `demo_gratings_triggered` config: two masked gratings,
each hidden until its own input line goes high, each flashed for 2 s, each
re-arming itself afterwards — a complete trial-by-trial setup that runs with no
client connected. Every presentation marks its onset and its end on an output
line, and holds a third line high while it is finished.

`--fire` drives the input lines from software afterwards, which stands in for a
real DAQ edge so the script is useful on a machine with no wiring at all.

See docs/tutorials/gratings-triggers-config.md for the walkthrough.
"""

import json
import sys
import time

from _common import add_explanation, clean_slate, demo_parser

from vstimd import Connection, FinalAction, StartAction, VtlEdge, VtlHandle, VtlKind
from vstimd.stimuli import CircleParams, Color, GratingMask, GratingParams, GratingTexture, ShapeAppearance, Vec2

from vstimd.stimuli.shapes_models import ShapeDrawMode

EXPLANATION = (
    "demo_gratings_triggered — trigger in, trigger out\n"
    "\n"
    "Two masked gratings at the centre, hidden until triggered. Each re-arms itself, so\n"
    "every rising edge fires another 2 s presentation (120 frames @ 60 Hz).\n"
    "\n"
    "in_pin11 -> 45 deg   ·   onset out_pin36   ·   end out_pin37   ·   done out_pin35\n"
    "in_pin12 -> 135 deg  ·   onset out_pin38   ·   end out_pin40   ·   done out_pin32\n"
    "\n"
    "Onset and end pulse for one frame — the marks a recording system timestamps.\n"
    "The done line instead stays HIGH until that grating's next presentation starts."
)

#: The lines this demo uses, as (name, bank, bit, kind). The bit numbers are the
#: Raspberry Pi 5 header pins the shipped `gpiochip-daqd` example maps them to,
#: which is why the names read like pin labels — a VTL line number is not
#: inherently a pin number, the DAQ mapping decides that.
LINES = [
    ("in_pin11",  0, 11, VtlKind.INPUT),
    ("in_pin12",  0, 12, VtlKind.INPUT),
    ("out_pin36", 0, 36, VtlKind.OUTPUT),
    ("out_pin37", 0, 37, VtlKind.OUTPUT),
    ("out_pin38", 0, 38, VtlKind.OUTPUT),
    ("out_pin40", 0, 40, VtlKind.OUTPUT),
    ("out_pin35", 0, 35, VtlKind.OUTPUT),
    ("out_pin32", 0, 32, VtlKind.OUTPUT),
]


def main() -> None:
    parser = demo_parser(__doc__.splitlines()[0], "my_gratings_triggered")
    parser.add_argument(
        "--fire",
        action="store_true",
        help="pulse both input lines from software once the scene is built",
    )
    args = parser.parse_args()

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        clean_slate(conn)
        conn.system.set_background(0.5, 0.5, 0.5)

        # ── 1. Name the trigger lines ─────────────────────────────────────────
        # Naming is optional but pays for itself twice: the overlay and the web
        # UI show the names instead of bare bit numbers, and the names are saved
        # with the config, so the I/O map travels with the scene.
        for name, bank, bit, kind in LINES:
            conn.vtl.set_line_name(bank, bit, kind, name=name)

        in_45,  in_135  = VtlHandle.input(0, 11), VtlHandle.input(0, 12)
        on_45,  on_135  = VtlHandle.output(0, 36), VtlHandle.output(0, 38)
        end_45, end_135 = VtlHandle.output(0, 37), VtlHandle.output(0, 40)
        done_45, done_135 = VtlHandle.output(0, 35), VtlHandle.output(0, 32)

        # ── 2. The two gratings ───────────────────────────────────────────────
        # Identical apart from orientation, so a difference in the response is a
        # difference in orientation tuning and nothing else. Both start hidden —
        # the animation owns their visibility from here on.
        gratings = {}
        for label, angle in (("45deg", 45.0), ("135deg", 135.0)):
            handle = conn.stimuli.grating.create_grating(position=Vec2(0, 0), rotation=angle, # fringe proportion: soft-edged patch
                name=f"grating_{label}", params=GratingParams(width=600, height=600, sf=0.02, contrast=1.0, waveform=GratingTexture.SIN, mask=GratingMask.RAISED_COS, mask_param=0.2))
            conn.stimuli.set_enabled(handle, False)
            gratings[label] = handle

        # ── 3. A fixation dot that stays put ──────────────────────────────────
        # Black core, white ring, so it reads against both the grey background
        # and the grating that appears behind it.
        dot = conn.stimuli.shapes.create_circle(
            position=Vec2(0, 0),
            name="fixation_dot",
            params=CircleParams(
                diameter=12,
                appearance=ShapeAppearance(fill_color=Color(0.0, 0.0, 0.0), outline_color=Color(1.0, 1.0, 1.0)),
            ),
        )
        conn.stimuli.shapes.set_draw_mode(dot, ShapeDrawMode.FILLED_AND_OUTLINED)
        conn.stimuli.shapes.set_outline_color(dot, Color(1.0, 1.0, 1.0))

        add_explanation(conn, EXPLANATION)

        # ── 4. Arm each grating against its input line ────────────────────────
        # This is the whole point of the demo: after this, the device runs
        # trials on its own. No Python is in the loop between the edge arriving
        # and the grating appearing.
        #
        #   start_trigger      — wait for this rising edge before starting
        #   StartAction.ENABLE — show the stimuli when the flash starts
        #   START_ACTION_TRIGGER_LINE  — pulse `start_action_trigger_line`
        #                                (one frame, at onset)
        #   FinalAction.DISABLE        — hide them again when the 120 frames are up
        #   FINAL_ACTION_TRIGGER_LINE  — pulse `final_action_trigger_line` at the end
        #   FinalAction.DONE_LEVEL     — drive `final_action_level_line` HIGH until
        #                                the next presentation starts
        #   FinalAction.REARM          — go back to Armed, so the next edge fires again
        for label, anim_name, trigger, onset, end, done in (
            ("45deg",  "flash_45deg_on_pin11",  in_45,  on_45,  end_45,  done_45),
            ("135deg", "flash_135deg_on_pin12", in_135, on_135, end_135, done_135),
        ):
            anim = conn.animations.create_flash(
                gratings[label],
                duration_frames=120,        # 2 s at 60 Hz — frames, not seconds
                name=anim_name,
                start_trigger=trigger,
                start_edge=VtlEdge.RISING,
                start_action_mask=StartAction.ENABLE | StartAction.START_ACTION_TRIGGER_LINE,
                start_action_trigger_line=onset,
                final_action_mask=(
                    FinalAction.DISABLE
                    | FinalAction.REARM
                    | FinalAction.FINAL_ACTION_TRIGGER_LINE
                    | FinalAction.DONE_LEVEL
                ),
                final_action_trigger_line=end,
                final_action_level_line=done,
            )
            # Created animations are Idle. Arming is what makes them listen.
            conn.animations.arm(anim)

        # ── 5. Persist the whole thing ────────────────────────────────────────
        # The config carries the stimuli, the animations *and* the VTL names, so
        # loading it on a fresh server restores a rig that is ready to trial.
        conn.config.save(args.save_as, overwrite=args.overwrite)
        print(f"Saved as '{args.save_as}'. Configs on the device: "
              f"{', '.join(conn.config.list_configs())}")

        # The shipped demo also turns on the photodiode patch, so a photodiode
        # taped to the corner timestamps the same onsets the pulses mark. That
        # is a scene setting with no command of its own yet — edit it into the
        # saved JSON and push it back, applying it in the same call.
        scene = json.loads(conn.config.retrieve())
        scene["scene"]["photodiode"]["enabled"] = True
        conn.config.upload(args.save_as, json.dumps(scene),
                           overwrite=True, apply_now=True)

        # ── 6. Optional: fire the triggers from software ──────────────────────
        if args.fire:
            print("Firing in_pin11 (45°) …")
            conn.vtl.set_line(in_45, True)      # rising edge
            conn.vtl.set_line(in_45, False)     # the level does not matter after the edge
            time.sleep(2.5)                     # let the 2 s flash finish and re-arm
            print("Firing in_pin12 (135°) …")
            conn.vtl.set_line(in_135, True)
            conn.vtl.set_line(in_135, False)
            time.sleep(2.5)
            print("Both fired. On a wired rig those edges come from the DAQ instead.")

if __name__ == "__main__":
    try:
        main()
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(0)
