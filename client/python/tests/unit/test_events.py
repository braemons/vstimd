"""The two things a PUB stream makes a subscriber's problem.

No socket: the accounting is exercised directly, because what is worth testing
is not that ZMQ delivers bytes but that a subscriber *notices* what it lost and
refuses to compare numbers across a server restart. Both are silent failures
otherwise, and both produce plausible-looking wrong answers rather than errors.
"""

from __future__ import annotations

import pytest

from vstimd._proto.vstimd.v1 import events_pb2
from vstimd.events import EventSubscriber, ServerRestarted, Topic


def an_event(sequence: int, *, frame: int = 1, count: int = 1) -> events_pb2.Event:
    event = events_pb2.Event(sequence=sequence, topic=Topic.FRAME_DROPPED, frame=frame)
    event.frame_dropped.count = count
    event.frame_dropped.total_since_start = count
    return event


def a_start(sequence: int, instance_id: str) -> events_pb2.Event:
    event = events_pb2.Event(sequence=sequence, topic=Topic.SERVER_STARTED)
    event.server_started.instance_id = instance_id
    event.server_started.version = "0.2.0"
    return event


class Offline(EventSubscriber):
    """A subscriber with no socket, so the accounting can be driven by hand."""

    def __init__(self) -> None:  # noqa: D107 - deliberately does not call super
        self._expected_sequence = None
        self._instance_id = None
        self.gaps = 0

    def feed(self, message: events_pb2.Event):
        return self._account_for(message.topic, message)


# -- a gap is invisible unless you look -----------------------------------------


def test_an_unbroken_stream_reports_no_loss():
    events = Offline()
    for sequence in range(1, 5):
        assert events.feed(an_event(sequence)).missed_before == 0
    assert events.gaps == 0


def test_a_gap_is_counted_and_carried_on_the_event_after_it():
    # PUB discards for a slow subscriber and says nothing. The sequence number is
    # the only signal, which is why the server assigns it before queueing.
    events = Offline()
    events.feed(an_event(1))
    gapped = events.feed(an_event(6))

    assert gapped.missed_before == 4
    assert events.gaps == 4


def test_joining_late_is_not_a_gap():
    # A subscriber that connected at sequence 4000 did not *miss* 3999 events,
    # it was not there. Reporting that as loss would make every connection look
    # like a fault.
    events = Offline()
    assert events.feed(an_event(4000)).missed_before == 0
    assert events.gaps == 0


def test_gaps_accumulate_across_several():
    events = Offline()
    events.feed(an_event(1))
    events.feed(an_event(3))  # missed 2
    events.feed(an_event(10))  # missed 4..9
    assert events.gaps == 7


# -- a restart resets everything ------------------------------------------------


def test_the_first_server_started_sets_the_baseline_quietly():
    events = Offline()
    events.feed(a_start(1, "run-a"))  # does not raise


def test_a_second_run_raises_rather_than_letting_numbers_be_compared():
    """The failure this exists to stop: a frame index from one run differenced
    against one from another. Both are small integers and neither looks wrong."""
    events = Offline()
    events.feed(a_start(1, "run-a"))
    events.feed(an_event(2, frame=900))

    with pytest.raises(ServerRestarted, match="run-b"):
        events.feed(a_start(1, "run-b"))


def test_the_restart_message_says_what_is_now_stale():
    events = Offline()
    events.feed(a_start(1, "run-a"))
    with pytest.raises(ServerRestarted) as restarted:
        events.feed(a_start(1, "run-b"))

    said = str(restarted.value)
    assert "frame index" in said and "sequence" in said


def test_after_a_restart_the_new_run_is_accounted_from_scratch():
    # The sequence baseline goes with it: the new run starts from 1, and
    # comparing that against the old expectation would report a huge fake gap.
    events = Offline()
    events.feed(a_start(1, "run-a"))
    events.feed(an_event(900))
    with pytest.raises(ServerRestarted):
        events.feed(a_start(1, "run-b"))

    assert events.feed(an_event(2)).missed_before == 0


# -- what a client does with an event it does not know --------------------------


def test_an_unknown_payload_is_not_an_error():
    """More event types will be added. A client that refused them would break on
    a server upgrade it had no reason to care about."""
    events = Offline()
    bare = events_pb2.Event(sequence=1, topic="something.new")
    received = events.feed(bare)

    assert received.kind == ""
    assert received.payload is None


def test_a_known_payload_is_reachable_without_naming_the_field_twice():
    events = Offline()
    received = events.feed(an_event(1, frame=4211, count=2))
    assert received.kind == "frame_dropped"
    assert received.payload.count == 2


def test_the_frame_is_on_the_envelope_because_it_is_a_clock():
    """Two clocks, and this is the one a trial is measured on. It is on the
    envelope rather than in each payload so *every* event is placeable on the
    frame axis, including ones whose payload has no frame of its own."""
    events = Offline()
    received = events.feed(an_event(1, frame=4211))

    assert received.frame == 4211
    assert not hasattr(received.payload, "frame")
