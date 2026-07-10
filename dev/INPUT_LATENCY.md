# Input & Position Control: Latency Analysis and Design

> Companion document to `PLAN.md` — covers the specific question of how to handle
> high-rate position input (gaze, joystick, treadmill, mouse wheel) in the Rust application.

> **Status.** §1–§3 (the latency argument) and §7 (scope boundary) are unchanged and still
> govern. §4–§6 and §8 were rewritten once the actual device set became clear: input is **not**
> always an absolute position, and the shm layout must mirror the `vtl` crate rather than a bare
> `f32[2]`. §10–§14 were rewritten because the renderer is **hand-written Vulkan via `ash`**, not
> `wgpu`, and because hardware vblank timestamps now exist.
>
> For frame-timing measurement specifically, `FRAME_TIMING.md` is the authority; §11 here only
> covers what the *client* can observe over the protocol.
>
> `ExternalPosition2D` is currently a **stub** — the variant, its proto message and its config
> serde all exist, but `animation_advance.rs` never reads shm. Nothing in §5 is implemented yet.

---

## 1. The Problem Statement

Visual neuroscience experiments require stimulus position to track a continuously moving signal
(eye position, joystick, touch) with sub-frame latency. At 120 Hz the display budget per frame
is ~8.3 ms. At 240 Hz it is ~4.2 ms. Any position-update mechanism that adds more than one
frame of jitter is experimentally unacceptable because it introduces a variable, unmeasured delay
between the animal's action and the visual consequence.

The original C++ application solved this with `CAnimExternalPositionControl`: a Win32 named
shared-memory region is opened once, and every frame the render thread reads two `f32` values
directly from that region — no syscall, no copy, no serialisation, no round-trip.

The question is: **can ZeroMQ replace shared memory here, and what are the tradeoffs?**

---

## 2. Latency Budget Analysis

### 2.1 Shared Memory (current approach)

```
Producer writes (x,y)         ← happens any time, independently
      ↓
Render thread wakes (vsync)
      ↓  ~100 ns
ptr.read_volatile()           ← two f32 reads from L3 / RAM
      ↓
stimulus.move_to(x, y)
      ↓
frame drawn, displayed        ← worst case: 1 frame after write
```

**End-to-end latency:** input-to-display ≈ 0–1 frame (0–8.3 ms at 120 Hz).  
**Jitter:** < 1 µs (deterministic memory read).  
**CPU cost:** ~2 ns per frame (two f32 reads, likely cached).

### 2.2 ZeroMQ REQ/REP (the control channel — NOT suitable for position)

```
Producer sends Request (serialize → send → kernel → network stack)
      ↓  ~50–200 µs on localhost TCP
Server receives, deserializes
      ↓
SceneState locked, move_to applied
      ↓
Server sends Response
      ↓  ~50–200 µs
Producer receives ack
```

**Round-trip on localhost:** 100–400 µs typical, up to 1–2 ms under load.  
At 120 Hz the frame budget is 8333 µs. A 400 µs round-trip eats 5% of that budget and
introduces **1–3 frames of variable latency** depending on when in the frame the message
arrives.

**Verdict: ZeroMQ REQ/REP is not suitable for per-frame position updates.**

### 2.3 ZeroMQ PUB/SUB (fire-and-forget, no ack)

```
Producer publishes (x,y)      ← fire and forget, no blocking
      ↓  ~20–80 µs one-way on localhost TCP
Server subscriber receives
      ↓
move_to applied to stimulus
```

**One-way latency:** 20–80 µs typical.  
**Jitter:** 10–50 µs, occasionally spiking to 200+ µs under OS scheduling pressure.  
**At 120 Hz:** 80 µs = ~1% of frame budget. Usually lands within the same frame, but
jitter means it can miss by one frame unpredictably.

**Verdict: ZeroMQ PUB/SUB is *borderline* for 60–120 Hz if the experiment can tolerate
occasional 1-frame latency variance. Not suitable for 240 Hz or latency-critical paradigms.**

### 2.4 Summary Table

| Mechanism | Typical latency | Jitter | Cross-host | Suitable for per-frame |
|---|---|---|---|---|
| Shared memory (mmap/shm) | < 1 µs | < 1 µs | No | **Yes** |
| ZeroMQ PUB/SUB (localhost) | 20–80 µs | 10–50 µs | Yes | Borderline |
| ZeroMQ REQ/REP (localhost) | 100–400 µs | 50–200 µs | Yes | **No** |
| ZeroMQ PUB/SUB (LAN) | 100–500 µs | 50–300 µs | Yes | **No** |
| Unix domain socket (raw) | 5–20 µs | 2–10 µs | No | Maybe |
| TCP loopback (raw) | 20–60 µs | 10–30 µs | Same host | Borderline |

---

## 3. Recommended Architecture

Use **shared memory for all high-rate position streams** and **ZeroMQ only for low-rate
control commands** (create, destroy, configure stimuli). This is a clean separation of
concerns.

```
┌─────────────────────────────────────────────────────────┐
│                    vstimd                         │
│                                                         │
│  ZMQ REP thread          Render thread (main)           │
│  ┌──────────────┐        ┌──────────────────────────┐   │
│  │ Create rect  │──────▶ │ SceneState (RwLock)      │   │
│  │ Set colour   │        │  stimuli, animations     │   │
│  │ Set enabled  │        └──────────┬───────────────┘   │
│  │ Deferred mode│                   │ per frame         │
│  └──────────────┘                   ▼                   │
│                        advance_animations()             │
│                            └─ device.read() ───────────▶│
│                                              draw frame │
│  ┌──────────────────────────────────────────────────┐   │
│  │  InputDevice registry  (Arc, opened once)        │   │
│  │   "gaze"      → Shm("/vstimd_gaze")   Absolute   │   │
│  │   "treadmill" → Shm("/vstimd_wheel")  Cumulative │   │
│  │   "debug"     → Gamepad{0} / Keyboard (in-proc)  │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
         ▲                       ▲
         │ protobuf/ZMQ           │ mmap write (vinput crate)
   experiment                eye tracker /
   control script            wheel reader /
   (Python, MATLAB)          DAQ process
```

Shared memory is the transport for **cross-process** sources. In-process sources (gamepad,
keyboard) fill the same `InputDevice` directly — see §5.1.

---

## 4. The `DirectionalInputDevice` Shared-Memory Contract

### 4.1 Not all input is a position

The original design here assumed every input source is an **absolute position** — an eye
tracker's gaze point, written as `f32[2]`. That is one of three semantics, and the other two
matter more for navigation:

| Semantic | Producer writes | vstimd does | Example |
|---|---|---|---|
| `Absolute` | current value | use directly | eye tracker, joystick position |
| `Cumulative` | running total, never reset | **differences** it against last frame | mouse wheel, rotary encoder, treadmill |
| `Rate` | current velocity | multiplies by frame Δt | gamepad stick, held key |

**A wheel or encoder must publish a cumulative accumulator, never a per-write delta.** If the
producer writes "ticks since last write", the reader has to consume-and-clear, which races with
the next write, and any missed read silently loses distance. With a cumulative counter the reader
just computes `now - last_seen`: nothing is ever lost or double-counted, no matter how the
producer's write rate and the render thread's 60–240 Hz read rate interleave, and a dropped frame
merely yields a larger delta next frame.

This is the same reason quadrature encoders expose counts rather than deltas.

### 4.2 Mirror the `vtl` crate; do not add a second shm mechanism

The `vtl` crate (`vtl/src/{layout,segment,owner,client}.rs`) already solves shared memory for this
project: `shm_open` + `mmap`, a `#[repr(C)]` header with `MAGIC` and `VERSION`, an `AtomicU64`
state section, an owner/client split, and documented `Send`/`Sync` safety. It is used in
production for trigger lines.

The earlier recommendation here — the `shared_memory` crate 0.12 — was never adopted and should
not be. A new sibling crate mirrors `vtl`:

> **Crate name.** It cannot be `input`: the server already depends on the `input` crate (libinput)
> for DRM keyboard handling. Use `vinput`.

Producers link `vinput` directly (Rust), or speak the documented layout from Python/C.

### 4.3 Layout

```
offset 0      VinputHeader   magic "VIN1", version, n_axes, device name
offset 128    AxisDesc[N]    per-axis: name, semantic, unit scale, deadzone
offset 0x1000 StateSection   seqlock + [f64; N] values + writer heartbeat
```

Three things the old `f32[2]` layout lacked, each of which is load-bearing:

**A seqlock**, because a multi-axis read must be a *coherent snapshot*. Reading `x` from before a
producer write and `y` from after it yields a position that never existed. The producer bumps an
`AtomicU32` sequence before and after writing (odd = write in progress); the reader retries while
the sequence is odd or changed across the read. Cost is a handful of nanoseconds. Note `vtl` does
*not* need this — each of its banks is an independent `AtomicU64` with no cross-bank invariant.
This does.

The reader's retry loop runs on the render thread and **must be bounded** — spin a fixed number of
times, then reuse the previous snapshot and count a `torn_read` stat. Never loop unbounded on the
render thread; a producer that is `SIGSTOP`ed mid-write would otherwise hang the display.

**`f64` values**, not `f32`. A treadmill accumulator running for a kilometre at cm resolution
exhausts `f32`'s 24-bit mantissa (~1 mm resolution at 10 000 cm, worse beyond). The *derived*
per-frame delta is small and converts to `f32` safely; the accumulator must not.

**A writer heartbeat** — a monotonic `AtomicU64` nanosecond timestamp the producer refreshes on
every write. This is a safety requirement, not a nicety:

> If the treadmill process crashes while the animal is running, a `Rate` axis holding its last
> value would drive the camera forward **forever**, silently, for the rest of the session.

So: if the heartbeat has not advanced for longer than `stale_after_ms` (rig-config, default
100 ms), vstimd treats `Rate` axes as **zero**, freezes `Cumulative` axes at their last value, and
holds `Absolute` axes at their last sample. It logs once on transition and exposes the stale flag
in the overlay and in `QueryAnimation`. It does **not** disable the animation or error the frame —
the experiment keeps running with the stimulus stationary, which is the safe failure mode.

### 4.4 Producer side

**Python** (via the client shipped with this work; closes #15):
```python
from vstimd.shm import InputDevice, Semantic

dev = InputDevice.create("/vstimd_wheel", axes=[("distance", Semantic.CUMULATIVE, 1.0)])
total = 0.0
for ticks in read_serial_wheel():
    total += ticks          # never reset, never a delta
    dev.write([total])      # bumps seq + heartbeat
```

**Rust:** depend on `vinput` and use `VinputOwner`, mirroring `VtlOwner`.

### 4.5 Consumer side (render thread)

Reads are atomic loads against an already-mapped region: no syscall, no allocation, no lock. The
mapping is established on the **ZMQ thread** when the animation is created or the config is
loaded — never inside `advance()`, which runs on the render thread and must not block or allocate
(`CLAUDE.md`).

Devices are opened once and shared: `SceneState.runtime.input_devices: HashMap<String,
Arc<InputDevice>>`, refcounted, so two animations naming `/vstimd_wheel` map it once. This map is
runtime state and is **not** serialized into the config JSON — the animation stores the device
*name*, and the mapping is re-established on load (same split as `GratingStimulus::phase_accum`).

---

## 5. Animation Types for Position Control

> Earlier revisions of this section described `AnimExternalPos`, `AnimZmqPos`, `AnimMousePos` and
> `AnimGamepadPos`, sketched as `impl Animation for …` over `Box<dyn Stimulus>`. **None of those
> exist**, and trait objects are explicitly rejected (`CLAUDE.md`: flat enums, no trait objects).
> The real type is the `Animation` enum in `scene/animation/animation_kind.rs`.

### 5.1 Shared memory is a *backend*, not the interface

The abstraction vstimd exposes is an **input device with N axes**. Where the axis values come from
is a backend detail:

| Backend | Mechanism | Use |
|---|---|---|
| `Shm(name)` | `vinput` mmap (§4) | **Production.** Treadmill, mouse wheel, eye tracker, joystick |
| `Gamepad { id }` | `gilrs`, background thread, in-process | Debug override — no shm |
| `Keyboard` | existing `drm_keyboard_input.rs` / winit events | Debug override — no shm |

Routing a gamepad through shared memory would be pure ceremony: it is already in-process, and the
latency argument in §2 is about crossing a *process* boundary. The gamepad and keyboard backends
fill the same `InputDevice` axis array directly.

Because the backends are interchangeable, a **debug override** falls out for free:
`--input-override /vstimd_wheel=gamepad:0` substitutes a gamepad for a named shm device, letting a
full experiment config run on a desk with no rig hardware attached. The overlay shows which
backend is live.

### 5.2 `ExternalPosition2D` — absolute position (currently a stub)

```rust
ExternalPosition2D { shm_name: String, x_offset: f32, y_offset: f32 }
```

The variant, its proto message and its config serde exist. **`animation_advance.rs` never reads
shm** — the match arm returns `false` and nothing moves. Implementing it means mapping the device
on create and calling `move_to` from two `Absolute` axes. Direct descendant of
`CAnimExternalPositionControl`.

### 5.3 `DeviceDrivenTransform` — the general mapping

Maps device axes onto transform channels of the animation's target (`AnimationTarget::Stimuli` or
`::Camera`). This is what "move / rotate / scale a stimulus or the camera from a wheel" means.

```rust
DeviceDrivenTransform {
    device: String,          // logical name; resolved via the rig config
    axes:   Vec<AxisMap>,
}

pub struct AxisMap {
    axis:     u8,                  // index into the device's axes
    channel:  TransformChannel,
    gain:     f32,                 // device units → cm or degrees
    deadzone: f32,                 // sticks; ignored for Cumulative
    invert:   bool,
    clamp:    Option<[f32; 2]>,
    wrap:     Option<f32>,         // corridor period, cm
}

pub enum TransformChannel {
    PosX, PosY, PosZ,
    Yaw, Pitch, Roll,
    ScaleX, ScaleY, ScaleZ, ScaleUniform,
    Forward, Strafe,               // camera-local; only with AnimationTarget::Camera
}
```

The axis's **semantic** (§4.1) decides how its value becomes a channel update — `Absolute` sets,
`Cumulative` differences, `Rate` integrates over the frame Δt. The `AxisMap` does not restate it;
the device declares it once.

### 5.4 `LinearNav3D` takes its speed from a device, not from `SetAnimParam`

An earlier draft of the 3-D roadmap had `LinearNav3D` reading `speed_cm_s` updated **every frame**
via a `SetAnimParam` ZMQ command. §2.2 of this document rules that out directly: REQ/REP is
100–400 µs with 1–3 frames of jitter, and it is the exact anti-pattern this whole document exists
to prevent. A treadmill must not be a ZMQ command stream.

```rust
LinearNav3D {
    source:         AxisRef,        // device + axis; Rate or Cumulative
    wrap_period_cm: Option<f32>,
}
```

`SetAnimParam` remains valid for **low-rate, scripted** speed changes (a block-design "now run at
20 cm/s"), which is a different thing from per-frame hardware input.

`LinearNav3D` keeps the two-value discipline: a monotonic `f64 distance_travelled_cm` for the
experiment log, and a wrapped `f32` camera position for the renderer. See `3D_ROADMAP.md` §11.2.

### 5.5 Deliberately not built

- **`AnimZmqPos`** (ZMQ SUB from a remote producer). §2.3 rates PUB/SUB as *borderline* at 60–120 Hz
  and unsuitable above. No current experiment needs a cross-host position source. Reconsider only
  with a concrete requirement; the `Rate` staleness rule (§4.3) would need a network-loss story.
- **`AnimMousePos`** (winit `CursorMoved`). Subsumed by the `Keyboard`/`Gamepad` debug backends,
  and a desktop mouse cursor is not a stimulus-space position. If wanted, it is a fourth backend,
  not a fourth animation type.

---

## 6. Declaring Input Devices

POSIX shm names take a leading `/`, by convention prefixed `vstimd_`:

| Source | shm name | Axes |
|---|---|---|
| Eye tracker (gaze) | `/vstimd_gaze` | `x`, `y` — `Absolute`, screen px, centre = 0 |
| Treadmill / mouse wheel | `/vstimd_wheel` | `distance` — `Cumulative`, encoder counts |
| Joystick / lever | `/vstimd_joystick` | `x`, `y` — `Absolute`, normalised −1..1 |
| Custom DAQ | `/vstimd_daq` | user-defined |

**Devices are declared in the rig config, not in the animation.** Calibration (counts → cm), axis
semantics and staleness thresholds are properties of the *hardware on this rig*, and belong
alongside the existing `[vtl]` section in `rig-config.toml` — not embedded in every animation that
happens to use the device, and not re-sent by every experiment script.

```toml
[[input.device]]
name           = "treadmill"        # logical name; animations reference this
shm            = "/vstimd_wheel"
stale_after_ms = 100

  [[input.device.axis]]
  name     = "distance"
  semantic = "cumulative"
  scale    = 0.0127                 # counts → cm (encoder-specific)
```

Animations then say `device = "treadmill"`, and a rig with a different encoder needs no script
change. This mirrors how VTL lines get names rather than raw bank/bit indices.

`ExternalPosition2D` keeps its literal `shm_name` field for backward compatibility with its
existing proto message; new work uses the logical-name path.

---

## 7. What vstimd Should NOT Own

To keep the render server simple and its dependencies minimal:

- **Device drivers**: eye tracker SDKs (EyeLink, Tobii, SR Research) stay in their own
  process. They write to shared memory.
- **Calibration**: gaze calibration is the responsibility of the tracking process or
  the experiment control script. vstimd receives already-calibrated screen coordinates.
- **Input recording**: timestamped logging of gaze/joystick traces belongs in the
  experiment control software, not the render server.
- **Synchronisation signals**: photodiode flash output is handled by the server (see PLAN.md §10).
  Reward delivery, TTL pulses, and other DAQ output belong in a separate DAQ process.

---

## 8. Decision Summary

| Scenario | Recommended mechanism |
|---|---|
| Eye tracker / joystick on same host | `ExternalPosition2D` — shm device, `Absolute` axes |
| Treadmill or mouse wheel | `LinearNav3D` — shm device, `Cumulative` axis (§5.4) |
| Wheel driving a 2-D stimulus, or rotate/scale | `DeviceDrivenTransform` — shm device (§5.3) |
| Testing / demo with gamepad | same animation, `--input-override …=gamepad:0` |
| Testing / demo with keyboard | same animation, `--input-override …=keyboard` |
| Eye tracker on another host | Not supported. See §5.5 |
| Script-driven one-off moves | ZMQ `SetPosition` (not per-frame) |
| Scripted speed changes (block design) | `SetAnimParam` — low-rate only, never per-frame |
| Smooth scripted trajectories | `MoveAlongPath2D` / `MoveAlongSegments2D` (preloaded) |

Note the debug rows change **only the `--input-override` flag**, not the animation, not the
config, not the experiment script. That is the point of making shm a backend rather than the
interface.

**Short answer to the original question:**
ZeroMQ is fast enough for everything *except* per-frame position tracking at high refresh rates.
Keep shared memory for that. The hybrid model (ZMQ for commands, shared memory for streaming
position) gives the best of both worlds with no measurable overhead.

---

## 9. Render-Loop Latency: Implementation Notes

The sections below cover the render-side contributors to end-to-end latency that are not
specific to position input. They complement the command-to-photon breakdown in `PLAN.md §3.4`.

---

## 10. Present Mode and Swap Chain Strategy

### 10.1 Mode comparison

The renderer is `ash`, so these are `vk::PresentModeKHR` values, not `wgpu::PresentMode`.

| `vk::PresentModeKHR` | Tearing | Latency vs FIFO | Flip timing accuracy | Use case |
|---|---|---|---|---|
| `FIFO` | Never | baseline | Highest — blocks on vsync | **Production default** |
| `FIFO_RELAXED` | On late frame | baseline | Tears only when already late | Never — hides dropped frames |
| `MAILBOX` | Never | −0.5 to −1 frame | Lower — newest rendered frame shown | Benchmarking only |
| `IMMEDIATE` | Yes | Minimal | N/A | Never appropriate for stimuli |

For psychophysics, **`FIFO` is mandatory in production**. The experiment software must be able to
reason about which display frame a deferred flip was visible on. `MAILBOX` destroys that
guarantee, and `FIFO_RELAXED` is worse than either: it silently converts a dropped frame into a
tear, hiding exactly the event the timing analysis needs to see.

**Implemented.** `select_present_mode()` (`render/vk/vk_context.rs:196`) takes a preference list
and falls back to `FIFO`, which the Vulkan spec guarantees is always available. The swapchain is
created with `FIFO` (`vk_context.rs:376`).

### 10.2 CLI flag — not implemented

There is no `--present-mode` flag, and the server does **not** use `clap` (arguments are parsed by
hand in `main.rs`). Exposing the mode for benchmarking means adding a flag to that hand-rolled
parser and threading it into `select_present_mode`'s preference list.

If added: default `fifo`, and the README must state that changing it invalidates every timing
measurement.

### 10.3 Number of swapchain images

`wgpu`'s `desired_maximum_frame_latency` has no direct Vulkan equivalent — you control queue depth
through the swapchain image count and how many frames you allow in flight.

**Implemented.** `vk_context.rs:592` requests `image_count = 2`, clamped to the surface's
`min_image_count` / `max_image_count`. Double buffering with `FIFO` means the CPU blocks on
acquire once the GPU is one frame ahead, which is the low-latency behaviour that
`desired_maximum_frame_latency = 1` was asking for.

The GPU will occasionally stall waiting for the previous frame to be presented. At 120 Hz that
stall is ≤ 8.3 ms and is dwarfed by the vsync wait. The latency saving is worth it.

---

## 11. Flip Timestamp Reporting

### 11.1 The problem

The original C++ server answered `CmdQueryTimestamp` with a value from
`IDXGISwapChain1::GetFrameStatistics::SyncQPCTime` — the hardware-latched presentation counter for
the most recent flip. Clients used this to correlate stimulus events with electrophysiology
recordings.

An earlier revision of this section said "wgpu does not expose swap-chain presentation timestamps.
This must be emulated." **That is obsolete on two counts.** The renderer is `ash`, not wgpu; and
the DRM backend already obtains real, hardware-latched vblank timestamps.

### 11.2 What exists today

- `render/drm/drm_vblank.rs` registers a fence before present and collects the vblank timestamp
  afterwards, with an explicit guard for a clock that dies at runtime.
- `timing.rs::FrameTick` carries `vblank_time: Instant` — described in-tree as *"the best available
  proxy for the vblank that confirmed the stimulus"* — plus a count of extra vblanks elapsed, i.e.
  **dropped-frame detection**.
- `timing.rs::FrameStats` / `FrameSummary` aggregate these. `FRAME_TIMING.md` marks this Layer 1 as
  **IMPLEMENTED**, and the egui HUD (Layer 2) as implemented.

So the server has a better timestamp than any of the three fallbacks this section used to propose.
`FRAME_TIMING.md` is the authority on measurement; the open question is purely what the *client*
can see.

### 11.3 The gap: none of it is exposed over the protocol

There is **no timestamp field anywhere in `proto/`**. `QueryServerInfoResponse` carries `width`,
`height`, `frame_rate`, `background_color`, `backend` and `version` — no frame index, no vblank
time. `WaitForFramesRequest` and `WaitUntilRequest` let a client *synchronise* to frames, but never
learn when one actually happened.

A client therefore cannot align a stimulus onset to an ephys recording without a photodiode. That
is the real work item, and it is protocol work, not rendering work:

- Expose `frame_index: u64` (already tracked as `SceneRuntimeState.frame_count`).
- Expose the last flip's `vblank_time` as nanoseconds against a stated clock
  (`CLOCK_MONOTONIC`), plus `session_start` so absolute reconstruction is possible.
- State the clock domain explicitly in the `.proto` comment. A timestamp whose epoch the client has
  to guess is worse than none.

`FRAME_TIMING.md` Phase 3 ("ZMQ PUB frame events") is the streaming version of the same data and
should share these field definitions rather than inventing parallel ones.

### 11.4 GPU timestamp queries (only if sub-vblank attribution is needed)

Vulkan exposes `vkCmdWriteTimestamp` into a `VkQueryPool`, converted to nanoseconds via
`VkPhysicalDeviceLimits::timestampPeriod`. This latches when *rendering* finished, not when the
frame was *presented*.

Given a real vblank timestamp (§11.2), this adds little for stimulus timing — it answers "when did
the GPU finish drawing", which matters for diagnosing missed frames, not for knowing when the
animal saw something. Implement only when profiling the render path, and prefer the existing
`FramePhases` breakdown in `timing.rs` first.

### 11.5 Recommendation

The measurement side is done. **Ship §11.3** — expose `frame_index` and the vblank timestamp
through `QueryServerInfo` or a dedicated query, with an explicit clock domain. Everything else in
this section is already handled by `FRAME_TIMING.md`.
---

## 12. RwLock Contention and the Mid-Frame Command Problem

### 12.1 The problem

```
Frame N:                          Frame N+1:
  render thread acquires WRITE lock
  flip / advance animations / tessellate   ← ZMQ command arrives HERE
  release write lock, acquire READ lock
  record command buffer, submit
  present (blocks on vsync)
  release read lock               ← ZMQ server NOW acquires write lock
                                  next frame sees the change
```

(The render thread takes the **write** lock first — `apply_flip`, animation advance and
tessellation all mutate `SceneState` — then downgrades to a read lock to record the draw. See
`render_frame.rs` and the threading note in `CLAUDE.md`. The write lock is released before the
read lock is taken, so the ZMQ thread always has a window between frames.)

A command that arrives while the render thread holds the read lock on `SceneState` is
processed *after* the current frame is already submitted. The change is not visible until
frame N+2 — one extra frame of silent latency with no error signal to the client.

### 12.2 Why this is acceptable

This matches the behaviour of the original C++ system when a command arrived during the
display thread's critical section: the pipe thread would block on `g_criticalDrawSection`
and the change would land in the next frame. The original system documented this as
"commands may be delayed by up to one frame". The Rust port inherits the same guarantee.

For experiments where exact-frame delivery is required, the client must use **deferred
mode**: send all commands between `DeferredMode{start: true}` and `DeferredMode{start:
false}`, then the server atomically promotes all changes on the next `pending_flip` frame.

### 12.3 Mitigation if sub-frame latency becomes critical

If experiments require commands to land in the *same* frame they are received, consider a
**lock-free double-buffer** for `SceneState`:

- The ZMQ thread writes to a staging state (no lock, only atomic pointer swap).
- At the start of each frame the render thread atomically adopts the staging state.
- Deferred mode is always-on at the hardware level; the client chooses when to "commit".

This adds significant complexity. Only implement if the one-frame RwLock delay is measured
to be experimentally significant.

---

## 13. Thread Priority

### 13.1 Why it matters

Without elevated priority, the OS scheduler can preempt the render thread for 1–15 ms
(Linux CFS default timeslice) while a background process consumes CPU. At 120 Hz, 15 ms
is nearly two full frames — a dropped frame, a visible stutter, and a timing artefact.

### 13.2 Linux

Use `SCHED_FIFO` with priority 50 on the render (main) thread. This requires either
running vstimd as root or setting the `CAP_SYS_NICE` capability:

```rust
// In main(), after the event loop is created but before run_app():
#[cfg(target_os = "linux")]
unsafe {
    let mut param = libc::sched_param { sched_priority: 50 };
    let ret = libc::pthread_setschedparam(
        libc::pthread_self(),
        libc::SCHED_FIFO,
        &param,
    );
    if ret != 0 { eprintln!("Warning: could not set SCHED_FIFO ({}). Run as root or set CAP_SYS_NICE.", ret); }
}
```

Grant the capability without running as root:
```bash
sudo setcap cap_sys_nice+eip ./target/release/vstimd
```

The ZMQ server thread and messenger thread should stay at `SCHED_OTHER` (default). Only
the render thread needs elevated priority.

**Status: not implemented.** No `SCHED_FIFO` or `pthread_setschedparam` call exists in the tree.
Tracked as #27.

### 13.3 Other platforms

**vstimd is Linux-only.** An earlier revision of this section carried a Windows
`SetThreadPriority` snippet and suggested reusing "the `windows-sys` crate already pulled in by
wgpu's dependency tree". There is no `target_os = "windows"` code anywhere in the tree, neither
`wgpu` nor `windows-sys` is a dependency, and the server hard-depends on `drm`, `sd-notify` and
`wayland-client` and ships as a `.deb`.

### 13.4 CPU affinity

Pin the render thread to a dedicated core to eliminate cache-invalidation jitter from other
threads migrating onto the same core.

**Status: half-built.** `rig_config.rs` already parses a `[scheduling]` section with
`render_cpu_core: Option<usize>`, and its own doc comment states the fields are *"parsed but not
yet applied"*. The remaining work is to apply it — not to decide whether to add a dependency.
Tracked as #65.

---

## 14. VRR / GSync / FreeSync

Variable refresh rate technology (NVIDIA G-Sync, AMD FreeSync, VESA AdaptiveSync) allows
the display to vary its refresh period to match the GPU's output rate. This is beneficial
for gaming (eliminates tearing without fixed-rate vsync stalls) but harmful for
psychophysics:

- **Non-deterministic frame duration**: the display may hold a frame for 6 ms or 14 ms
  depending on GPU load. Stimulus onset time relative to the physical frame boundary
  becomes unknown.
- **Timing analysis breaks**: inter-stimulus intervals computed from frame indices assume
  a fixed frame period. VRR violates this assumption.
- **Photodiode timing misleads**: the photodiode flash appears for a variable duration,
  making photodiode-based electrophysiology alignment unreliable.

**Recommendation: disable VRR on the stimulus display unconditionally.**

| Mode | How to disable |
|---|---|
| KMS/DRM (production, `--drm`) | Set the connector's `VRR_ENABLED` property to 0. vstimd already owns the connector in this mode, so it can do this itself. |
| Wayland / desktop (development) | Compositor display settings → Adaptive Sync → Off |
| X11 (development) | `xrandr --output <display> --set "vrr_capable" 0` |

Add VRR-off to the deployment checklist in the README. Two startup checks worth adding:

- In DRM mode, read the connector's `VRR_ENABLED` property and **clear it** (or refuse to start
  and say why). This is strictly better than documenting a manual step, because vstimd is already
  the DRM master.
- Log a warning if `select_present_mode` finds that `FIFO` is unavailable — the Vulkan spec
  guarantees it, so its absence indicates something badly wrong with the driver or the display
  path.

---

*End of document. See `PLAN.md` for the full porting plan and §3.4 for the latency budget summary.*