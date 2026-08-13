"""Command-line interface to a vstimd server (``vstimd-client``)."""
from .address import AddressError, normalize_address
from .discovery import (
    DiscoveredServer,
    DiscoveryUnavailableError,
    available_backends,
    discover,
)
from .exit_codes import ExitCode
from .main import main, run

__all__ = [
    "AddressError",
    "DiscoveredServer",
    "DiscoveryUnavailableError",
    "ExitCode",
    "available_backends",
    "discover",
    "main",
    "normalize_address",
    "run",
]
