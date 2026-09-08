"""The SUB socket, and the two things a PUB stream makes a subscriber's problem."""

from __future__ import annotations

import dataclasses
from collections.abc import Iterator
from typing import Any

import zmq  # type: ignore[import]

from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1 import events_pb2

#: The port vstimd publishes on by default. The command port plus one.
DEFAULT_EVENT_PORT = 5556


class Topic:
    """The topics a subscriber can filter on, at the socket.

    Hierarchical and dot-separated, so a prefix means what it looks like:
    ``Topic.FRAME`` takes both frame events, ``""`` takes everything. Filtering
    here rather than after decoding is the point of the topic frame -- a
    subscriber that only cares about frame loss should not pay for every
    presented frame.
    """

    ALL = ""
    FRAME = "frame."
    FRAME_DROPPED = "frame.dropped"
    FRAME_PRESENTED = "frame.presented"
    VTL_EDGE = "vtl.edge"
    ANIMATION_STATE = "animation.state"
    SERVER_STARTED = "server.started"
    COMMAND_APPLIED = "command.applied"


def decode_command(applied: events_pb2.CommandApplied) -> service_pb2.Request:
    """The command inside a ``command.applied`` event.

    ``CommandApplied.request`` is raw bytes rather than an embedded message, so
    events.proto need not import the whole command surface and a subscriber that
    only counts commands never decodes one. This is the other half of that
    bargain, for a subscriber that wants to read them.

    A replayer generally does **not** need this: the bytes can be sent straight
    back at a server's command socket without ever being decoded, which is the
    other reason they are bytes.
    """
    request = service_pb2.Request()
    request.ParseFromString(applied.request)
    return request


class ServerRestarted(RuntimeError):
    """The stream is now a different run of the server.

    Sequence numbers, the monotonic clock and frame indices all restart with the
    process, so anything held from before is not comparable to anything after.
    Raised rather than logged because silently continuing is how a frame index
    from one run gets differenced against one from another.
    """


@dataclasses.dataclass(frozen=True, slots=True)
class Event:
    """One published event, with the gap before it if there was one."""

    topic: str
    message: events_pb2.Event
    missed_before: int = 0
    """How many events were lost immediately before this one.

    Non-zero means PUB dropped messages for this subscriber. They are gone --
    there is no retransmission and no backlog -- so what this is for is deciding
    what the loss means for the window it spans. Where a count matters, prefer
    differencing an absolute field such as ``FrameDropped.total_since_start``,
    which is correct across a gap; counting events is not.
    """

    @property
    def sequence(self) -> int:
        """Where this sits in the stream. See :attr:`missed_before`."""
        return self.message.sequence

    @property
    def frame(self) -> int:
        """The display's frame index when this was stated.

        **The clock a trial is measured on**, and the join key. Microseconds are
        continuous and a display is not: a stimulus is on screen for a whole
        number of refreshes, so "which frame" is exact where "which microsecond"
        carries the uncertainty of whatever measured it.

        Note the frame when a trial is configured and again when it ends, and
        everything stated between the two belongs to that trial. vstimd never
        learns what a trial is; you own the join because you are the only side
        that knows.

        Zero before the first frame is presented, which in practice means only
        ``server.started``. Restarts with the server -- see
        :class:`ServerRestarted`.
        """
        return self.message.frame

    @property
    def monotonic_us(self) -> int:
        """Microseconds since the server started. The other clock.

        Orders this event against anything else on the stream, including events
        that fall on the same frame.
        """
        return self.message.monotonic_us

    @property
    def kind(self) -> str:
        """Which payload is set, or ``""`` for one this client does not know.

        **An unknown kind is not an error.** More event types will be added, and
        a client that refused them would break on a server upgrade it had no
        reason to care about.
        """
        return self.message.WhichOneof("payload") or ""

    @property
    def payload(self) -> Any:
        """The set payload, or None for a kind this client does not know."""
        kind = self.kind
        return getattr(self.message, kind) if kind else None


class EventSubscriber:
    """A SUB socket on one vstimd's event stream.

    Use it as a context manager, or call :meth:`close`::

        with EventSubscriber("rig.local", topic=Topic.FRAME_DROPPED) as events:
            for event in events:
                print(event.payload.frame, event.payload.count)
    """

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = DEFAULT_EVENT_PORT,
        *,
        topic: str = Topic.ALL,
        context: zmq.Context | None = None,
    ) -> None:
        self._owns_context = context is None
        self._context = context if context is not None else zmq.Context.instance()
        self._socket = self._context.socket(zmq.SUB)
        self._socket.connect(f"tcp://{host}:{port}")
        self._socket.setsockopt_string(zmq.SUBSCRIBE, topic)

        self._expected_sequence: int | None = None
        self._instance_id: str | None = None
        self.gaps = 0
        """Events lost to this subscriber since it connected.

        A number that moves means the subscriber is not keeping up, or the
        server's own queue overflowed. Either way the data is gone.
        """

    # -- the socket -------------------------------------------------------------

    def close(self) -> None:
        self._socket.close()

    def __enter__(self) -> EventSubscriber:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def __iter__(self) -> Iterator[Event]:
        while True:
            event = self.receive()
            if event is not None:
                yield event

    def receive(self, timeout_ms: int | None = None) -> Event | None:
        """The next event, or None if `timeout_ms` passed without one.

        Raises:
            ServerRestarted: if the stream is now a different run of the server.
        """
        if timeout_ms is not None and not self._socket.poll(timeout_ms):
            return None
        topic_frame, payload = self._socket.recv_multipart()
        message = events_pb2.Event()
        message.ParseFromString(payload)
        return self._account_for(topic_frame.decode(), message)

    # -- the two things a PUB stream makes your problem -------------------------

    def _account_for(self, topic: str, message: events_pb2.Event) -> Event:
        missed = self._missed_before(message.sequence)
        self._expected_sequence = message.sequence + 1
        if message.WhichOneof("payload") == "server_started":
            self._check_instance(message.server_started.instance_id)
        return Event(topic=topic, message=message, missed_before=missed)

    def _missed_before(self, sequence: int) -> int:
        """How many events were lost immediately before this one.

        The first event sets the baseline rather than reporting everything
        published before the subscriber existed as a gap: a subscriber that
        joined at sequence 4000 did not *miss* 3999 events, it was not there.
        """
        if self._expected_sequence is None:
            return 0
        missed = max(0, sequence - self._expected_sequence)
        self.gaps += missed
        return missed

    def _check_instance(self, instance_id: str) -> None:
        if self._instance_id is None:
            self._instance_id = instance_id
            return
        if instance_id != self._instance_id:
            was, self._instance_id = self._instance_id, instance_id
            # The counters restart with the process, so anything held is stale.
            self._expected_sequence = None
            raise ServerRestarted(
                f"the event stream is now server run {instance_id!r}, was {was!r}: "
                f"sequence numbers, the monotonic clock and frame indices have all "
                f"restarted, so any frame index or sequence held from before is not "
                f"comparable to anything after"
            )
