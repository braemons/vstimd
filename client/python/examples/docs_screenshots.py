"""docs_screenshots.py — set up each scene the documentation needs a picture of.

Usage
-----
    # Server must already be running, on a display you can see:
    #   cargo run --release -- --windowed 1280x720
    #
    # Put the shots somewhere findable by starting the server with:
    #   VSTIMD_SCREENSHOT_DIR=$HOME/Pictures/vstimd cargo run --release -- --windowed 1280x720

    uv run examples/docs_screenshots.py                  # every shot, in order
    uv run examples/docs_screenshots.py --list           # just show the list
    uv run examples/docs_screenshots.py --only grating-masks dots-figure-ground
    uv run examples/docs_screenshots.py --skip overlay   # stimuli only

This script builds the scene; **you** press F12 (or PrintScreen) in the vstimd
window to save it. vstimd writes the PNG itself, out of its own swapchain, so
the file is the frame that was actually drawn rather than a capture of a window
that happened to be on top — and the same keypress works on a bare-metal rig
where no screenshot tool exists at all.

Each shot prints the filename to rename the result to, so a run leaves you with
a directory you can sort out afterwards rather than a pile of timestamps.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import replace
from typing import Callable

from vstimd import Connection, FinalAction, StartAction, VtlHandle, VtlKind
from vstimd.stimuli import (
    Aperture,
    ApertureClip,
    ApertureShape,
    CircleParams,
    Color,
    DotsParams,
    EllipseParams,
    GratingMask,
    GratingParams,
    GratingTexture,
    NoiseRule,
    RectParams,
    ShapeAppearance,
    ShapeDrawMode,
    TextParams,
    Vec2,
)

# Mid grey unless a shot says otherwise: a stimulus is easier to judge against
# the background it is meant to sit on than against black.
GREY = (0.5, 0.5, 0.5)
DARK = (0.05, 0.05, 0.05)

WHITE = Color(1.0, 1.0, 1.0)
BLACK = Color(0.0, 0.0, 0.0)


def caption(conn: Connection, text: str) -> None:
    """A label across the bottom, so a shot is self-describing in the docs."""
    conn.stimuli.text.create_text(
        name="caption",
        position_px=Vec2(0, -300),
        params=TextParams(
            text=text,
            letter_height_px=22,
            text_color=Color(0.9, 0.9, 0.9),
            fill_color=Color(0.0, 0.0, 0.0, 0.6),
            box_size_px=Vec2(1200, 60),
        ),
    )


def label(conn: Connection, text: str, at: Vec2) -> None:
    """A small caption under one item in a row of variants."""
    conn.stimuli.text.create_text(
        position_px=at,
        params=TextParams(
            text=text,
            letter_height_px=18,
            text_color=Color(0.95, 0.95, 0.95),
            box_size_px=Vec2(260, 30),
        ),
    )


# ── The shots ─────────────────────────────────────────────────────────────────
#
# Each builder gets a clean scene and leaves it ready to photograph.


def shapes_overview(conn: Connection) -> None:
    conn.system.set_background(*DARK)
    fill = ShapeAppearance(fill_color=Color(0.85, 0.2, 0.2))
    conn.stimuli.shapes.create_rect(
        name="rect", position_px=Vec2(-380, 60),
        params=RectParams(width_px=220, height_px=150, appearance=fill),
    )
    conn.stimuli.shapes.create_circle(
        name="circle", position_px=Vec2(-90, 60),
        params=CircleParams(diameter_px=180, appearance=fill),
    )
    conn.stimuli.shapes.create_ellipse(
        name="ellipse", position_px=Vec2(190, 60),
        params=EllipseParams(width_px=240, height_px=140, appearance=fill),
    )
    # A rect rotated about its own centre — the thing `rotation_deg` means.
    conn.stimuli.shapes.create_rect(
        name="rotated", position_px=Vec2(450, 60), rotation_deg=30.0,
        params=RectParams(width_px=180, height_px=120, appearance=fill),
    )
    for text, x in (("Rect", -380), ("Circle", -90), ("Ellipse", 190), ("rotation_deg=30", 450)):
        label(conn, text, Vec2(x, -60))
    caption(conn, "Shapes — every size is a full extent, never a half-extent")


def shape_draw_modes(conn: Connection) -> None:
    conn.system.set_background(*DARK)
    modes = [
        (ShapeDrawMode.FILLED, "FILLED", -320),
        (ShapeDrawMode.OUTLINED, "OUTLINED", 0),
        (ShapeDrawMode.FILLED_AND_OUTLINED, "FILLED_AND_OUTLINED", 320),
    ]
    for mode, text, x in modes:
        conn.stimuli.shapes.create_circle(
            name=f"mode_{text.lower()}", position_px=Vec2(x, 40),
            params=CircleParams(
                diameter_px=200,
                appearance=ShapeAppearance(
                    fill_color=Color(0.2, 0.45, 0.8),
                    outline_color=Color(1.0, 0.85, 0.2),
                    outline_width_px=6.0,
                    draw_mode=mode,
                ),
            ),
        )
        label(conn, text, Vec2(x, -95))
    caption(conn, "draw_mode turns an outline on and off — not outline_width_px")


def grating_masks(conn: Connection) -> None:
    conn.system.set_background(*GREY)
    masks = [
        (GratingMask.NONE, "NONE", -480),
        (GratingMask.CIRCLE, "CIRCLE", -240),
        (GratingMask.GAUSS, "GAUSS", 0),
        (GratingMask.RAISED_COS, "RAISED_COS", 240),
        (GratingMask.HANN, "HANN", 480),
    ]
    for mask, text, x in masks:
        conn.stimuli.grating.create_grating(
            name=f"mask_{text.lower()}", position_px=Vec2(x, 60),
            params=GratingParams(
                width_px=210, height_px=210,
                sf_cycles_per_px=0.02, contrast=1.0,
                waveform=GratingTexture.SIN, mask=mask,
            ),
        )
        label(conn, text, Vec2(x, -80))
    caption(conn, "Aperture masks — a tapered edge stops the patch being its own stimulus")


def grating_waveforms(conn: Connection) -> None:
    conn.system.set_background(*GREY)
    for wave, text, x in (
        (GratingTexture.SIN, "SIN", -480),
        (GratingTexture.SQR, "SQR", -160),
        (GratingTexture.SAW, "SAW", 160),
        (GratingTexture.TRI, "TRI", 480),
    ):
        conn.stimuli.grating.create_grating(
            name=f"wave_{text.lower()}", position_px=Vec2(x, 60),
            params=GratingParams(
                width_px=260, height_px=260,
                sf_cycles_per_px=0.012, contrast=1.0,
                waveform=wave, mask=GratingMask.CIRCLE,
            ),
        )
        label(conn, text, Vec2(x, -100))
    caption(conn, "Carrier waveforms at the same spatial frequency")


def grating_drifting(conn: Connection) -> None:
    """The hero grating: a drifting Gabor, which is what the demo config shows."""
    conn.system.set_background(*GREY)
    conn.stimuli.grating.create_grating(
        name="drifting_gabor", position_px=Vec2(0, 30), rotation_deg=0.0,
        params=GratingParams(
            width_px=420, height_px=420,
            sf_cycles_per_px=0.01, contrast=1.0,
            waveform=GratingTexture.SIN, mask=GratingMask.GAUSS,
            drift_speed_hz=4.0,
        ),
    )
    caption(conn, "Gabor, 0.01 cyc/px, drifting 4 cyc/s — the server owns the motion")


def text_anchors(conn: Connection) -> None:
    conn.system.set_background(*DARK)
    conn.stimuli.text.create_text(
        name="anchor_center", position_px=Vec2(0, 120),
        params=TextParams(
            text="anchor='center' with a backing box",
            letter_height_px=30, anchor="center",
            text_color=WHITE, fill_color=Color(0.0, 0.0, 0.0, 0.65),
            border_color=Color(0.4, 0.6, 1.0), box_size_px=Vec2(700, 70),
        ),
    )
    conn.stimuli.text.create_text(
        name="anchor_topleft", position_px=Vec2(-560, -40),
        params=TextParams(
            text="anchor='top-left' — pinned to a corner\nregardless of string length",
            letter_height_px=24, anchor="top-left", text_color=Color(0.95, 0.85, 0.3),
        ),
    )
    caption(conn, "Text — pos_px places the box, anchor says which of its points lands there")


def dots_classic(conn: Connection) -> None:
    """A textbook coherence RDK: aperture the size of the field, hard edge visible."""
    conn.system.set_background(*DARK)
    conn.stimuli.dots.create_dots(
        name="classic_rdk", position_px=Vec2(0, 40),
        params=DotsParams(
            field_width_px=460, field_height_px=460,
            aperture=Aperture(
                shape=ApertureShape.CIRCLE, width_px=460, height_px=460,
                clip=ApertureClip.PIXEL,   # the aperture is meant to be seen here
            ),
            dot_count=250, dot_size_px=6.0,
            direction_deg=0.0, speed_px_per_s=140.0, coherence=0.6,
            noise_rule=NoiseRule.DIRECTION, seed=7,
        ),
    )
    caption(conn, "Classic RDK — 60% coherence, PIXEL clipping so the window edge is crisp")


def dots_coherence(conn: Connection) -> None:
    conn.system.set_background(*DARK)
    common = DotsParams(
        field_width_px=300, field_height_px=300,
        aperture=Aperture(shape=ApertureShape.CIRCLE, width_px=300, height_px=300,
                          clip=ApertureClip.PIXEL),
        dot_count=160, dot_size_px=6.0,
        direction_deg=0.0, speed_px_per_s=140.0,
        noise_rule=NoiseRule.DIRECTION,
    )
    for coh, x in ((0.0, -400), (0.5, 0), (1.0, 400)):
        conn.stimuli.dots.create_dots(
            name=f"coh_{int(coh * 100)}", position_px=Vec2(x, 60),
            params=replace(common, coherence=coh, seed=int(coh * 100) + 1),
        )
        label(conn, f"coherence = {coh:.1f}", Vec2(x, -125))
    caption(conn, "Coherence is a per-dot Bernoulli, so the signal count varies frame to frame")


def dots_figure_ground(conn: Connection) -> None:
    """The whole point of separating field from aperture."""
    conn.system.set_background(*GREY)
    circle = Aperture(
        shape=ApertureShape.CIRCLE, width_px=380.0, height_px=380.0,
        clip=ApertureClip.DOT_CENTER,   # never PIXEL here: a cut edge is a static cue
    )
    common = DotsParams(
        field_width_px=1280.0, field_height_px=720.0,
        dot_count=700, dot_size_px=6.0, dot_color=WHITE,
        speed_px_per_s=200.0, coherence=1.0,
    )
    conn.stimuli.dots.create_dots(
        name="ground",
        params=replace(common, aperture=replace(circle, invert=True),
                       direction_deg=0.0, seed=1),
    )
    conn.stimuli.dots.create_dots(
        name="figure",
        params=replace(common, aperture=circle, direction_deg=90.0, seed=2),
    )
    caption(conn, "Figure-ground RDK — freeze this frame and the circle vanishes")


def dots_clipping(conn: Connection) -> None:
    """Side by side, the difference that decides whether a figure has an outline."""
    conn.system.set_background(*DARK)
    common = DotsParams(
        field_width_px=340, field_height_px=340,
        dot_count=140, dot_size_px=16.0,   # big dots: the clip mode is the point
        direction_deg=0.0, speed_px_per_s=60.0, coherence=1.0, seed=3,
    )
    for clip, text, x in ((ApertureClip.DOT_CENTER, "DOT_CENTER", -260),
                          (ApertureClip.PIXEL, "PIXEL", 260)):
        conn.stimuli.dots.create_dots(
            name=f"clip_{text.lower()}", position_px=Vec2(x, 60),
            params=replace(common, aperture=Aperture(
                shape=ApertureShape.CIRCLE, width_px=280, height_px=280, clip=clip)),
        )
        label(conn, text, Vec2(x, -130))
    caption(conn, "DOT_CENTER lets dots overhang; PIXEL cuts them, drawing the aperture")


def overlay_scene(conn: Connection) -> None:
    """A scene with enough in it that every overlay panel has something to show.

    An empty Stimuli panel or an empty Animations table makes a useless
    screenshot, so this populates stimuli, animations and named VTL lines
    before you start cycling through F1-F7.
    """
    conn.system.set_background(*GREY)

    conn.vtl.set_line_name(0, 11, VtlKind.INPUT, name="trial_start")
    conn.vtl.set_line_name(0, 12, VtlKind.INPUT, name="reward")
    conn.vtl.set_line_name(0, 36, VtlKind.OUTPUT, name="stim_onset")
    conn.vtl.set_line_name(0, 37, VtlKind.OUTPUT, name="stim_offset")

    fixation = conn.stimuli.shapes.create_circle(
        name="fixation_dot", position_px=Vec2(0, 0),
        params=CircleParams(diameter_px=20, appearance=ShapeAppearance(fill_color=WHITE)),
    )
    grating = conn.stimuli.grating.create_grating(
        name="probe_grating", position_px=Vec2(-300, 0), rotation_deg=45.0,
        params=GratingParams(width_px=300, height_px=300, sf_cycles_per_px=0.015,
                             contrast=0.8, mask=GratingMask.GAUSS, drift_speed_hz=2.0),
    )
    target = conn.stimuli.shapes.create_rect(
        name="response_target", position_px=Vec2(300, 0),
        params=RectParams(width_px=120, height_px=120,
                          appearance=ShapeAppearance(fill_color=Color(0.2, 0.8, 0.3))),
    )
    conn.stimuli.set_enabled(target, False)

    flash = conn.animations.create_flash(
        target, duration_frames=120, name="target_flash",
        start_trigger=VtlHandle.named("trial_start", VtlKind.INPUT),
        start_action_mask=StartAction.ENABLE | StartAction.START_ACTION_TRIGGER_LINE,
        start_action_trigger_line=VtlHandle.named("stim_onset", VtlKind.OUTPUT),
        final_action_mask=FinalAction.DISABLE | FinalAction.REARM
        | FinalAction.FINAL_ACTION_TRIGGER_LINE,
        final_action_trigger_line=VtlHandle.named("stim_offset", VtlKind.OUTPUT),
    )
    conn.animations.arm(flash)

    flicker = conn.animations.create_flicker(
        fixation, on_frames=30, off_frames=30, name="fixation_blink",
    )
    conn.animations.arm(flicker)

    # Named so the Stimuli panel reads as a real experiment rather than a demo.
    _ = grating
    caption(conn, "Overlay — F1 Stimuli · F2 Log · F3 VTL · F4 Animations · F5 System"
                  " · F6 Scene-config · F7 Benchmarks")


Shot = tuple[str, str, Callable[[Connection], None], str]

SHOTS: list[Shot] = [
    ("shapes-overview", "shapes/shapes.md", shapes_overview,
     "the four geometries"),
    ("shape-draw-modes", "shapes/draw-modes.md", shape_draw_modes,
     "fill / outline / both"),
    ("grating-masks", "stimuli/gratings.md", grating_masks,
     "all five aperture masks"),
    ("grating-waveforms", "stimuli/gratings.md", grating_waveforms,
     "all four carriers"),
    ("grating-drifting", "tutorials/drifting-grating.md", grating_drifting,
     "the hero Gabor — it is drifting, so any frame will do"),
    ("text-anchors", "stimuli/text.md", text_anchors,
     "box, border and two anchors"),
    ("dots-classic", "stimuli/random-dots.md", dots_classic,
     "a textbook coherence RDK"),
    ("dots-coherence", "stimuli/random-dots.md", dots_coherence,
     "0%, 50%, 100% side by side"),
    ("dots-figure-ground", "tutorials/figure-ground-rdk.md", dots_figure_ground,
     "let it run a second first — the circle only exists in motion"),
    ("dots-clipping", "stimuli/random-dots.md", dots_clipping,
     "DOT_CENTER vs PIXEL, with deliberately large dots"),
    ("overlay", "concepts/ + developer/", overlay_scene,
     "press F1-F7 and shoot each panel; Shift+Fn hides one, backtick hides all"),
]


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("address", nargs="?", default="tcp://localhost:5555",
                        help="ZMQ address of the server (default: %(default)s)")
    parser.add_argument("--list", action="store_true", help="list the shots and exit")
    parser.add_argument("--only", nargs="+", metavar="SLUG", help="run only these shots")
    parser.add_argument("--skip", nargs="+", metavar="SLUG", help="skip these shots")
    args = parser.parse_args()

    shots = SHOTS
    if args.only:
        unknown = set(args.only) - {s[0] for s in SHOTS}
        if unknown:
            print(f"unknown shot(s): {', '.join(sorted(unknown))}", file=sys.stderr)
            return 2
        shots = [s for s in SHOTS if s[0] in set(args.only)]
    if args.skip:
        shots = [s for s in shots if s[0] not in set(args.skip)]

    if args.list:
        for slug, page, _, hint in SHOTS:
            print(f"  {slug:22} {page:34} {hint}")
        return 0

    print(f"Connecting to {args.address} …")
    with Connection(args.address) as conn:
        conn.wait_until_ready(timeout_s=15)
        info = conn.system.query_server_info()
        print(f"{info.width_px}x{info.height_px} @ {info.frame_rate_hz:.1f} Hz, "
              f"vstimd {info.version}\n")
        print("Press F12 (or PrintScreen) in the vstimd window to save each shot.\n"
              "Files land in $VSTIMD_SCREENSHOT_DIR (or the server's working directory).\n")

        for n, (slug, page, build, hint) in enumerate(shots, 1):
            conn.system.clear_all()
            build(conn)
            print(f"[{n}/{len(shots)}] {slug}")
            print(f"        for: {page}")
            print(f"        {hint}")
            print(f"        → rename the PNG to {slug}.png")
            try:
                input("        F12 to capture, then Enter for the next shot… ")
            except (EOFError, KeyboardInterrupt):
                print("\nStopped.")
                return 0
            print()

        print("Done. The scene is left as the last shot built it.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
