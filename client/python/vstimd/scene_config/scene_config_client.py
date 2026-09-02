from __future__ import annotations

from typing import Callable

from vstimd._proto import service_pb2, system_pb2
from vstimd.response import ServerResponse


_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class SceneConfigClient:
    """Save, load, and retrieve named scene-configs on the server.

    Accessed as ``conn.scene_config`` on a :class:`~vstimd.Connection` instance.

    A scene-config is one experiment: stimuli, animations, background,
    photodiode and the named VTL trigger lines.  The server stores each one in a
    *project* — a directory holding everything a study needs — under its
    ``--storage-dir``, as
    ``<storage-dir>/projects/<project>/scene-configs/<name>.config.json``.

    Methods that take a *name* accept ``[<project>/]<name>``.  An unqualified
    name means the ``default`` project, so the common case stays one word; the
    shipped demos live in ``demos``.

    Example::

        with Connection() as conn:
            # Save the current scene into the default project.
            conn.scene_config.save("my_experiment")

            # Later, restore it.
            conn.scene_config.load("my_experiment")

            # Load one of the shipped demos.
            conn.scene_config.load("demos/drifting_grating")

            # List what's available, across every project.
            names = conn.scene_config.list_scene_configs()

            # ...or just one project's.
            names = conn.scene_config.list_scene_configs(project="demos")

            # Round-trip: retrieve raw JSON, inspect it, re-upload.
            json_str = conn.scene_config.retrieve()
            conn.scene_config.upload("backup", json_str, overwrite=True)
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    def list_scene_configs(self, *, project: str = "") -> list[str]:
        """Return the scene-config names available on the server.

        Parameters
        ----------
        project:
            Empty (default) lists every project, returning ``<project>/<name>``
            for anything outside ``default`` and a bare name inside it.  Naming
            a project scopes the listing to it and returns bare names.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            list_scene_configs=system_pb2.ListSceneConfigsRequest(project=project),
        )
        resp = self._send(req)
        return list(resp.scene_config_list.names)

    def load(self, name: str, *, additive: bool = False) -> ServerResponse:
        """Load a named scene-config from the server.

        Parameters
        ----------
        name:
            ``[<project>/]<name>`` — no extension.  An unqualified name means
            the ``default`` project.
        additive:
            If ``True``, merge stimuli and animations into the existing scene
            (handles are remapped to avoid collisions).  The I/O config (VTL
            names) is always fully replaced.  If ``False`` (default), the
            scene is cleared before loading.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            load_scene_config=system_pb2.LoadSceneConfigRequest(
                name=name, additive=additive
            ),
        )
        return ServerResponse._from_proto(self._send(req))

    def retrieve(self) -> str:
        """Return the current scene and I/O config as a JSON string.

        The returned JSON can be saved locally, inspected, or re-uploaded via
        :meth:`upload`.  The format is the same as the server's
        ``.config.json`` files.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            retrieve_scene_config=system_pb2.RetrieveSceneConfigRequest(),
        )
        resp = self._send(req)
        return resp.retrieved_scene_config.json

    def upload(
        self,
        name: str,
        json: str,
        *,
        overwrite: bool = False,
        apply_now: bool = False,
        additive: bool = False,
    ) -> ServerResponse:
        """Upload a scene-config JSON string to the server.

        Parameters
        ----------
        name:
            ``[<project>/]<name>`` — no extension.  An unqualified name means
            the ``default`` project, which is created if it does not exist.
        json:
            Scene-config JSON string — as produced by :meth:`retrieve`.
        overwrite:
            If ``False`` (default) and a scene-config with this name exists,
            raises :class:`~vstimd.SceneConfigAlreadyExistsError`.
        apply_now:
            If ``True``, apply the scene-config immediately after saving.
        additive:
            Only used when *apply_now* is ``True``.  See :meth:`load`.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            upload_scene_config=system_pb2.UploadSceneConfigRequest(
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
            ``[<project>/]<name>`` — no extension.  An unqualified name means
            the ``default`` project.
        overwrite:
            If ``False`` (default) and the name already exists, raises
            :class:`~vstimd.SceneConfigAlreadyExistsError`.
        """
        json = self.retrieve()
        return self.upload(name, json, overwrite=overwrite)
