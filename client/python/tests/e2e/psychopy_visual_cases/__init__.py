import pytest

import vstimd.psychopy.visual as visual

from .test_circle import *   # noqa: F401, F403
from .test_grating import *  # noqa: F401, F403
from .test_rect import *     # noqa: F401, F403
from .test_text import *     # noqa: F401, F403


@pytest.fixture(autouse=True)
def flush_window(win: visual.Window):
    """Send whatever the test left staged before the scene is cleared.

    A PsychoPy Window is deferred: `autoDraw = False` at the end of a test only
    queues a set_enabled, which the next `flip()` sends — and by then the
    per-test scene reset has deleted the stimulus it names, so the next test
    fails on a handle that was never its own.
    """
    yield
    win.flip()
