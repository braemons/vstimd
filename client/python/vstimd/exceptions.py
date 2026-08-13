"""Exceptions raised when the server refuses a command.

Every RPC goes through :meth:`vstimd.Connection._send`, which turns a non-OK
:class:`~vstimd.response.ErrorCode` into one of the exceptions below. Nothing
returns an error code to the caller, so a script that does not catch anything
still cannot silently carry on after a failed command.

The exceptions form a shallow tree, so you can catch a family or a specific
failure::

    try:
        conn.config.load("gratings")
    except ConfigNotFoundError:
        conn.config.save("gratings")     # first run on this rig
    except ConfigError as exc:
        log.error("config %s unusable: %s", exc.detail, exc.code.name)

Each one carries the machine-readable :attr:`~VstimdError.code`, the server's
own message as :attr:`~VstimdError.detail`, and — when the server was answering
a specific request — which :attr:`~VstimdError.command` failed and which
:attr:`~VstimdError.handle` it addressed.
"""
from __future__ import annotations

from vstimd.response import ErrorCode

# Populated by VstimdError.__init_subclass__ below.
_BY_CODE: dict[ErrorCode, type["VstimdError"]] = {}


class VstimdError(Exception):
    """Base class for all errors reported by vstimd.

    Parameters
    ----------
    detail:
        The server's own description of what went wrong.
    code:
        The :class:`~vstimd.response.ErrorCode` the server returned. Defaults
        to the subclass's own code, so ``HandleNotFoundError("gone").code`` is
        ``ErrorCode.HANDLE_NOT_FOUND`` without having to say so.
    command:
        Name of the request that failed, e.g. ``"set_circle_radius"``.
    handle:
        Stimulus handle the request addressed, or ``None`` for scene-wide
        commands.
    """

    code: ErrorCode | None = None
    """The code this class represents, overridden per instance by the code the
    server actually sent. ``None`` on the grouping base classes."""

    def __init__(
        self,
        detail: str = "",
        *,
        code: ErrorCode | None = None,
        command: str | None = None,
        handle: int | None = None,
    ) -> None:
        self.detail = detail
        self.code = code if code is not None else type(self).code
        self.command = command
        self.handle = handle
        super().__init__(self._message())

    def __init_subclass__(cls, **kwargs: object) -> None:
        # Registering here is what keeps the code→exception mapping honest: a
        # new error code cannot be added to the enum and then forgotten in a
        # table somewhere else. The first class to claim a code keeps it, so
        # user subclasses of these do not steal the mapping.
        super().__init_subclass__(**kwargs)
        if cls.code is not None:
            _BY_CODE.setdefault(cls.code, cls)

    def _message(self) -> str:
        text = self.detail or (
            f"server error {self.code.name}" if self.code else "server error"
        )
        context = []
        if self.command:
            context.append(self.command)
        if self.handle is not None:
            context.append(f"handle {self.handle}")
        return f"{text} ({', '.join(context)})" if context else text


# ── Client-side failures ──────────────────────────────────────────────────────


class ProtocolError(VstimdError):
    """The server's reply could not be understood.

    A truncated frame, a message from something that is not vstimd, or a reply
    with no result code set. Distinct from :class:`UnknownServerError`, which
    is the server correctly reporting that *it* hit something unexpected.
    """

    code = ErrorCode.UNSPECIFIED


# ── Addressing a stimulus ─────────────────────────────────────────────────────


class StimulusError(VstimdError):
    """Something is wrong with the stimulus a command was addressed to."""


class HandleNotFoundError(StimulusError):
    """The stimulus handle does not exist on the server."""

    code = ErrorCode.HANDLE_NOT_FOUND


class WrongStimulusTypeError(StimulusError):
    """The command is not applicable to this stimulus type."""

    code = ErrorCode.WRONG_STIMULUS_TYPE


class WrongTargetError(StimulusError):
    """A system command was sent with a stimulus handle, or vice versa."""

    code = ErrorCode.WRONG_TARGET


# ── Command rejected ──────────────────────────────────────────────────────────


class CreationFailedError(VstimdError):
    """The server could not create the stimulus (resource exhaustion, etc.)."""

    code = ErrorCode.CREATION_FAILED


class InvalidArgumentError(VstimdError):
    """A field value is out of range or logically invalid."""

    code = ErrorCode.INVALID_ARGUMENT


class NotSupportedError(VstimdError):
    """Command exists but is not supported in the current configuration."""

    code = ErrorCode.NOT_SUPPORTED


class NotReadyError(VstimdError):
    """Server is still initialising; retry after the first rendered frame."""

    code = ErrorCode.NOT_READY


class UnknownServerError(VstimdError):
    """Unexpected server-side error.

    Also raised for an error code this client does not recognise, which is what
    a newer server talking to an older client looks like.
    """

    code = ErrorCode.UNKNOWN


# ── Scene configs ─────────────────────────────────────────────────────────────


class ConfigError(VstimdError):
    """Something went wrong loading, saving, or parsing a scene config."""


class ConfigNotFoundError(ConfigError):
    """Named config does not exist in the server's config directory."""

    code = ErrorCode.FILE_NOT_FOUND


class ConfigIoError(ConfigError):
    """Filesystem error while reading or writing a config file."""

    code = ErrorCode.FILE_IO


class ConfigFormatError(ConfigError):
    """Config file contains invalid JSON or does not match the expected schema."""

    code = ErrorCode.FILE_FORMAT


class ConfigVersionError(ConfigError):
    """Config file version is not supported by this server."""

    code = ErrorCode.UNSUPPORTED_VERSION


class ConfigAlreadyExistsError(ConfigError):
    """Config already exists and overwrite was not requested."""

    code = ErrorCode.FILE_ALREADY_EXISTS


# ── Code → exception ──────────────────────────────────────────────────────────


def exception_type_for(code: int) -> type[VstimdError]:
    """Return the exception class representing *code*.

    Falls back to :class:`UnknownServerError` for a code this client has never
    heard of, so an older client keeps working against a newer server.
    """
    return _BY_CODE.get(ErrorCode(code), UnknownServerError)


def error_for_code(
    code: int,
    detail: str = "",
    *,
    command: str | None = None,
    handle: int | None = None,
) -> VstimdError:
    """Build the exception for a server error code.

    The returned exception keeps the numeric *code* even when it is one this
    client does not know, so a bug report can quote it.
    """
    try:
        parsed = ErrorCode(code)
    except ValueError:  # pragma: no cover - ErrorCode._missing_ handles this
        parsed = ErrorCode.UNKNOWN
    detail = detail or f"server error code {int(code)}"
    return exception_type_for(code)(
        detail, code=parsed, command=command, handle=handle
    )
