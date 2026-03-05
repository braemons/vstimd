"""vstim_client — PsychoPy-compatible client for vstim_server.

Usage
-----
    from vstim_client import visual

    # Connect to server on the same host (default)
    win = visual.Window(size=(1920, 1080))

    # Connect to a remote server — provide IP and port via the address parameter
    win = visual.Window(size=(1920, 1080), address='tcp://192.168.1.10:5555')

    circle = visual.Circle(win, radius=50, fillColor='red')
    circle.autoDraw = True
    win.flip()
"""

from . import visual

__all__ = ["visual"]
