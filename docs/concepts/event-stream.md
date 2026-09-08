# The event stream

vstimd broadcasts what it *saw* — frames it dropped, frames it presented,
trigger lines that changed, animations that started and ended — on a ZeroMQ
**PUB** socket, port **5556** by default.

Subscribing is connecting. Unsubscribing is disconnecting. There is no
registration, no setting naming a consumer, and **vstimd has no way to know
whether anybody is listening**. A rig with nothing subscribed renders exactly the
same, which is what vstimd does most of the time: it is always on, for alignment,
display checks and luminance, with no experiment running at all.

That is not indifference. It is the only thing a renderer can honestly promise.
It cannot know whether a consumer exists, or should, or is in the middle of a
session — so **only a consumer can tell "not yet" from "never"**, and the
deadline belongs to whoever is waiting.

## This is not the command socket

| | port | shape |
|---|---|---|
| commands | 5555 | REP — you ask, it answers |
| events | 5556 | PUB — it states, you listen or you don't |

Two sockets because they are two different things. A request has an addressee
and a deadline; an observation has neither. Keeping events off the command
socket also keeps them out of the path that must not stall: commands are
dispatched under a write lock on the scene, and a per-frame stream there would
be contention on the one path a frame clock cannot afford.

## Reading it

```python
from vstimd.events import EventSubscriber, Topic

with EventSubscriber("rig.local", topic=Topic.FRAME_DROPPED) as events:
    for event in events:
        print(event.frame, event.payload.count, event.missed_before)
```

`EventSubscriber` handles the two traps below for you. Raw, it is two ZeroMQ
frames and a protobuf decode:

```python
topic, payload = socket.recv_multipart()
event = events_pb2.Event()
event.ParseFromString(payload)
```

Each message is two frames: `[topic][encoded Event]`. The topic frame is what
ZeroMQ filters on without decoding anything, so a subscriber that only cares
about frame loss does not pay for every presented frame.

## Two clocks

Every event is timestamped twice, on the envelope rather than in any payload —
so *every* event is placeable, including ones whose payload has no time of its
own.

| | |
|---|---|
| `monotonic_us` | microseconds since the server started. Orders an event against anything, including others on the same frame. |
| `frame` | the display's frame index when it was stated. |

**`frame` is the one a trial is measured on.** Microseconds are continuous and a
display is not: a stimulus is on screen for a whole number of refreshes, so
"which frame" is exact where "which microsecond" carries the uncertainty of
whatever measured it.

Neither is wall-clock, deliberately. A consumer that needs wall-clock has its own
and its own uncertainty about the offset, which is a problem only it can solve —
and the reason a rig puts its onset markers on
[TTL lines](recording-integration.md) recorded by the acquisition system's own
clock instead.

Topics are hierarchical and dot-separated, so a prefix means what it looks like:

| topic | payload |
|---|---|
| `frame.dropped` | `FrameDropped` |
| `frame.presented` | `FramePresented` |
| `vtl.edge` | `VtlLineChanged` |
| `animation.state` | `AnimationStateChanged` |
| `server.started` | `ServerStarted` |

`"frame."` takes both frame events; `""` takes everything.

**Ignore topics you do not recognise.** More will be added, and a client that
treated an unknown topic or an unset `payload` as an error would break on a
server upgrade it had no reason to care about.

## Two things you must handle

### PUB drops, and will not tell you

If your subscriber cannot keep up, ZeroMQ discards messages for it silently.
That is the correct behaviour for a renderer — the frame clock must never wait
on a socket — but it means **a gap is invisible unless you look for one**.

`Event.sequence` is what you look for. It is monotonic from 1, and it is
assigned *before* the message is queued, so an event lost anywhere — a slow
subscriber, or vstimd's own internal queue under load — leaves a hole you can
see.

```python
if event.sequence != expected:
    missing = event.sequence - expected
    # You cannot recover them. Decide what that means for the window they span.
expected = event.sequence + 1
```

Where a count matters, **prefer differencing an absolute number over counting
events**. `FrameDropped.total_since_start` is there for exactly this: it is
correct across a gap, and counting messages is not.

### A restart resets everything

`ServerStarted` carries an `instance_id`. When you see a new one, every number
you were holding — sequence and **both clocks** — belongs to a different run of
the server. Discard them: they are small integers that look perfectly reasonable
next to the new ones, which is exactly why this is a message and not a footnote.
`EventSubscriber` raises `ServerRestarted` rather than letting it pass.

## What it does *not* carry

**No trial ever appears in this stream, and none ever will.** vstimd renders;
what counts as a trial is the decision authority's business. Every event carries
`frame`, and a consumer that wants per-trial numbers notes it when it configures
a trial and again when the trial ends, and owns the join — it is the only side
that knows what a trial is.

That is what lets vstimd stay usefully ignorant. It has no trial concept to keep
in sync, no session state to get wrong, and no way to mislabel data it never
understood.

## The stream is a record, never a trigger

`vtl.edge` reports lines changing, and it is tempting to arm something on it.
**Don't.** The TTL edge is what carries the timing; this is the account of it,
arriving over a slow bus after the fact. A consumer treating it as a trigger has
reinvented the host round trip that
[virtual trigger lines](vtl-and-animations.md) exist to avoid.

## Switches

| | |
|---|---|
| `--event-port <N>` | Port for the stream. Default 5556. |
| `--no-events` | Do not publish at all. |

The stream is on by default: a PUB socket with no subscribers costs one bind and
nothing per frame, and a rig where it is off by default is a rig where somebody
debugging it has to restart the daemon first.

## The schema

`proto/vstimd/v1/events.proto` is the whole surface and is meant to be read
start to finish, like the command API's `service.proto`.
