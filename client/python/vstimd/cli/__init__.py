"""Command-line interface to a vstimd server (``vstimd-client``)."""
from .discovery import (
    DiscoveredServer,
    DiscoveryUnavailableError,
    available_backends,
    discover,
)
from .main import main, run

__all__ = [
    "DiscoveredServer",
    "DiscoveryUnavailableError",
    "available_backends",
    "discover",
    "main",
    "run",
]
