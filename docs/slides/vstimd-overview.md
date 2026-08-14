---
marp: true
title: vstimd — Visual Stimulation Daemon
description: Goals, clients/APIs, and the VTL + animation model
author: Joscha Schmiedt, University of Bremen
theme: uncover
paginate: true
size: 16:9
backgroundColor: #0f1115
color: #e6e6e6
style: |
  section {
    font-family: "Inter", "Helvetica Neue", system-ui, sans-serif;
    font-size: 26px;
    line-height: 1.45;
    text-align: left;
    justify-content: flex-start;
    padding: 60px 70px;
  }
  h1 { color: #7dd3fc; font-size: 1.9em; }
  h2 { color: #7dd3fc; font-size: 1.35em; border-bottom: 2px solid #1f2937; padding-bottom: .2em; }
  h3 { color: #a5b4fc; }
  strong { color: #fbbf24; }
  a { color: #7dd3fc; }
  /* ---- readable inline + block code ---- */
  code {
    background: #263041;
    color: #f4d58d;
    border-radius: 4px;
    padding: 0 .3em;
  }
  pre {
    background: #10141b;
    border: 1px solid #33405a;
    border-radius: 8px;
    font-size: .72em;
    color: #e9eef6;
  }
  pre code {
    background: transparent;
    color: #e9eef6;
    text-shadow: none;
  }
  /* highlight.js token colours tuned for the dark background */
  .hljs-keyword, .hljs-selector-tag, .hljs-built_in { color: #c792ea; }
  .hljs-string, .hljs-attr, .hljs-symbol { color: #b5e853; }
  .hljs-number, .hljs-literal, .hljs-meta { color: #ff9e64; }
  .hljs-comment, .hljs-quote { color: #8290ab; font-style: italic; }
  .hljs-function, .hljs-title, .hljs-title.function_ { color: #82aaff; }
  .hljs-params, .hljs-variable { color: #e9eef6; }
  .hljs-property, .hljs-attribute { color: #7dd3fc; }
  /* ---- tables ---- */
  table { font-size: .8em; }
  th { color: #7dd3fc; border-bottom: 2px solid #334155; }
  td, th { padding: .3em .7em; }
  ul { margin-top: .2em; }
  li { margin: .25em 0; }
  /* ---- mermaid ---- */
  .mermaid { text-align: center; margin-top: .3em; }
  .mermaid svg { max-height: 360px; width: auto; }
  /* ---- helpers ---- */
  section.lead { text-align: center; justify-content: center; }
  section.lead h1 { font-size: 2.4em; }
  .muted { color: #94a3b8; font-size: .85em; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 1.4em; }
  footer { color: #64748b; }
---

<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
  mermaid.initialize({
    startOnLoad: true,
    theme: 'dark',
    securityLevel: 'loose',
    themeVariables: {
      fontSize: '18px',
      primaryColor: '#1e293b',
      primaryTextColor: '#e9eef6',
      primaryBorderColor: '#7dd3fc',
      lineColor: '#94a3b8',
      secondaryColor: '#312e81',
      tertiaryColor: '#164e63'
    }
  });
</script>

<!-- _class: lead -->
<!-- _paginate: false -->

# vstimd
## Visual Stimulation Daemon

A frame-accurate visual stimulus server for
neuroscience & psychophysics experiments

<span class="muted">Joscha Schmiedt · University of Bremen · AGPLv3</span>

---

<!-- _class: lead -->

## The problem

Psychophysics needs stimuli shown at **exactly the right frame**,
with **known, low latency**, driven from **any experiment PC**.

General-purpose OSes, windowing systems, and compositors
fight all three of those goals.

---

## The idea

Split the two jobs onto two machines: your experiment logic stays on your PC;
a **dedicated stimulus PC** owns the display and nothing else.

<div class="mermaid">
flowchart LR
  subgraph exp["Experiment PC"]
    S["Your experiment<br/>Python · MATLAB · C# · PsychoPy"]
  end
  subgraph stim["Stimulus PC — dedicated"]
    V["vstimd<br/>Vulkan + KMS/DRM<br/>no compositor"]
  end
  S -- "ZMQ / TCP · protobuf" --> V
  V --> M["Monitor"]
</div>

---

## Goals

- **Frame-accurate timing** — vsync-locked render loop, DRM vblank wait, few skipped frames
- **Low, known latency** — dedicated device, no compositor in the path
- **Cross-platform clients** — Linux, Windows, macOS
- **Multiple API flavours** — PsychoPy, StimServer, Bonsai/C#, native
- **Deterministic event logging** — for experiment replay & analysis
- **No hardware lock-in** — TTL triggers via a shared-memory abstraction, not baked-in DAQ drivers

<span class="muted">Builds on ideas from M. Stephan's StimServer and A. Kreiter's VStim.</span>

---

## Architecture at a glance

<div class="mermaid">
flowchart LR
  C["Clients<br/>Python · MATLAB · C#"]
  subgraph server["vstimd server"]
    Z["ZMQ thread<br/>(tokio)"]
    S[("SceneState<br/>Arc&lt;RwLock&gt;")]
    R["Render thread<br/>Vulkan / DRM"]
    Z <--> S
    R <--> S
  end
  C -- "TCP:5555 · protobuf" --> Z
  R --> D["Display"]
</div>

- **Two threads, one shared `SceneState`.** ZMQ writes commands; the render thread tessellates + draws once per vsync.
- The write lock is released **before** the render pass → clients always get a window between frames.

---

## Why the timing is trustworthy

Per frame, in order:

```text
acquire swapchain image
 ├── deferred flip        ← atomically promote staged changes
 ├── tessellate dirty     ← CPU: lyon → triangles
 ├── upload GPU buffers
 ├── render pass (draw stimuli + optional overlay)
 ├── vkQueuePresentKHR
 └── vblank wait          ← DRM vblank / present_wait / display_timing
```

- **Deferred mode** batches many changes into a *single* atomic frame flip.
- **Live overlay (F1)** shows frame timing, stimulus list, and command log while you work.

---

## One protocol, many clients

**The protobuf schema in `proto/` is the single source of truth.**
Every client is generated from / speaks the same wire format.

| Client | Audience | Style |
|---|---|---|
| **Python** | general scripting | native `vstimd` package |
| **PsychoPy layer** | existing PsychoPy users | drop-in `visual.Window` / `Rect` |
| **MATLAB** | MATLAB labs | `vstimd.Connection` |
| **Web** | monitoring / control | TypeScript UI |
| **C# / Bonsai** | Bonsai workflows | planned |

Add a language → generate stubs, keep the same semantics.

---

## Native Python

```python
from vstimd import Connection
from vstimd.stimuli import Vec2, Color

with Connection("tcp://stimulus-pc:5555") as conn:
    h = conn.stimuli.shapes.create_rect(
        pos=Vec2(0, 0), width=200, height=100,
        color=Color(1.0, 0.0, 0.0))
    conn.stimuli.set_enabled(h, True)
    conn.stimuli.delete(h)

    info = conn.system.query_server_info()
    print(info.version)
```

- Namespaced API: `conn.stimuli`, `conn.system`, `conn.vtl`, `conn.animations`
- Handles (`h`) are server-assigned IDs you reuse to mutate/delete

---

## PsychoPy compatibility & MATLAB

<div class="cols">

**PsychoPy — drop-in**
```python
from vstimd.psychopy import visual

win = visual.Window(
    address="tcp://stimulus-pc:5555")
rect = visual.Rect(win,
    width=0.5, height=0.25,
    fillColor="red")
rect.draw()
win.flip()
```

**MATLAB**
```matlab
conn = vstimd.Connection( ...
   'tcp://stimulus-pc:5555');
h = conn.stimuli.create_rect( ...
   'x',0,'y',0, ...
   'width',200,'height',100, ...
   'r',1,'g',0,'b',0);
conn.stimuli.set_enabled(h,true);
conn.close();
```

</div>

Same server, same guarantees — meet users where they already are.

---

## Stimulus model

Stimuli are a **flat enum with composition** — no trait objects, no per-stimulus heap allocation.

| Type | Description |
|---|---|
| Rectangle | Axis-aligned fill + optional outline |
| Circle / Ellipse | Filled |
| Grating | Analytical sinusoid, aperture masks, drift |
| Text | Font, size, colour, anchor |
| Polygon | Arbitrary shape |

Shared fields live in components (`Transform2D`, `ShapeAppearance`, `StimulusFlags`)
composed into each variant.

---

<!-- _class: lead -->

# Part 2
## Virtual Trigger Lines & Animations

*the logic that makes stimuli react in hardware time*

---

## The trigger problem

Experiments must react to **TTL pulses** (e.g. from an NI-DAQ):
show a stimulus *on this pulse*, mark stimulus onset *on that line*.

Two bad options:
- ❌ Bake DAQ drivers into the stimulus server
- ❌ Round-trip every trigger back to the experiment PC (too slow)

**vstimd's answer: Virtual Trigger Lines (VTL).**

---

## Virtual Trigger Lines

A bank of trigger bits living in **POSIX shared memory**. A bridge process maps
real hardware lines onto the words; vstimd polls them once per frame.

<div class="mermaid">
flowchart LR
  H["Hardware DAQ<br/>daqd bridge"] -- writes --> M[("VTL banks<br/>shared memory<br/>64-bit words")]
  Q["ZMQ sim<br/>(no hardware)"] -- writes --> M
  M -- "poll @ frame start" --> L["vstimd<br/>render loop"]
  L -- "write @ frame end" --> M
</div>

- **Zero syscall overhead** per-frame polling — no DAQ code inside vstimd.
- No hardware? Simulate any line over ZMQ (`SetVirtualTriggerLine`).

---

## Input vs. output — clear ownership

Direction is part of every line's identity (input & output banks are independent).

<div class="cols">

**Input lines**
- Signals arriving *into* vstimd
- Written by **daqd** (or ZMQ sim)
- vstimd **never** writes them
- Read at **frame start** → rising/falling edges drive animations

**Output lines**
- Signals driven *by* vstimd
- Written by the **render loop only**
- Written at **frame end** (after present)
- Read back by daqd → hardware markers

</div>

<span class="muted">ZMQ writes to output lines exist for bench/debug override only.</span>

---

## VTL in the frame loop

```rust
// ── vblank N ──────────────────────────────────────────
write_outputs(prev);              // commit last frame's anim outputs
let input_edges = poll();         // drain rise/fall latches
let out_snapshot = snapshot();    // freeze outputs for triggers

advance_animations(input_edges, out_snapshot, &mut pending);
//  → animations update stimuli (visibility, position)
//  → accumulate output bits, run final actions

tessellate(); submit(); present();
prev = take(pending);             // save for next frame
// ── vblank N+1 ────────────────────────────────────────
```

- All animations advance in **one pass**; outputs commit together → order-independent.
- An output edge from anim A reaches anim B **one frame later** → predictable chaining.

---

## Animations: declarative reactions

An **animation** couples stimuli to time and/or trigger lines.
You *create → arm* it; the render thread runs it.

<div class="mermaid">
stateDiagram-v2
  [*] --> IDLE: create
  IDLE --> ARMED: arm
  ARMED --> RUNNING: start trigger / immediate
  RUNNING --> DONE: complete
  RUNNING --> DONE: cancel
  DONE --> ARMED: RESTART
</div>

- **ARMED** waits for a start trigger (or runs immediately) · **RUNNING** executes each frame · **DONE** won't fire again until re-armed.

---

## Animation variants

| Variant | What it does |
|---|---|
| `CoupleVisibilityToTriggerLine` | Visibility follows a line's level (runs forever) |
| `EnableOnTriggerEdge` | Flip enabled once on an edge |
| `FlashForNFrames` | Show for N frames |
| `FlickerForNFrames` | On/off cycling, optional total |
| `MoveAlongPath2D` | Play a preloaded per-frame position sequence |
| `MoveAlongSegments2D` | Piecewise-linear motion at constant speed |
| `ExternalPosition2D` | Read position from shared memory each frame |

One animation can drive **many stimuli** at once.

---

## Triggers & actions

Every animation shares an optional trigger + action vocabulary:

- **`start_trigger` + edge** — wait for this edge before running (else run on arm)
- **`cancel_trigger` + edge** — abort while armed/running
- **`start_action_mask`** — on Armed→Running: enable, toggle photodiode, pulse a line
- **`final_action_mask`** — on completion: disable, toggle PD, pulse a line, **restart**, restore state, end deferred
- **`cancel_action_mask`** — on cancel: same menu, always terminal

Triggers can watch **input *or* output** lines — that's the key to chaining.

---

## Chaining entirely inside the server

Because output edges are visible one frame later, animations compose —
no experiment-PC round trip:

<div class="mermaid">
sequenceDiagram
  participant A as Anim A (Flash)
  participant X as Output bit X
  participant B as Anim B
  A->>X: pulse on final action (frame N)
  Note over X: edge visible next frame
  X->>B: rising edge (frame N+1)
  B->>B: starts
</div>

Sequences of stimuli run in **hardware time**, synchronised to the display and to DAQ markers.

---

## Two-layer visibility

A subtle but important rule: **users and animations never silently clobber each other.**

<div class="mermaid">
flowchart LR
  U["user_enabled<br/>(SetEnabled — your commands)"] --> AND{"AND"}
  AN["anim_enabled<br/>(render thread — animations)"] --> AND
  AND --> V["Stimulus visible"]
</div>

- `user_enabled` is part of deferred mode; `anim_enabled` is owned exclusively by the render thread.
- `RESTORE_STATE` snapshots `user_enabled` at start and puts it back on completion → "flash and restore" without your script tracking prior state.

---

## Recap

- **vstimd** = dedicated, frame-accurate stimulus server; experiment logic stays on your PC.
- **One protobuf protocol**, many clients: Python, PsychoPy, MATLAB, web, C#.
- **VTL** = hardware triggers via shared memory — no DAQ code inside vstimd, zero-overhead per-frame polling.
- **Animations** = declarative, trigger-driven stimulus reactions with a rich start/final/cancel action vocabulary.
- Output-edge visibility rules let animations **chain in hardware time**, entirely server-side.

---

<!-- _class: lead -->

# Thank you

vstimd — frame-accurate stimuli, on your terms

<span class="muted">github.com/braemons/vstimd · AGPLv3 · © 2026 Joscha Schmiedt</span>
