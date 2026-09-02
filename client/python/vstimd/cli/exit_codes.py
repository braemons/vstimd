"""Exit codes returned by ``vstimd-client``.

The codes are narrow enough that a script can branch on *why* a command failed
without parsing stderr — "the rig is off" (``UNAVAILABLE``) and "the rig said
no" (``SERVER_ERROR``) call for different reactions in an experiment runner.

``USAGE`` is 2 because :mod:`argparse` hard-codes 2 for command-line errors and
there is no way to talk it out of that; the rest are chosen not to collide.
``INTERRUPTED`` is 130 by the shell convention of 128 + SIGINT.
"""
from __future__ import annotations

from enum import IntEnum


class ExitCode(IntEnum):
    """Process exit status of the ``vstimd-client`` command."""

    OK = 0
    """The command did what it was asked to."""

    FAILURE = 1
    """Something went wrong that no other code describes."""

    USAGE = 2
    """The command line itself was wrong, or no command was given."""

    UNAVAILABLE = 3
    """The server could not be reached — bad address, or nothing listening."""

    TIMEOUT = 4
    """The server did not answer within ``--timeout`` seconds."""

    SERVER_ERROR = 5
    """The server answered, and the answer was an error."""

    NOT_FOUND = 6
    """What was asked for does not exist: no rigs discovered, no such scene-config."""

    NO_BACKEND = 7
    """``discover`` has no mDNS implementation available on this machine."""

    INTERRUPTED = 130
    """Ctrl-C."""
