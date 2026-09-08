"""Subscribing to what a vstimd saw: frame drops, presented frames, VTL edges.

A separate socket from :class:`~vstimd.connection.Connection`, and a different
kind of thing. A command is a request with an addressee and a deadline; an event
is a statement with neither. The server publishes and **learns about nobody** --
connecting is the whole of subscribing, and a rig with nothing attached renders
exactly the same.

Which means the two failure modes are yours to handle, and both are here:

* **A gap is invisible unless you look.** ZMQ PUB discards messages for a
  subscriber that cannot keep up and does not say so. :attr:`Event.sequence` is
  how you notice; :attr:`EventSubscriber.gaps` counts what you missed.
* **A restart resets everything.** Sequence numbers, the monotonic clock and
  frame indices all begin again. `server.started` carries an ``instance_id``,
  and this class raises the first time it changes rather than letting numbers
  from two runs be compared.
"""

from vstimd.events.subscriber import (
    Event,
    EventSubscriber,
    ServerRestarted,
    Topic,
    decode_command,
)

__all__ = [
    "Event",
    "EventSubscriber",
    "ServerRestarted",
    "Topic",
    "decode_command",
]
