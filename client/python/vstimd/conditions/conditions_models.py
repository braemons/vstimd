from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum


class ConditionAction(IntEnum):
    """What a condition switch does to an animation's lifecycle state.

    Membership decides *whether* an animation runs; this decides what happens
    to one at the moment that answer changes.

    ``RESET`` (the default) idles the animation on the way out of its condition
    and re-arms it on the way in, so a protocol step plays the same way every
    time it comes up.  ``HOLD`` leaves the state alone: the animation freezes
    where it is and resumes from the same frame when its condition returns.
    ``STOP`` idles it on the way out but does not re-arm it on the way in.

    Values match ``vstimd.v1.ConditionAction`` on the wire.
    """

    RESET = 1
    HOLD = 2
    STOP = 3


@dataclass
class Condition:
    """One declared condition: the index it is addressed by, and its name.

    ``name`` is ``""`` for a condition declared without one — an index alone is
    enough to switch to it.
    """

    index: int
    name: str = ""


@dataclass
class ConditionStatus:
    """What :meth:`ConditionsClient.list_conditions` reports.

    ``declared`` is only what has been *named*; ``active_index`` need not be
    among them, because any index is a valid, nameless condition.
    """

    declared: list[Condition]
    active_index: int
    #: Name of the active condition, ``""`` when it has none.
    active_name: str
