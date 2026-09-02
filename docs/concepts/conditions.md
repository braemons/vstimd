# Conditions

A **condition** is one step of an experimental protocol — baseline vs. treatment, or numbered
steps in a sequence — and it selects which stimuli and which animations are active. Exactly one
condition is active at a time, and switching is a **hard cut**: the next frame is drawn from the
new condition, with no cross-fade.

Conditions let overlapping sets share stimuli without duplication. A fixation dot that belongs to
every step is created once and simply never mentions conditions; only what opts in is ever gated.

## Indices, and optionally names

A condition is addressed by its **index**. Any index is a valid condition, so a protocol that just
counts upwards needs no setup at all:

```python
conn.conditions.set(0)
conn.conditions.set(1)
```

*Declaring* a condition additionally gives its index a name, so a script can say what it means:

```python
conn.conditions.declare([(0, "baseline"), (1, "probe")])
conn.conditions.set("probe")        # same as set(1)
```

Declaring neither creates nor destroys conditions, and it does not change which one is active.
Indices must be unique, and so must the names. An **unknown name is an error** rather than a
guess — a typo that silently selected some other condition would blank the screen. An unknown
*index* is fine: it is a valid, nameless condition.

## Membership

Membership is a list of condition indices, set per stimulus and per animation. An **empty list
means every condition**, which is what everything starts as — so a scene that never mentions
conditions behaves exactly as it always did.

```python
conn.conditions.set_stimulus_conditions(fixation, [])       # always shown
conn.conditions.set_stimulus_conditions(grating, [1])       # probe only
conn.conditions.set_stimulus_conditions(cue, [0, 1])        # both steps
```

A stimulus outside the active condition is **hidden**, and its own `enabled` flag is left
untouched — so switching back restores exactly what the operator had set, rather than turning on
something they had turned off. Visibility is three independent gates, all of which must be open:

| Gate | Set by | Reported as |
|---|---|---|
| `enabled` | you, via `set_enabled` | `StimulusInfo.enabled` |
| `condition_enabled` | the active condition | `StimulusInfo.condition_enabled` |
| `anim_enabled` | a running animation | `StimulusInfo.anim_enabled` |

## Animations across a switch

An animation outside the active condition does not advance: it observes no trigger and holds
nothing. What a switch does to its *lifecycle state* is per-animation policy, because the useful
readings genuinely differ — a sequence belonging to one protocol step wants to start afresh every
time that step comes up, while a background animation shared across steps wants to be left where
it is.

```python
from vstimd.conditions import ConditionAction

conn.conditions.set_animation_conditions(flash, [1])                            # RESET (default)
conn.conditions.set_animation_conditions(drift, [], action=ConditionAction.HOLD)
```

| `ConditionAction` | Leaving its condition | Entering its condition |
|---|---|---|
| `RESET` (default) | back to `IDLE`, releasing any visibility hold | re-armed, from the start |
| `HOLD` | state left alone; frozen where it is | resumes from the same frame |
| `STOP` | back to `IDLE` | left alone — arm it yourself |

A condition switch **disarms** rather than cancels: it is bookkeeping, and firing an animation's
`cancel_action` — which can pulse a trigger line — would put a mark on the recording that no
experiment asked for.

## Saved with the scene

Conditions are part of the [scene-config](saving-loading.md): the declarations, every membership
and the active index are all saved and restored. A scene that does not use conditions writes no
`conditions` block at all, so existing config files are unchanged.

```python
conn.scene_config.save("my_protocol")
conn.scene_config.load("my_protocol")   # comes back in the condition it was saved in
```

## In the overlay

The Stimuli panel shows the active condition — `2 (probe)` when named, `2` when not — with `◀` and
`▶` to step through the protocol by hand. A stimulus the active condition excludes is drawn greyed
in the table, with its `enabled` checkbox still showing what you asked for; an animation the active
condition excludes is marked inactive next to its state.

## Alternatives

Conditions are server-side state, which is what makes a switch one command and one frame. If your
protocol is better expressed as whole different scenes, `conn.scene_config.load(...)` replaces the
scene from a prepared file instead — heavier per switch, but with no need to pre-declare anything.
