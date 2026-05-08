from __future__ import annotations


class WonderlampError(Exception):
    """Base class for all errors returned by wonderlamp_server."""


class HandleNotFoundError(WonderlampError):
    """The stimulus handle does not exist on the server."""


class WrongStimulusTypeError(WonderlampError):
    """The command is not applicable to this stimulus type."""


class WrongTargetError(WonderlampError):
    """A system command was sent with a stimulus handle, or vice versa."""


class CreationFailedError(WonderlampError):
    """The server could not create the stimulus (resource exhaustion, etc.)."""


class InvalidArgumentError(WonderlampError):
    """A field value is out of range or logically invalid."""


class NotSupportedError(WonderlampError):
    """Command exists but is not supported in the current configuration."""


class UnknownServerError(WonderlampError):
    """Unexpected server-side error."""
