from __future__ import annotations

import vstimd.psychopy.visual as visual
from vstimd.stimuli.stimuli_models import Color, Vec2
from vstimd.stimuli import TextParams


def label(win: visual.Window, test_id: str, description: str = "") -> int:
    text = f"[{test_id}] {description}".rstrip()
    return win._conn.stimuli.text.create_text(
        position_px=Vec2(0, 260),
        name="_label",
        params=TextParams(
            text=text,
            letter_height_px=28,
            text_color=Color(1.0, 1.0, 0.0),
            anchor="center",
            box_size_px=Vec2(900, 200),
        ),
    )


def update_label(win: visual.Window, handle: int, test_id: str, description: str) -> None:
    win._conn.stimuli.text.set_text(handle, f"[{test_id}] {description}")
