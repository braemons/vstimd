"""What a subscriber asks for, over a real PUB/SUB pair.

The rest of the event tests drive the accounting offline, with no socket. These
need one: **topic filtering happens inside ZeroMQ**, before a message is ever
queued for the client, so a test that faked the socket would be testing this
module's opinion of ZMQ rather than ZMQ.

That filtering is the point of the topic frame. A consumer with work to do --
triald, say, which wants to know about lost frames and nothing else -- must not
be handed a heartbeat at the display's refresh rate, because a subscriber that
falls behind loses events silently and the cheapest way not to fall behind is
not to ask.
"""

from __future__ import annotations

import socket as socketlib

import pytest
import zmq

from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1 import events_pb2
from vstimd.events import EventSubscriber, Topic, decode_command


def free_port() -> int:
    with socketlib.socket() as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


class Publisher:
    """A stand-in for vstimd's PUB socket, framing events the way it does."""

    def __init__(self, port: int) -> None:
        self.socket = zmq.Context.instance().socket(zmq.PUB)
        self.socket.bind(f"tcp://127.0.0.1:{port}")

    def send(self, topic: str, **fields: object) -> None:
        event = events_pb2.Event(topic=topic, **fields)  # type: ignore[arg-type]
        self.socket.send_multipart([topic.encode(), event.SerializeToString()])

    def close(self) -> None:
        self.socket.close()


@pytest.fixture
def stream():
    port = free_port()
    publisher = Publisher(port)
    yield port, publisher
    publisher.close()


def settle(subscriber: EventSubscriber) -> None:
    """Give the subscription time to reach the publisher.

    Not politeness: ZMQ's connect is asynchronous and a PUB socket discards
    anything published before the subscription arrives. That slow-joiner window
    is inherent to PUB, and it is why `server.started` is a message rather than
    something a client assumes.
    """
    import time

    time.sleep(0.3)
    _ = subscriber


def test_a_single_topic_takes_that_topic_and_no_other(stream):
    port, publisher = stream
    with EventSubscriber("127.0.0.1", port, topic=Topic.FRAME_DROPPED) as events:
        settle(events)
        publisher.send(Topic.FRAME_PRESENTED, sequence=1, frame=1)
        publisher.send(Topic.FRAME_DROPPED, sequence=2, frame=2)

        event = events.receive(timeout_ms=2000)
        assert event is not None
        assert event.topic == Topic.FRAME_DROPPED
        # And the presented frame never arrives at all -- it was dropped by the
        # socket, not skipped by this client.
        assert events.receive(timeout_ms=200) is None


def test_several_topics_can_be_taken_at_once(stream):
    """The shape triald uses: frame loss and VTL edges, not the heartbeat."""
    port, publisher = stream
    wanted = [Topic.FRAME_DROPPED, Topic.VTL_EDGE]
    with EventSubscriber("127.0.0.1", port, topic=wanted) as events:
        settle(events)
        publisher.send(Topic.FRAME_PRESENTED, sequence=1, frame=1)
        publisher.send(Topic.FRAME_DROPPED, sequence=2, frame=2)
        publisher.send(Topic.SERVER_STARTED, sequence=3, frame=0)
        publisher.send(Topic.VTL_EDGE, sequence=4, frame=4)

        got = [events.receive(timeout_ms=2000), events.receive(timeout_ms=2000)]
        assert [e.topic for e in got if e] == [Topic.FRAME_DROPPED, Topic.VTL_EDGE]
        assert events.receive(timeout_ms=200) is None


def test_a_gap_left_by_the_filter_is_not_reported_as_loss(stream):
    """**The one trap in filtering**, and it must not cost a false alarm.

    Sequence numbers are assigned across the whole stream, so a subscriber
    taking one topic sees 2, 4, 7 -- holes it created itself by asking for less.
    Those are not the gaps `gaps` exists to catch.
    """
    port, publisher = stream
    with EventSubscriber("127.0.0.1", port, topic=Topic.FRAME_DROPPED) as events:
        settle(events)
        for sequence in range(1, 8):
            topic = Topic.FRAME_DROPPED if sequence in (2, 4, 7) else Topic.FRAME_PRESENTED
            publisher.send(topic, sequence=sequence, frame=sequence)

        received = [events.receive(timeout_ms=2000) for _ in range(3)]
        assert [e.sequence for e in received if e] == [2, 4, 7]
        assert events.gaps == 0, (
            "holes made by the subscriber's own filter are not lost events"
        )


def test_a_prefix_takes_the_whole_family(stream):
    port, publisher = stream
    with EventSubscriber("127.0.0.1", port, topic=Topic.FRAME) as events:
        settle(events)
        publisher.send(Topic.VTL_EDGE, sequence=1, frame=1)
        publisher.send(Topic.FRAME_PRESENTED, sequence=2, frame=2)
        publisher.send(Topic.FRAME_DROPPED, sequence=3, frame=3)

        got = [events.receive(timeout_ms=2000) for _ in range(2)]
        assert [e.topic for e in got if e] == [Topic.FRAME_PRESENTED, Topic.FRAME_DROPPED]


def test_the_topics_asked_for_are_readable_afterwards():
    subscriber = EventSubscriber("127.0.0.1", free_port(), topic=Topic.VTL_EDGE)
    assert subscriber.topics == (Topic.VTL_EDGE,)
    subscriber.close()


def test_no_topics_is_refused_rather_than_silently_receiving_nothing():
    """A SUB socket with no subscription receives nothing, and says nothing.

    An empty list is almost always a filter built from an empty config, and the
    result is indistinguishable from a rig that is switched off.
    """
    with pytest.raises(ValueError, match="not an empty list"):
        EventSubscriber("127.0.0.1", free_port(), topic=[])


def test_a_recorded_command_decodes_back_to_the_request_that_was_sent():
    """`CommandApplied.request` is bytes so events.proto need not import the
    whole command surface -- this is the other half of that bargain."""
    request = service_pb2.Request()
    request.create_rect.params.width_px = 100
    request.create_rect.params.height_px = 50

    applied = events_pb2.CommandApplied(
        request=request.SerializeToString(), accepted=True, response_handle=3
    )
    assert decode_command(applied) == request
    assert decode_command(applied).create_rect.params.width_px == 100
