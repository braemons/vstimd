"""Textual widgets for building terminal front ends against a vstimd server.

These are the pieces a control program is made of — what is in the scene, what
the trigger lines are doing, what the server itself reports — as widgets rather
than as one program, so an experiment's own console, the e2e review app, and
anything else can each put them together their own way. They mirror what the
server's on-screen overlay shows, for the times you are on the rig over SSH and
the overlay is on a display you cannot see.

Each widget takes a :class:`~vstimd.Connection` and polls it; none of them owns
one, so a program with several panels can share a single connection — but note
a connection is a single ZMQ socket and is not thread-safe, so give a
background worker its own.

Needs the ``tui`` extra::

    pip install "vstimd-client[tui]"
"""

from .status import ServerStatus
from .stimuli import StimulusList
from .triggers import TriggerLines

__all__ = ["ServerStatus", "StimulusList", "TriggerLines"]
