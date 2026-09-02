from __future__ import annotations

from typing import Callable, Iterable, Sequence, Union

from vstimd._handles import AnimationHandle, StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1 import conditions_pb2
from vstimd.response import ServerResponse
from .conditions_models import Condition, ConditionAction, ConditionStatus


_SendFn = Callable[[service_pb2.Request], service_pb2.Response]

#: What :meth:`ConditionsClient.declare` accepts for one condition: a bare
#: index, an ``(index, name)`` pair, or a :class:`Condition`.
ConditionSpec = Union[int, tuple[int, str], Condition]


def _condition(spec: ConditionSpec) -> conditions_pb2.Condition:
    if isinstance(spec, Condition):
        return conditions_pb2.Condition(index=spec.index, name=spec.name)
    if isinstance(spec, int):
        return conditions_pb2.Condition(index=spec)
    index, name = spec
    return conditions_pb2.Condition(index=index, name=name)


def _sys() -> service_pb2.SystemTarget:
    return service_pb2.SystemTarget()


class ConditionsClient:
    """Switch between experimental conditions.

    Accessed as ``conn.conditions`` on a :class:`~vstimd.Connection` instance.

    A **condition** is one step of an experimental protocol — baseline vs.
    treatment, numbered protocol steps — and it selects which stimuli and which
    animations are active.  Exactly one condition is active at a time, and
    switching is a hard cut on the next frame: there is no cross-fade.

    A condition is addressed by its **index**.  :meth:`declare` additionally
    gives an index a name, so a script can say ``set("probe")`` instead of
    ``set(2)``; declaring is optional, and an undeclared index is a perfectly
    good nameless condition.  Only a *name* has to be declared before use.

    Membership is per stimulus and per animation, and an **empty membership
    means every condition** — so a scene that never mentions conditions behaves
    exactly as it always did, and only what opts in is ever gated.

    A stimulus outside the active condition is hidden.  Its own ``enabled`` flag
    is left untouched, so switching back restores what you had set.  An
    animation outside the active condition does not advance; what a switch does
    to its lifecycle state is per-animation policy — see :class:`ConditionAction`.

    Conditions are part of the scene-config, so ``conn.scene_config.save(...)``
    stores the declarations, the memberships and the active index alike.

    Example::

        with Connection() as conn:
            conn.conditions.declare([(0, "baseline"), (1, "probe")])
            conn.conditions.set_stimulus_conditions(fixation, [])      # always on
            conn.conditions.set_stimulus_conditions(grating, [1])      # probe only
            conn.conditions.set_animation_conditions(flash, [1])

            conn.conditions.set("baseline")   # grating hidden
            conn.conditions.set("probe")      # grating shown, flash re-armed
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── The active condition ──────────────────────────────────────────────────

    def set(self, condition: Union[int, str]) -> ServerResponse:
        """Make *condition* active — by index, or by a declared name.

        A hard cut: the next frame is drawn from the new condition.

        An unknown *name* raises :class:`~vstimd.InvalidArgumentError` rather
        than selecting anything, because a typo that silently switched to some
        other condition would blank the screen.  Any *index* is accepted:
        declaring is what gives a condition a name, not what brings it into
        existence, so a protocol that just counts upwards needs no declarations.
        """
        if isinstance(condition, str):
            req = conditions_pb2.SetConditionRequest(name=condition)
        else:
            req = conditions_pb2.SetConditionRequest(index=condition)
        return ServerResponse._from_proto(
            self._send(service_pb2.Request(system=_sys(), set_condition=req))
        )

    def list_conditions(self) -> ConditionStatus:
        """Return the declared conditions and which one is active."""
        resp = self._send(
            service_pb2.Request(
                system=_sys(),
                list_conditions=conditions_pb2.ListConditionsRequest(),
            )
        )
        r = resp.condition_list
        return ConditionStatus(
            declared=[Condition(index=c.index, name=c.name) for c in r.conditions],
            active_index=r.active_index,
            active_name=r.active_name,
        )

    @property
    def active(self) -> int:
        """The active condition index."""
        return self.list_conditions().active_index

    # ── Declaring ─────────────────────────────────────────────────────────────

    def declare(self, conditions: Iterable[ConditionSpec]) -> ServerResponse:
        """Replace the declared condition set.

        Declaring gives an index a name; it neither creates nor destroys
        conditions, and it does not change which one is active.

        Parameters
        ----------
        conditions:
            Each entry is a bare index, an ``(index, name)`` pair, or a
            :class:`Condition`.  Indices must be unique, and so must the names
            — a duplicate of either makes an address ambiguous, and raises
            :class:`~vstimd.InvalidArgumentError`.
        """
        req = conditions_pb2.DeclareConditionsRequest(
            conditions=[_condition(c) for c in conditions]
        )
        return ServerResponse._from_proto(
            self._send(service_pb2.Request(system=_sys(), declare_conditions=req))
        )

    # ── Membership ────────────────────────────────────────────────────────────

    def set_stimulus_conditions(
        self, handle: StimulusHandle, conditions: Sequence[int]
    ) -> ServerResponse:
        """Set the conditions *handle* is active in.

        An empty sequence means every condition — the default, and what every
        stimulus starts as.  Outside its conditions the stimulus is hidden,
        with its own ``enabled`` flag untouched.
        """
        req = conditions_pb2.SetStimulusConditionsRequest(
            condition_indices=list(conditions)
        )
        return ServerResponse._from_proto(
            self._send(
                service_pb2.Request(stimulus=handle, set_stimulus_conditions=req)
            )
        )

    def set_animation_conditions(
        self,
        handle: AnimationHandle,
        conditions: Sequence[int],
        *,
        action: ConditionAction = ConditionAction.RESET,
    ) -> ServerResponse:
        """Set the conditions *handle* is active in, and what a switch does to it.

        An empty *conditions* sequence means every condition — the default.
        Outside its conditions the animation does not advance.

        Parameters
        ----------
        action:
            See :class:`ConditionAction`.  ``RESET`` (the default) re-arms the
            animation each time its condition becomes active and idles it when
            the condition leaves; ``HOLD`` freezes it; ``STOP`` idles it on the
            way out without re-arming it on the way in.
        """
        req = conditions_pb2.SetAnimationConditionsRequest(
            handle=handle,
            condition_indices=list(conditions),
            condition_action=conditions_pb2.ConditionAction.ValueType(action),
        )
        return ServerResponse._from_proto(
            self._send(
                service_pb2.Request(system=_sys(), set_animation_conditions=req)
            )
        )
