from __future__ import annotations

import vstimd.psychopy.visual as visual
from vstimd.stimuli.stimuli_models import Color, Vec2


def label(win: visual.Window, test_id: str, description: str = "") -> int:
    text = f"[{test_id}] {description}".rstrip()
    return win._conn.stimuli.text.create_text(
        text=text, pos=Vec2(0, 260),
        box_width=900, box_height=50,
        letter_height=28,
        color=Color(1.0, 1.0, 0.0),
        anchor="center",
        name="_label",
    )


def update_label(win: visual.Window, handle: int, test_id: str, description: str) -> None:
    win._conn.stimuli.text.set_text(handle, f"[{test_id}] {description}")
