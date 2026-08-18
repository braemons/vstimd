"""Quick visual test for the text stimulus — shows text on screen for 3 seconds."""
import time
from vstimd import Connection
from vstimd.stimuli.stimuli_models import Color, Vec2
from vstimd.stimuli import TextParams

with Connection() as conn:
    # White "Hello vstimd" centred on screen
    h = conn.stimuli.text.create_text(
        position_px=Vec2(0, 50),
        name="demo_text",
        params=TextParams(
            text="Hello vstimd",
            letter_height_px=64,
            text_color=Color(1.0, 1.0, 1.0),
            box_size_px=Vec2(600, 120),
        ),
    )
    print(f"created text handle: {h}")
    time.sleep(2)

    # Change text
    conn.stimuli.text.set_text(h, "Step 6 works!")
    print("updated text")
    time.sleep(2)

    # Change colour to yellow
    conn.stimuli.text.set_text_color(h, Color(1.0, 1.0, 0.0))
    print("changed colour to yellow")
    time.sleep(2)

    conn.stimuli.delete(h)
    print("done")
