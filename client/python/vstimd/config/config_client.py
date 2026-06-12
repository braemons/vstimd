from __future__ import annotations

from typing import Callable

from vstimd._proto import service_pb2, system_pb2
from vstimd.response import ServerResponse


_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class ConfigClient:
    """Save, load, and retrieve named scene configs on the server.

    Accessed as ``conn.config`` on a :class:`~vstimd.Connection` instance.

    The server maintains a *config directory* (set via ``--config-dir``).  Each
    config is stored as a ``<name>.config.json`` file.  Methods that accept a
    *name* argument use the bare name — no path, no extension.

    Example::

        with Connection() as conn:
            # Save the current scene to the server's config directory.
            conn.config.save("my_experiment")

            # Later, restore it.
            conn.config.load("my_experiment")

            # List what's available.
            names = conn.config.list_configs()

            # Round-trip: retrieve raw JSON, inspect it, re-upload.
            json_str = conn.config.retrieve()
            conn.config.upload("backup", json_str, overwrite=True)
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    def list_configs(self) -> list[str]:
        """Return bare config names available in the server's config directory."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            list_configs=system_pb2.ListConfigsRequest(),
        )
        resp = self._send(req)
        return list(resp.config_list.names)

    def load(self, name: str, *, additive: bool = False) -> ServerResponse:
        """Load a named config from the server's config directory.

        Parameters
        ----------
        name:
            Bare config name (no path, no extension).
        additive:
            If ``True``, merge stimuli and animations into the existing scene
            (handles are remapped to avoid collisions).  The I/O config (VTL
            names) is always fully replaced.  If ``False`` (default), the
            scene is cleared before loading.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            load_config=system_pb2.LoadConfigRequest(name=name, additive=additive),
        )
        return ServerResponse._from_proto(self._send(req))

    def retrieve(self) -> str:
        """Return the current scene and I/O config as a JSON string.

        The returned JSON can be saved locally, inspected, or re-uploaded via
        :meth:`upload`.  The format is the same as the server's ``.config.json``
        files.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            retrieve_config=system_pb2.RetrieveConfigRequest(),
        )
        resp = self._send(req)
        return resp.retrieved_config.json

    def upload(
        self,
        name: str,
        json: str,
        *,
        overwrite: bool = False,
        apply_now: bool = False,
        additive: bool = False,
    ) -> ServerResponse:
        """Upload a config JSON string to the server's config directory.

        Parameters
        ----------
        name:
            Bare config name (no path, no extension).
        json:
            Config JSON string — as produced by :meth:`retrieve`.
        overwrite:
            If ``False`` (default) and a config with this name already exists,
            raises :class:`~vstimd.ConfigAlreadyExistsError`.
        apply_now:
            If ``True``, apply the config immediately after saving.
        additive:
            Only used when *apply_now* is ``True``.  See :meth:`load`.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            upload_config=system_pb2.UploadConfigRequest(
                name=name,
                json=json,
                overwrite=overwrite,
                apply_now=apply_now,
                additive=additive,
            ),
        )
        return ServerResponse._from_proto(self._send(req))

    def save(self, name: str, *, overwrite: bool = False) -> ServerResponse:
        """Retrieve the current scene and save it under *name* in one call.

        Convenience wrapper around :meth:`retrieve` + :meth:`upload`.

        Parameters
        ----------
        name:
            Bare config name (no path, no extension).
        overwrite:
            If ``False`` (default) and the name already exists, raises
            :class:`~vstimd.ConfigAlreadyExistsError`.
        """
        json = self.retrieve()
        return self.upload(name, json, overwrite=overwrite)
