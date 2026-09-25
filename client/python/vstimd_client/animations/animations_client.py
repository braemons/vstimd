from __future__ import annotations

import math
from typing import Callable, Optional, Union

from vstimd_client._handles import AnimationHandle, StimulusHandle
from vstimd_client._proto import service_pb2
from vstimd_client._proto.vstimd.v1 import animations_pb2, vtl_pb2
from vstimd_client.conditions import ConditionAction
from vstimd_client.response import ServerResponse
from vstimd_client.vtl import VtlHandle
from .device_models import AxisMap, AxisRef
from .animations_models import AnimationDetails, AnimationInfo, AnimationState, CancelAction, FinalAction, StartAction, VtlEdge, VtlPolarity
from vstimd_client.stimuli import RectParams, ShapeAppearance


_SendFn = Callable[[service_pb2.Request], service_pb2.Response]
_FpsGetter = Callable[[], float]

# A stimulus or list of stimuli.
Stimuli = Union[StimulusHandle, list[StimulusHandle]]


def _to_stimuli(s: Stimuli) -> list[StimulusHandle]:
    return [s] if isinstance(s, int) else list(s)


def _target_stimuli(params: animations_pb2.CreateAnimationRequest) -> list[int]:
    """The stimulus handles out of an animation's target, or none — empty for a
    camera animation."""
    if params.target.WhichOneof("target") != "stimuli":
        return []
    return list(params.target.stimuli.handles)


def _target(stimuli: Stimuli | None) -> animations_pb2.AnimationTarget:
    """Wrap stimulus handles as the animation's target; ``None`` is the camera."""
    if stimuli is None:
        return animations_pb2.AnimationTarget(camera=animations_pb2.AnimationCamera())
    return animations_pb2.AnimationTarget(
        stimuli=animations_pb2.AnimationStimuli(handles=_to_stimuli(stimuli)),
    )


def _sys() -> service_pb2.SystemTarget:
    return service_pb2.SystemTarget()


class AnimationClient:
    """Frame-accurate animation commands.

    Accessed as ``conn.animations`` on a :class:`~vstimd_client.VstimdClient` instance.

    Animations run once per frame in the render loop. They are created in the
    ``IDLE`` state and must be *armed* before they fire.  Trigger-reactive
    animations wait for a VTL edge after arming; free-running animations start
    immediately when armed (unless ``start_trigger`` is also set).

    All ``create_*`` methods return an :class:`~vstimd_client.AnimationHandle`.

    Frame/time parameters accept either a ``*_frames`` integer or a ``*_ms``
    float.  Specify exactly one; the ms variant is converted using the server's
    reported frame rate, queried lazily on first use and cached.

    Example::

        with VstimdClient() as conn:
            h = conn.stimuli.shapes.create_rect(
                params=RectParams(width_px=100, height_px=100,
                                  appearance=ShapeAppearance(fill_color=Color(1, 0, 0))),
            )
            anim = conn.animations.create_flash(h, duration_ms=100)
            conn.animations.arm(anim)
    """

    def __init__(self, send: _SendFn, fps_getter: _FpsGetter) -> None:
        self._send = send
        self._fps_getter = fps_getter
        self._fps_cache: float | None = None

    @property
    def fps(self) -> float:
        """Server frame rate, queried once on first use."""
        if self._fps_cache is None:
            self._fps_cache = self._fps_getter()
        return self._fps_cache

    def refresh_fps(self) -> None:
        """Invalidate the cached frame rate so it is re-queried on next use."""
        self._fps_cache = None

    # ── Frame/ms conversion helpers ───────────────────────────────────────────

    def _to_frames(self, frames: int | None, ms: float | None, param: str) -> int:
        if frames is not None and ms is not None:
            raise ValueError(f"specify either {param}_frames or {param}_ms, not both")
        if frames is not None:
            return frames
        if ms is not None:
            return max(1, math.ceil(ms / 1000.0 * self.fps))
        raise ValueError(f"one of {param}_frames or {param}_ms must be specified")

    # ── Lifecycle ─────────────────────────────────────────────────────────────

    def arm(self, handle: AnimationHandle) -> ServerResponse:
        """Arm an animation (IDLE → ARMED or RUNNING for free-running types)."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=_sys(),
            arm_animation=animations_pb2.ArmAnimationRequest(handle=handle),
        )))

    def disarm(self, handle: AnimationHandle) -> ServerResponse:
        """Disarm an animation (returns it to IDLE)."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=_sys(),
            disarm_animation=animations_pb2.DisarmAnimationRequest(handle=handle),
        )))

    def cancel(self, handle: AnimationHandle) -> ServerResponse:
        """Cancel an animation with a clean teardown (ends in DONE).

        Unlike :meth:`disarm` (which just returns to IDLE), cancel applies the
        animation's configured ``cancel_action_mask`` (independent of
        ``final_action`` and possibly empty for a hard abort), including any
        ``cancel_action_trigger_line`` pulse, and releases the animation hold
        when cancelling from RUNNING. Works whether the animation is ARMED
        (stopped before it starts) or RUNNING.
        """
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=_sys(),
            cancel_animation=animations_pb2.CancelAnimationRequest(handle=handle),
        )))

    def delete(self, handle: AnimationHandle) -> ServerResponse:
        """Delete an animation."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=_sys(),
            delete_animation=animations_pb2.DeleteAnimationRequest(handle=handle),
        )))

    def list_animations(self) -> list[AnimationInfo]:
        """Return all animations and their current state."""
        resp = self._send(service_pb2.Request(
            system=_sys(),
            list_animations=animations_pb2.ListAnimationsRequest(),
        ))
        return [
            AnimationInfo(
                handle=AnimationHandle(a.handle),
                name=a.name,
                state=AnimationState(a.state),
                type_name=a.type_name,
                condition_indices=tuple(a.condition_indices),
                condition_enabled=a.condition_enabled,
            )
            for a in resp.animation_list.animations
        ]

    def query(self, handle: AnimationHandle) -> AnimationDetails:
        """Return the full configuration and current state of an animation."""
        resp = self._send(service_pb2.Request(
            system=_sys(),
            query_animation=animations_pb2.QueryAnimationRequest(handle=handle),
        ))
        r = resp.query_animation_response
        p = r.params
        # `type_name` is the server's canonical tag (the Rust variant name, which
        # is also the serde config-file tag) — sent verbatim in both list and
        # query, so it matches list_animations() and never drifts from configs.
        return AnimationDetails(
            handle=AnimationHandle(r.handle),
            name=p.name,
            state=AnimationState(r.state),
            type_name=r.type_name,
            stimuli=tuple(StimulusHandle(s) for s in _target_stimuli(p)),
            final_action=FinalAction(p.final_action_mask),
            cancel_action=CancelAction(p.cancel_action_mask),
            condition_indices=tuple(r.condition_indices),
            condition_action=ConditionAction(r.condition_action),
            condition_enabled=r.condition_enabled,
            camera=p.target.WhichOneof("target") == "camera",
            distance_travelled_cm=r.distance_travelled_cm,
            device_backend=r.device_backend,
            device_stale=r.device_stale,
        )

    # ── Shared keyword args (passed through _make_req) ────────────────────────

    def _make_req(
        self,
        stimuli: Stimuli | None,
        body_kwargs: dict,
        *,
        name: str,
        start_action_mask: StartAction,
        start_action_trigger_line: Optional[VtlHandle],
        final_action_mask: FinalAction,
        final_action_trigger_line: Optional[VtlHandle],
        final_action_level_line: Optional[VtlHandle],
        start_trigger: Optional[VtlHandle],
        start_edge: VtlEdge,
        cancel_trigger: Optional[VtlHandle],
        cancel_edge: VtlEdge,
        cancel_action_mask: CancelAction,
        cancel_action_trigger_line: Optional[VtlHandle],
    ) -> animations_pb2.CreateAnimationRequest:
        # Each trigger carries its own kind (via VtlHandle, or resolved from
        # a named line). Action trigger lines must address an output line — the
        # server rejects an input-directed handle there.
        return animations_pb2.CreateAnimationRequest(
            name=name,
            start_action_mask=int(start_action_mask),
            start_action_trigger_line=start_action_trigger_line._to_proto() if start_action_trigger_line else None,
            final_action_mask=int(final_action_mask),
            final_action_trigger_line=final_action_trigger_line._to_proto() if final_action_trigger_line else None,
            final_action_level_line=final_action_level_line._to_proto() if final_action_level_line else None,
            start_trigger=start_trigger._to_proto() if start_trigger else None,
            start_edge=int(start_edge),  # ty: ignore[invalid-argument-type]  (an IntEnum is the proto enum's int)
            cancel_trigger=cancel_trigger._to_proto() if cancel_trigger else None,
            cancel_edge=int(cancel_edge),  # ty: ignore[invalid-argument-type]  (an IntEnum is the proto enum's int)
            cancel_action_mask=int(cancel_action_mask),
            cancel_action_trigger_line=cancel_action_trigger_line._to_proto() if cancel_action_trigger_line else None,
            target=_target(stimuli),
            **body_kwargs,
        )

    # ── Animation types ───────────────────────────────────────────────────────

    def create_couple_visibility_to_trigger_line(
        self,
        trigger: VtlHandle,
        stimuli: Stimuli,
        *,
        polarity: VtlPolarity = VtlPolarity.ACTIVE_HIGH,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Mirror stimulus enabled state to the level of a trigger line (input or output).

        ``polarity`` selects which level shows the stimuli: ``ACTIVE_HIGH``
        (default) shows them while the line is HIGH, ``ACTIVE_LOW`` while LOW.
        """
        req = self._make_req(
            stimuli, {
                "couple_visibility_to_trigger_line":
                    animations_pb2.CoupleVisibilityToTriggerLine(
                        trigger=trigger._to_proto(),
                        polarity=int(polarity),  # ty: ignore[invalid-argument-type]  (an IntEnum is the proto enum's int)
                    ),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_enable_on_trigger_edge(
        self,
        trigger: VtlHandle,
        stimuli: Stimuli,
        *,
        edge: VtlEdge = VtlEdge.RISING,
        enabled: bool = True,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Set stimulus enabled once when a trigger edge fires."""
        req = self._make_req(
            stimuli, {
                "enable_on_trigger_edge": animations_pb2.EnableOnTriggerEdge(
                    trigger=trigger._to_proto(),
                    edge=int(edge),  # ty: ignore[invalid-argument-type]  (an IntEnum is the proto enum's int)
                    enabled=enabled,
                ),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_flash(
        self,
        stimuli: Stimuli,
        duration_frames: int | None = None,
        *,
        duration_ms: float | None = None,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Enable stimuli for the given duration.

        If ``start_trigger`` is given, waits for that edge before starting;
        otherwise starts immediately when armed.
        """
        req = self._make_req(
            stimuli, {
                "flash_for_n_frames": animations_pb2.FlashForNFrames(
                    duration_frames=self._to_frames(duration_frames, duration_ms, "duration"),
                ),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_flicker(
        self,
        stimuli: Stimuli,
        on_frames: int | None = None,
        off_frames: int | None = None,
        *,
        on_ms: float | None = None,
        off_ms: float | None = None,
        total_frames: int | None = None,
        total_ms: float | None = None,
        start_on_phase: bool = True,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Flicker stimuli on/off. Omit ``total_*`` to run forever.

        ``start_on_phase=False`` starts in the off-phase_cycles instead of the on-phase_cycles.
        If ``start_trigger`` is given, waits for that edge before starting.
        """
        msg = animations_pb2.FlickerForNFrames(
            on_frames=self._to_frames(on_frames, on_ms, "on"),
            off_frames=self._to_frames(off_frames, off_ms, "off"),
            start_on_phase=start_on_phase,
        )
        if total_frames is not None or total_ms is not None:
            msg.total_frames = self._to_frames(total_frames, total_ms, "total")
        req = self._make_req(
            stimuli, {"flicker_for_n_frames": msg},
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_move_along_path_2d(
        self,
        stimuli: Stimuli,
        x_px: list[float],
        y_px: list[float],
        *,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Move stimulus through a sequence of 2-D positions, one per frame.

        ``x_px`` and ``y_px`` must have the same length. The animation completes
        after all positions have been played.
        """
        if len(x_px) != len(y_px):
            raise ValueError("x_px and y_px must have equal length")
        req = self._make_req(
            stimuli, {
                "move_along_path_2d": animations_pb2.MoveAlongPath2D(x_px=x_px, y_px=y_px),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_move_along_segments_2d(
        self,
        stimuli: Stimuli,
        x_px: list[float],
        y_px: list[float],
        speed_px_per_sec: float,
        *,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Move stimulus along piecewise-linear waypoints at a constant speed.

        ``x_px`` and ``y_px`` must have the same length and at least 2 entries.
        ``speed_px_per_sec`` is in screen units per second; the server converts
        to frame steps using the measured display frame rate.
        """
        if len(x_px) != len(y_px):
            raise ValueError("x_px and y_px must have equal length")
        if len(x_px) < 2:
            raise ValueError("at least 2 waypoints required")
        req = self._make_req(
            stimuli, {
                "move_along_segments_2d": animations_pb2.MoveAlongSegments2D(
                    x_px=x_px, y_px=y_px, speed_px_per_sec=speed_px_per_sec,
                ),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_external_position_2d(
        self,
        stimuli: Stimuli,
        shm_name: str,
        *,
        x_offset_px: float = 0.0,
        y_offset_px: float = 0.0,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        final_action_mask: FinalAction = FinalAction(0),
        final_action_trigger_line: Optional[VtlHandle] = None,
        final_action_level_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Set stimulus position every frame from a rig-config input device.

        ``shm_name`` names the device — its rig-config name (``"eye_tracker"``) or
        its segment (``"/vstimd_gaze"``). Its first two axes must be absolute; they
        give the position in pixels after the rig-config's scale, plus the
        offsets. While the device is stale the stimulus holds its last position.

        Raises:
            InvalidArgumentError: the rig declares no such device, or its first
                two axes are not absolute.
        """
        req = self._make_req(
            stimuli, {
                "external_position_2d": animations_pb2.ExternalPosition2D(
                    shm_name=shm_name,
                    x_offset_px=x_offset_px,
                    y_offset_px=y_offset_px,
                ),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=final_action_mask,
            final_action_trigger_line=final_action_trigger_line,
            final_action_level_line=final_action_level_line,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_linear_nav_3d(
        self,
        speed_cm_per_s: float,
        *,
        wrap_period_cm: float | None = None,
        track_length_cm: float | None = None,
        fade_frames: int = 0,
        source: AxisRef | None = None,
        name: str = "",
        start_action_mask: StartAction = StartAction(0),
        start_action_trigger_line: Optional[VtlHandle] = None,
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
        cancel_action_mask: CancelAction = CancelAction(0),
        cancel_action_trigger_line: Optional[VtlHandle] = None,
    ) -> AnimationHandle:
        """Move the 3-D camera straight ahead at ``speed_cm_per_s``, every frame.

        The camera moves along its horizontal forward direction — its yaw, not
        its pitch — and keeps its height. Negative speed moves backwards. The
        animation never finishes on its own; cancel or disarm it.

        With ``wrap_period_cm``, the camera's ``z`` wraps into
        ``[0, wrap_period_cm)``, for an endless corridor built from geometry that
        repeats with that period. :meth:`query` reports the true distance as
        ``distance_travelled_cm``, which never wraps — log that, not the camera
        position.

        With ``track_length_cm`` instead, the track is finite, for content that
        does not repeat (a scanned corridor): once the camera is that far ahead of
        where it started, measured along its starting heading, the 3-D view fades
        to the background colour over ``fade_frames``, the camera jumps back to
        its start position and heading, and the view fades in again over
        ``fade_frames``. The camera holds still while fading out. 2-D stimuli are
        not faded. ``distance_travelled_cm`` counts real movement, never the jump.

        Change the speed with :meth:`set_nav_speed`. It is meant for scripted
        changes, not for streaming a treadmill's speed every frame — for that,
        give a ``source``: an axis of a rig-config input device. A rate axis is
        then the speed in cm/s, integrated over real frame time; a cumulative
        axis moves the camera by exactly its change. ``speed_cm_per_s`` is
        ignored while a source is set.

        Raises:
            InvalidArgumentError: a stimulus-only action bit (``ENABLE``,
                ``DISABLE``, ``RESTORE_VISIBILITY``) was given, or both
                ``wrap_period_cm`` and ``track_length_cm``.
        """
        req = self._make_req(
            None, {
                "linear_nav_3d": animations_pb2.LinearNav3D(
                    speed_cm_per_s=speed_cm_per_s,
                    wrap_period_cm=wrap_period_cm or 0.0,
                    track_length_cm=track_length_cm or 0.0,
                    fade_frames=fade_frames,
                    source=source.to_proto() if source else None,
                ),
            },
            name=name,
            start_action_mask=start_action_mask,
            start_action_trigger_line=start_action_trigger_line,
            final_action_mask=FinalAction(0),
            final_action_trigger_line=None,
            final_action_level_line=None,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=cancel_action_mask,
            cancel_action_trigger_line=cancel_action_trigger_line,
        )
        return self._create(req)

    def create_device_driven_transform(
        self,
        stimuli: Stimuli | None,
        device: str,
        axes: list[AxisMap],
        *,
        name: str = "",
        start_trigger: Optional[VtlHandle] = None,
        start_edge: VtlEdge = VtlEdge.RISING,
        cancel_trigger: Optional[VtlHandle] = None,
        cancel_edge: VtlEdge = VtlEdge.RISING,
    ) -> AnimationHandle:
        """Drive stimuli — or, with ``stimuli=None``, the 3-D camera — from an
        input device declared in the rig-config, every frame.

        Each :class:`AxisMap` names one of the device's axes and the transform
        channel it drives. The animation never finishes on its own. While the
        device is stale (its producer silent) the target holds still; check
        :meth:`query`'s ``device_stale``.

        Example — a treadmill walks the camera, a knob turns it::

            conn.animations.create_device_driven_transform(None, "treadmill", [
                AxisMap("distance", TransformChannel.FORWARD),
                AxisMap("knob", TransformChannel.YAW, gain=0.5),
            ])

        Raises:
            InvalidArgumentError: the device or an axis is unknown, a camera
                channel lacks a camera target, an absolute axis drives FORWARD
                or STRAFE, or a scale channel targets the camera.
        """
        req = self._make_req(
            stimuli, {
                "device_driven_transform": animations_pb2.DeviceDrivenTransform(
                    device=device, axes=[a.to_proto() for a in axes],
                ),
            },
            name=name,
            start_action_mask=StartAction(0),
            start_action_trigger_line=None,
            final_action_mask=FinalAction(0),
            final_action_trigger_line=None,
            final_action_level_line=None,
            start_trigger=start_trigger, start_edge=start_edge,
            cancel_trigger=cancel_trigger, cancel_edge=cancel_edge,
            cancel_action_mask=CancelAction(0),
            cancel_action_trigger_line=None,
        )
        return self._create(req)

    def set_nav_speed(self, handle: AnimationHandle, speed_cm_per_s: float) -> ServerResponse:
        """Change a :meth:`create_linear_nav_3d` animation's speed from the next frame."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=_sys(),
            set_nav_speed=animations_pb2.SetNavSpeedRequest(
                handle=handle, speed_cm_per_s=speed_cm_per_s
            ),
        )))

    # ── Internal ──────────────────────────────────────────────────────────────

    def _create(self, proto_req: animations_pb2.CreateAnimationRequest) -> AnimationHandle:
        resp = self._send(service_pb2.Request(system=_sys(), create_animation=proto_req))
        return AnimationHandle(resp.handle)
