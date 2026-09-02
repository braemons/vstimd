from __future__ import annotations

from dataclasses import dataclass, field

from vstimd._handles import StimulusHandle
from vstimd.stimuli.color import Color


@dataclass(order=True, repr=False)
class ServerVersion:
    """Semver triple reported by the server."""

    major: int
    minor: int
    patch: int

    def __repr__(self) -> str:
        return f"ServerVersion({self.major}, {self.minor}, {self.patch})"

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


@dataclass
class DeferredModeStatus:
    """What a :meth:`SystemClient.set_deferred_mode` call actually did.

    Ending or cancelling deferred mode that was never begun is a no-op rather
    than an error — a client is allowed to say "off, whatever the state" — so
    the reply distinguishes the two, and says when the staged frame lands.
    """

    #: Deferred mode after the call: true only when it was just begun.
    deferred: bool
    #: A flip is queued and lands on the next vsync.
    flip_scheduled: bool
    #: Deferred mode before the call: `False` here with `active=False` means the
    #: call did nothing, because there was nothing staged to end.
    was_deferred: bool
    #: The frame the staged state is first drawn from; 0 when no flip is queued.
    #: Pass it to :meth:`SystemClient.wait_for_frame` to wait for exactly that.
    flip_frame: int
    #: The frame the server was on when it handled this call. On the call that
    #: begins deferred mode this is where the staging started; on the one that
    #: ends it, where it ended — with ``flip_frame`` saying where it lands.
    frame_count: int

    @property
    def was_a_no_op(self) -> bool:
        """True when the call found nothing to do — neither begun nor staged."""
        return not self.deferred and not self.was_deferred

    @property
    def frames_staged(self) -> int | None:
        """How many frames the staged batch spans, if this call ended one.

        ``None`` for the call that begins deferred mode, and for one that found
        nothing to end: there is no span to report.
        """
        if not self.flip_scheduled:
            return None
        return self.flip_frame - self.frame_count


@dataclass(repr=False)
class ServerInfo:
    """Display and version information returned by :meth:`SystemClient.query_server_info`."""

    width_px: int
    height_px: int
    frame_rate_hz: float
    version: ServerVersion
    background_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0))

    def __repr__(self) -> str:
        return (
            f"ServerInfo(width_px={self.width_px}, height_px={self.height_px}, "
            f"frame_rate_hz={self.frame_rate_hz:.1f}, version={self.version})"
        )


@dataclass
class StimulusListEntry:
    """One entry returned by :meth:`SystemClient.list_stimuli`."""

    handle: StimulusHandle
    enabled: bool
    id: str
    name: str
    #: Conditions this stimulus is active in; empty means every condition.
    condition_indices: list[int] = field(default_factory=list)
