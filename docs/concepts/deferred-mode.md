# Deferred Mode

Deferred mode makes several stimulus changes appear on **exactly the same
frame**. Without it, two commands sent back to back may land on two different
frames — a difference of 16.7 ms at 60 Hz, and a visible artefact if the two
changes were meant to be one event.

## The three calls

```python
conn.system.set_deferred_mode(active=True)    # 1. begin: changes are staged

conn.stimuli.set_enabled(h1, True)            # 2. …not visible yet
conn.stimuli.set_position(h2, Vec2(100, 0))
conn.stimuli.set_fill_color(h3, Color(1.0, 0.0, 0.0))

conn.system.set_deferred_mode(active=False)   # 3. commit: all three, one frame
```

Between the begin and the commit, commands are accepted and acknowledged as
usual but nothing on screen changes. The commit queues one atomic flip, and from
the next frame onwards the whole batch is visible together.

To throw the staged changes away instead of showing them:

```python
conn.system.set_deferred_mode(active=False, cancel=True)
```

## Knowing which frame it landed on

The reply is not an acknowledgement but a `DeferredModeStatus`, and for a
timed experiment its useful field is `flip_frame` — the absolute frame number
the batch is first drawn from:

```python
conn.system.set_deferred_mode(active=True)
conn.stimuli.set_enabled(left,  True)
conn.stimuli.set_enabled(right, True)
ended = conn.system.set_deferred_mode(active=False)

if ended.flip_scheduled:
    conn.system.wait_for_frame(ended.flip_frame)   # returns once it is on screen
    log_stimulus_onset(trial, frame=ended.flip_frame)
```

That is the number to record as the stimulus onset: it identifies the frame, so
it can be lined up with a photodiode trace or a VTL marker afterwards.

| Field | Means |
|---|---|
| `deferred` | Deferred mode after the call — true only on the call that began it |
| `flip_scheduled` | A flip is queued; `flip_frame` is meaningful |
| `flip_frame` | The frame the staged state is first drawn from; `0` when nothing was queued |
| `was_deferred` | Deferred mode *before* the call |
| `frame_count` | The frame the server was on when it handled this call |
| `was_a_no_op` | The call found nothing to do — neither begun nor staged |
| `frames_staged` | How many frames the staged batch spanned, if this call ended one |

Ending or cancelling a mode that was never begun is a **no-op, not an error** —
a client is allowed to say "off, whatever the state" — which is why the reply
distinguishes the two rather than just succeeding.

## What it guarantees

- Every command sent between begin and commit becomes visible on one frame,
  together.
- Only one batch can be in progress at a time.
- Commands sent *outside* a batch are applied as soon as the render loop picks
  them up, typically within one frame — which is fine for anything whose timing
  your script sets, and not fine for anything that has to coincide.

## When to reach for it

- **Flicker and reversal paradigms** — several stimuli toggling as one event.
- **Stimulus arrays** — move or recolour a whole set without it arriving
  piecemeal.
- **Reveal** — create several stimuli disabled, then show them all at once.
- **Any onset you will timestamp** — one flip means one onset frame to record,
  rather than a spread of two or three.

An animation can also end a batch for you: `FinalAction.END_DEFERRED` commits on
the frame the animation completes, which is how a deferred change gets tied to
on-device timing rather than to your script's. See
[Triggers & animations](vtl-and-animations.md).

For how staging and the flip are implemented, see
[Architecture](../developer/architecture.md).
