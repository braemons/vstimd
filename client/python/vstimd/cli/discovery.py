"""mDNS/DNS-SD discovery of vstimd servers on the local network.

vstimd advertises itself as ``_vstimd._tcp`` (see
``packaging/avahi/vstimd.service.tmpl``) with an ``id=<hostname>`` TXT record.
The TXT record is the reliable identity: Avahi may append ``#2``, ``#3`` to the
advertised display name on collision, but the TXT record is a literal written
at boot by ``vstimd-set-hostname``.

Two backends are supported:

* ``zeroconf`` — the pure-Python `zeroconf <https://pypi.org/project/zeroconf/>`_
  package.  Cross-platform, no daemon needed, and a dependency of
  ``vstimd-client``, so it is normally present.
* ``avahi`` — shells out to ``avahi-browse``.  Linux only, needs a running
  ``avahi-daemon``.

By default the first available backend is used, ``zeroconf`` first.
"""
from __future__ import annotations

import shutil
import subprocess
import time
from dataclasses import dataclass, field
from typing import Any

SERVICE_TYPE = "_vstimd._tcp"
"""DNS-SD service type advertised by vstimd servers."""

_ZC_SERVICE_TYPE = SERVICE_TYPE + ".local."

DEFAULT_PORT = 5555


class DiscoveryUnavailableError(RuntimeError):
    """No usable discovery backend is installed."""


@dataclass(frozen=True)
class DiscoveredServer:
    """One vstimd server found on the local network."""

    name: str
    """Advertised service instance name (may carry an Avahi ``#2`` suffix)."""

    id: str
    """Value of the ``id=`` TXT record — the server's hostname. May be empty."""

    hostname: str
    """mDNS hostname, e.g. ``vstimd-a1b2c3.local``."""

    addresses: tuple[str, ...] = ()
    """Resolved IP addresses, in the order reported by the backend."""

    port: int = DEFAULT_PORT

    properties: dict[str, str] = field(default_factory=dict)
    """All TXT records, decoded as UTF-8."""

    @property
    def address(self) -> str:
        """ZMQ endpoint to pass to :class:`~vstimd.Connection`.

        Prefers the mDNS hostname (stable across DHCP leases) and falls back to
        the first resolved IP address.
        """
        host = self.hostname or (self.addresses[0] if self.addresses else "")
        return f"tcp://{host}:{self.port}"

    def _key(self) -> tuple[str, int]:
        return (self.id or self.name, self.port)


def discover(
    timeout_s: float = 2.0,
    *,
    backend: str | None = None,
) -> list[DiscoveredServer]:
    """Browse the local network for vstimd servers.

    Parameters
    ----------
    timeout_s:
        How long to listen for responses.  mDNS is best-effort — a longer
        timeout finds more servers on a busy or lossy network.
    backend:
        ``"zeroconf"``, ``"avahi"``, or ``None`` (default) to use whichever is
        available, preferring ``zeroconf``.

    Returns
    -------
    Servers sorted by ``id``, de-duplicated across network interfaces.

    Raises
    ------
    DiscoveryUnavailableError
        Neither the ``zeroconf`` package nor ``avahi-browse`` is available.
    """
    if backend == "zeroconf":
        found = _discover_zeroconf(timeout_s)
    elif backend == "avahi":
        found = _discover_avahi(timeout_s)
    elif backend is None:
        if _has_zeroconf():
            found = _discover_zeroconf(timeout_s)
        elif shutil.which("avahi-browse"):
            found = _discover_avahi(timeout_s)
        else:
            raise DiscoveryUnavailableError(
                "no mDNS backend available — zeroconf ships with vstimd-client, "
                "so reinstall it (pip install --force-reinstall vstimd-client) "
                "or install avahi-utils"
            )
    else:
        raise ValueError(f"unknown discovery backend: {backend!r}")

    return _dedupe(found)


def available_backends() -> list[str]:
    """Return the discovery backends usable on this machine."""
    backends = []
    if _has_zeroconf():
        backends.append("zeroconf")
    if shutil.which("avahi-browse"):
        backends.append("avahi")
    return backends


def _dedupe(servers: list[DiscoveredServer]) -> list[DiscoveredServer]:
    """Merge entries for the same server seen on several interfaces."""
    merged: dict[tuple[str, int], DiscoveredServer] = {}
    for server in servers:
        key = server._key()
        existing = merged.get(key)
        if existing is None:
            merged[key] = server
            continue
        addresses = existing.addresses + tuple(
            a for a in server.addresses if a not in existing.addresses
        )
        merged[key] = DiscoveredServer(
            name=existing.name,
            id=existing.id or server.id,
            hostname=existing.hostname or server.hostname,
            addresses=addresses,
            port=existing.port,
            properties={**server.properties, **existing.properties},
        )
    return sorted(merged.values(), key=lambda s: (s.id or s.name, s.port))


# ── zeroconf backend ──────────────────────────────────────────────────────────


def _has_zeroconf() -> bool:
    try:
        import zeroconf  # type: ignore[import-untyped]  # noqa: F401
    except ImportError:
        return False
    return True


def _discover_zeroconf(timeout_s: float) -> list[DiscoveredServer]:
    try:
        from zeroconf import ServiceBrowser, Zeroconf  # type: ignore[import-untyped]
    except ImportError as exc:  # pragma: no cover - exercised via backend selection
        raise DiscoveryUnavailableError(
            "the zeroconf package is not installed — it ships with "
            "vstimd-client, so reinstall it "
            "(pip install --force-reinstall vstimd-client)"
        ) from exc

    names: list[str] = []

    # zeroconf fires handlers with keyword arguments — the parameter names matter.
    def on_change(zeroconf: Any, service_type: str, name: str, state_change: Any) -> None:
        removed = "Removed" in str(state_change)
        if removed:
            if name in names:
                names.remove(name)
        elif name not in names:
            names.append(name)

    zc = Zeroconf()
    try:
        browser = ServiceBrowser(zc, _ZC_SERVICE_TYPE, handlers=[on_change])
        time.sleep(timeout_s)
        browser.cancel()

        servers = []
        for name in list(names):
            # Names are already cached by the browser, so this resolves fast.
            info = zc.get_service_info(_ZC_SERVICE_TYPE, name, timeout=1000)
            if info is None:
                continue
            servers.append(_from_zeroconf_info(name, info))
        return servers
    finally:
        zc.close()


def _from_zeroconf_info(name: str, info: Any) -> DiscoveredServer:
    props: dict[str, str] = {}
    for raw_key, raw_value in (getattr(info, "properties", None) or {}).items():
        key = raw_key.decode("utf-8", "replace") if isinstance(raw_key, bytes) else str(raw_key)
        if isinstance(raw_value, bytes):
            value = raw_value.decode("utf-8", "replace")
        else:
            value = "" if raw_value is None else str(raw_value)
        props[key] = value

    try:
        addresses = tuple(info.parsed_addresses())
    except Exception:
        addresses = ()

    return DiscoveredServer(
        name=_strip_suffix(name, "." + _ZC_SERVICE_TYPE),
        id=props.get("id", ""),
        hostname=_strip_suffix(getattr(info, "server", "") or "", "."),
        addresses=addresses,
        port=int(getattr(info, "port", DEFAULT_PORT) or DEFAULT_PORT),
        properties=props,
    )


# ── avahi-browse backend ──────────────────────────────────────────────────────


def _discover_avahi(timeout_s: float) -> list[DiscoveredServer]:
    if shutil.which("avahi-browse") is None:
        raise DiscoveryUnavailableError(
            "avahi-browse not found — install avahi-utils, or use the "
            "zeroconf backend (pip install 'vstimd[discover]')"
        )
    try:
        proc = subprocess.run(
            ["avahi-browse", "--resolve", "--parsable", "--terminate", "--no-db-lookup", SERVICE_TYPE],
            capture_output=True,
            text=True,
            # -t terminates on its own; the timeout is a backstop only.
            timeout=max(timeout_s, 1.0) + 10.0,
        )
    except subprocess.TimeoutExpired as exc:
        raise DiscoveryUnavailableError("avahi-browse did not terminate") from exc
    if proc.returncode != 0:
        detail = proc.stderr.strip() or f"exit code {proc.returncode}"
        raise DiscoveryUnavailableError(f"avahi-browse failed: {detail}")
    return parse_avahi_browse(proc.stdout)


def parse_avahi_browse(output: str) -> list[DiscoveredServer]:
    """Parse the ``avahi-browse --parsable`` output of resolved services.

    Resolved records start with ``=`` and have the fields::

        =;iface;proto;name;type;domain;hostname;address;port;txt...
    """
    servers = []
    for line in output.splitlines():
        if not line.startswith("="):
            continue
        fields = _split_avahi(line)
        if len(fields) < 9:
            continue
        props = _parse_txt(fields[9:])
        try:
            port = int(fields[8])
        except ValueError:
            port = DEFAULT_PORT
        servers.append(
            DiscoveredServer(
                name=fields[3],
                id=props.get("id", ""),
                hostname=fields[6],
                addresses=(fields[7],) if fields[7] else (),
                port=port,
                properties=props,
            )
        )
    return servers


def _split_avahi(line: str) -> list[str]:
    """Split on ``;``, honouring avahi's backslash escapes."""
    fields: list[str] = []
    current: list[str] = []
    escaped = False
    for char in line:
        if escaped:
            current.append(char)
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == ";":
            fields.append("".join(current))
            current = []
        else:
            current.append(char)
    fields.append("".join(current))
    return fields


def _parse_txt(txt_fields: list[str]) -> dict[str, str]:
    """Parse avahi's TXT column: space-separated, double-quoted ``k=v`` pairs."""
    props: dict[str, str] = {}
    for record in _split_quoted(" ".join(txt_fields)):
        key, _, value = record.partition("=")
        if key:
            props[key] = value
    return props


def _split_quoted(text: str) -> list[str]:
    records: list[str] = []
    current: list[str] = []
    in_quotes = False
    for char in text:
        if char == '"':
            if in_quotes:
                records.append("".join(current))
                current = []
            in_quotes = not in_quotes
        elif in_quotes:
            current.append(char)
    if current:
        records.append("".join(current))
    return records


def _strip_suffix(text: str, suffix: str) -> str:
    return text[: -len(suffix)] if suffix and text.endswith(suffix) else text
