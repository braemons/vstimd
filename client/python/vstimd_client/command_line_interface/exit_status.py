"""Exit statuses returned by ``vstimctl``.

The same numbers as every ``<name>ctl`` in the family
(``contracts/DAEMON_LAYOUT.md``), so that a script branches on *why* a command
failed without parsing stderr — "the rig is off" (``UNAVAILABLE``) and "the rig
said no" (``REFUSED``) call for different reactions in an experiment runner.

``USAGE`` is 2 because :mod:`argparse` hard-codes 2 for command-line errors and
there is no way to talk it out of that. ``INTERRUPTED`` is 130 by the shell
convention of 128 + SIGINT.
"""
from __future__ import annotations

from enum import IntEnum


class ExitStatus(IntEnum):
    """Process exit status of the ``vstimctl`` command."""

    OK = 0
    """The command did what it was asked to."""

    FAILURE = 1
    """Something went wrong that no other status describes."""

    USAGE = 2
    """The command line itself was wrong."""

    UNAVAILABLE = 3
    """Nothing answered within ``--timeout``: a rig that is off, or a wrong address."""

    TIMED_OUT = 4
    """The server answered, and a call did not finish in time."""

    REFUSED = 5
    """The server answered, and the answer was an error."""

    NOT_FOUND = 6
    """What was asked for does not exist: no rigs discovered, no such scene-config."""

    INTERRUPTED = 130
    """Ctrl-C."""
