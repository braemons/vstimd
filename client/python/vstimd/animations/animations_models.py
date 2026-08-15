from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum, IntFlag

from vstimd._handles import AnimationHandle, StimulusHandle


class AnimationState(IntEnum):
    IDLE = 0
    ARMED = 1
    RUNNING = 2
    DONE = 3


class VtlEdge(IntEnum):
    RISING = 0
    FALLING = 1


class VtlPolarity(IntEnum):
    """Which line level a level-sensitive animation treats as asserted.

    Values match ``vstimd.v1.VtlPolarity`` on the wire and are numbered so that
    truthiness reads naturally: ``ACTIVE_HIGH`` is truthy, ``ACTIVE_LOW`` falsy.
    """
    ACTIVE_LOW = 0
    ACTIVE_HIGH = 1


class StartAction(IntFlag):
    ENABLE                    = 0x02
    TOGGLE_PHOTODIODE         = 0x04
    START_ACTION_TRIGGER_LINE = 0x08


class FinalAction(IntFlag):
    """Actions applied when an animation completes.

    ``REARM`` is what makes a trigger-driven animation repeat: without it a
    completed animation is ``DONE`` and ignores further edges until it is armed
    again. With no ``start_trigger`` it behaves like ``RESTART``, which wins if
    both bits are set.

    Two ways to report completion on a VTL line, usable together on separate
    lines: ``FINAL_ACTION_TRIGGER_LINE`` pulses for one frame, marking *when* it
    finished, and ``DONE_LEVEL`` drives ``final_action_level_line`` HIGH until
    the animation next starts, answering *whether* it has finished.
    """
    DISABLE           = 0x01
    REARM                     = 0x02
    TOGGLE_PHOTODIODE = 0x04
    FINAL_ACTION_TRIGGER_LINE = 0x08
    RESTART                   = 0x10
    REVERSE                   = 0x20
    RESTORE_STATE             = 0x40
    END_DEFERRED              = 0x80
    DONE_LEVEL                = 0x100


class CancelAction(IntFlag):
    """Actions applied when an animation is cancelled (edge or software).

    Independent of :class:`FinalAction`; ``CancelAction(0)`` is a hard abort that
    leaves visibility as-is. ``RESTART``/``REVERSE`` do not apply — cancel is
    always terminal.
    """
    DISABLE                    = 0x01
    TOGGLE_PHOTODIODE          = 0x04
    CANCEL_ACTION_TRIGGER_LINE = 0x08
    RESTORE_STATE              = 0x40
    END_DEFERRED               = 0x80


@dataclass(frozen=True)
class AnimationInfo:
    handle: AnimationHandle
    name: str
    state: AnimationState
    type_name: str


@dataclass(frozen=True)
class AnimationDetails:
    handle: AnimationHandle
    name: str
    state: AnimationState
    type_name: str
    stimuli: tuple[StimulusHandle, ...]
    final_action: FinalAction
    cancel_action: CancelAction = CancelAction(0)
