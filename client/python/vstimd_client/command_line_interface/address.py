"""Turning what people type into a ZMQ endpoint.

``zmq_connect`` accepts only a full endpoint, and rejects anything else with
``Invalid argument`` and no indication of what was wrong. Since every rig is
reached over TCP on a well-known port, ``10.0.1.42``, ``10.0.1.42:5555``, and
``tcp://10.0.1.42:5555`` all name the same thing to a user — so accept all of
them, and complain in words when the input cannot be repaired.
"""
from __future__ import annotations

DEFAULT_PORT = 5555

DEFAULT_ADDRESS = f"tcp://localhost:{DEFAULT_PORT}"


class AddressError(ValueError):
    """The given address cannot be turned into a ZMQ endpoint."""


def normalize_address(raw: str, *, default_port: int = DEFAULT_PORT) -> str:
    """Return *raw* as a ZMQ endpoint, filling in the parts users leave out.

    A missing scheme becomes ``tcp://`` and a missing port becomes
    *default_port*. Non-TCP endpoints (``ipc://``, ``inproc://``) are passed
    through untouched — they have no host or port to complete.

    Raises
    ------
    AddressError
        The address is empty, has no host, or has a port that is not a number.
    """
    address = raw.strip()
    if not address:
        raise AddressError("empty address")

    scheme, separator, remainder = address.partition("://")
    if not separator:
        scheme, remainder = "tcp", address
    if scheme != "tcp":
        return address

    host, port = _split_host_port(remainder)
    if not host:
        raise AddressError(f"{raw!r} has no host — expected something like tcp://HOST:PORT")
    if port:
        try:
            port_number = int(port)
        except ValueError:
            raise AddressError(f"{raw!r} has a non-numeric port {port!r}") from None
        if not 1 <= port_number <= 65535:
            raise AddressError(f"{raw!r} has a port outside 1-65535")
    return f"tcp://{host}:{port or default_port}"


def _split_host_port(text: str) -> tuple[str, str]:
    """Split ``host``, ``host:port``, ``[v6]``, or ``[v6]:port`` into its parts.

    A bare IPv6 literal is bracketed on the way out, since that is the only
    form ZMQ can tell apart from a ``host:port`` pair.
    """
    if text.startswith("["):
        host, _, remainder = text.partition("]")
        return f"{host}]", remainder.lstrip(":")
    if text.count(":") == 1:
        host, _, port = text.partition(":")
        return host, port
    if ":" in text:
        return f"[{text}]", ""
    return text, ""
