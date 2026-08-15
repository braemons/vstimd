"""Shared scaffolding for the demo-building example scripts.

Every ``examples/demos/*.py`` script builds one of the shipped demo scenes
(``server/config/demos/vstimd_demo_*.config.json``) from an empty scene, then
saves it under a name of your own. The three things they all need live here so
each script stays about the demo it builds:

* :func:`demo_parser`   — the common command line (address, ``--save-as``, ``-f``)
* :func:`clean_slate`   — clear stimuli, animations, and VTL names
* :func:`add_explanation` — the caption box every demo puts at the bottom
"""

from __future__ import annotations

import argparse

from vstimd import Connection, StimulusHandle
from vstimd.stimuli.stimuli_models import Color, Vec2

#: Where the caption box sits, and how big it is. Same in every demo.
EXPLANATION_POS = Vec2(0, -340)
EXPLANATION_BOX = (1500.0, 320.0)


def demo_parser(description: str, default_name: str) -> argparse.ArgumentParser:
    """The command line shared by every demo-building script."""
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument(
        "address",
        nargs="?",
        default="tcp://localhost:5555",
        help="ZMQ address of the server (default: tcp://localhost:5555)",
    )
    parser.add_argument(
        "--save-as",
        default=default_name,
        metavar="NAME",
        help=f"config name to save the finished scene under (default: {default_name})",
    )
    parser.add_argument(
        "-f", "--overwrite",
        action="store_true",
        help="overwrite the config if that name already exists",
    )
    return parser


def clean_slate(conn: Connection) -> None:
    """Empty the scene so the script builds on nothing.

    ``delete_all`` removes the stimuli but **not** the animations — an animation
    outlives the stimuli it drives — and it does not touch the VTL name map
    either. Clearing all three is what makes these scripts reproducible: run one
    twice and you get the same scene, not the same scene twice over.
    """
    conn.system.delete_all()
    for anim in conn.animations.list_animations():
        conn.animations.delete(anim.handle)
    for line in conn.vtl.list_lines():
        conn.vtl.set_line_name(line.bank, line.bit, line.kind, name="")


def add_explanation(conn: Connection, text: str) -> StimulusHandle:
    """Add the on-screen caption every demo carries.

    A demo has to explain itself on a rig with no client attached, so each one
    ends with a dim text box across the bottom of the frame. Nothing about it is
    special — it is an ordinary text stimulus.
    """
    return conn.stimuli.text.create_text(
        text=text,
        pos=EXPLANATION_POS,
        box_width=EXPLANATION_BOX[0],
        box_height=EXPLANATION_BOX[1],
        letter_height=24,
        color=Color(0.9, 0.9, 0.9),
        fill_color=Color(0.0, 0.0, 0.0, 0.65),
        name="explanation",
    )
