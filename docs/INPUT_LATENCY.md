# Input & Position Control: Latency Analysis and Design

> Companion document to `PLAN.md` — covers the specific question of how to handle
> high-rate position input (gaze, joystick, mouse) in the ported Rust application.

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
│                    vstim_server                         │
│                                                         │
│  ZMQ REP thread          Render thread (main)           │
│  ┌──────────────┐        ┌──────────────────────────┐   │
│  │ Create rect  │──────▶ │ SceneState (RwLock)      │   │
│  │ Set colour   │        │  stimuli, animations     │   │
│  │ Set enabled  │        └──────────┬───────────────┘   │
│  │ Deferred mode│                   │ per frame         │
│  └──────────────┘                   ▼                   │
│                           AnimExternalPos               │
│                            └─ shm.read() ──────────────▶│
│                                              draw frame │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Shared memory regions (one per position source) │   │
│  │  /vstim/gaze_pos    → f32[2]                     │   │
│  │  /vstim/joystick    → f32[2]                     │   │
│  │  /vstim/cursor      → f32[2]                     │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
         ▲                       ▲
         │ protobuf/ZMQ           │ mmap write
   experiment                eye tracker /
   control script            joystick server /
   (Python, MATLAB)          DAQ process
```

---

## 4. Shared Memory on Linux

On Linux, `shm_open` + `mmap` provides the same semantics as Win32 `CreateFileMapping`.
The `shared_memory` crate (version 0.12) wraps both platforms transparently.

### Producer side (any language)

**Python:**
```python
import mmap, struct, posix_ipc

shm = posix_ipc.SharedMemory("/vstim_gaze", posix_ipc.O_CREAT, size=8)
mm  = mmap.mmap(shm.fd, 8)

def write_pos(x: float, y: float):
    mm.seek(0)
    mm.write(struct.pack('ff', x, y))
```

**C/C++:**
```c
int fd = shm_open("/vstim_gaze", O_RDWR | O_CREAT, 0666);
ftruncate(fd, 8);
float *pos = mmap(NULL, 8, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);
pos[0] = x;
pos[1] = y;
```

### Consumer side (Rust, in vstim_server)

```rust
use shared_memory::{Shmem, ShmemConf};

pub struct ShmPositionReader {
    _shmem: Shmem,          // keeps the mapping alive
    ptr:    *const [f32; 2],
    offset: (f32, f32),
}

impl ShmPositionReader {
    pub fn open(name: &str, offset: (f32, f32)) -> anyhow::Result<Self> {
        let shmem = ShmemConf::new().os_id(name).open()?;
        let ptr   = shmem.as_ptr() as *const [f32; 2];
        Ok(Self { _shmem: shmem, ptr, offset })
    }

    /// Called every frame from the render thread. No allocation, no syscall.
    pub fn read(&self) -> (f32, f32) {
        // SAFETY: producer writes two f32 atomically enough for display purposes.
        // For strict correctness, use atomic f32 or a seqlock if needed.
        let raw = unsafe { self.ptr.read_volatile() };
        (raw[0] + self.offset.0, raw[1] + self.offset.1)
    }
}

// Safety: the pointer is valid for the lifetime of _shmem.
// We never write through it, only read.
unsafe impl Send for ShmPositionReader {}
```

### Atomicity note

Two `f32` writes from one process and reads from another are not strictly atomic. In practice,
on x86-64/ARM64 aligned 4-byte reads are naturally atomic at the hardware level. For rigorously
correct code, use a **seqlock** pattern (writer increments a counter before and after the write;
reader retries if the counter changes during the read). This adds ~5 ns overhead. Implement this
if the experiment uses 64-bit position values or if tearing artefacts are observed.

---

## 5. Animation Types for Position Control

### 5.1 `AnimExternalPos` (shared memory — existing, ported)

Direct port of `CAnimExternalPositionControl`.

```rust
pub struct AnimExternalPos {
    reader:           ShmPositionReader,
    stimulus_handle:  Option<u32>,
    final_action:     FinalActionMask,
}

impl Animation for AnimExternalPos {
    fn advance(&mut self, stimuli: &mut IndexMap<u32, Box<dyn Stimulus>>, _fr: f32, _def: bool) {
        let (x, y) = self.reader.read();
        if let Some(h) = self.stimulus_handle {
            if let Some(s) = stimuli.get_mut(&h) {
                s.move_to(false, x, y);
            }
        }
    }
}
```

Created via: `CmdCreateAnimExternalPos { shm_name: "/vstim_gaze", offset: {x:0, y:0} }`

### 5.2 `AnimZmqPos` (ZeroMQ SUB — new, for remote producers)

For cases where the position producer is on a different machine and 1-frame latency is
acceptable (e.g. slow-moving stimuli, non-latency-critical paradigms).

```rust
pub struct AnimZmqPos {
    // Latest position received from the subscriber background task.
    // Written by the ZMQ receiver task, read by the render thread.
    latest:          Arc<AtomicCell<(f32, f32)>>,
    stimulus_handle: Option<u32>,
    final_action:    FinalActionMask,
}
```

The ZMQ SUB task runs on the tokio runtime and updates `latest` whenever a new message
arrives. `advance()` reads `latest.load()` — a single atomic 64-bit read on 64-bit platforms
(two f32 packed into a u64).

Created via: `CmdCreateAnimZmqPos { sub_addr: "tcp://192.168.1.5:5556", offset: {x:0, y:0} }`

**Note:** requires the producer to speak ZMQ PUB and publish raw `[f32; 2]` (or a small
protobuf `Vec2` message, configurable).

### 5.3 `AnimMousePos` (winit events — for testing)

Populated from winit `CursorMoved` events in the main thread event loop. No extra thread.
Useful for demos and debugging without hardware.

```rust
// In App::window_event():
WindowEvent::CursorMoved { position, .. } => {
    // Convert from winit physical pixels (top-left origin) to
    // stimulus space (centre origin, Y-up)
    let x =  position.x as f32 - (screen_w / 2) as f32;
    let y = -position.y as f32 + (screen_h / 2) as f32;
    scene.write().unwrap().set_mouse_pos(x, y);
}
```

Created via: `CmdCreateAnimMouse { scale: {x:1,y:1}, offset: {x:0,y:0} }`

### 5.4 `AnimGamepadPos` (gilrs — for testing)

A background thread runs `gilrs::Gilrs::next_event()` in a loop, updates an `AtomicCell<(f32, f32)>`
per gamepad axis pair. `advance()` reads it, same as `AnimZmqPos`.

Requires Cargo feature `gamepad = ["dep:gilrs"]`.

Created via: `CmdCreateAnimGamepad { gamepad_id: 0, axis_x: 0, axis_y: 1, scale: {x:400,y:400} }`

---

## 6. Configuring Shared Memory Names

By convention, use POSIX-style names with a leading `/`:

| Source | Shared memory name | Contents |
|---|---|---|
| Eye tracker (gaze) | `/vstim_gaze` | `f32[2]`: (x, y) in screen pixels, centre=0 |
| Joystick / lever | `/vstim_joystick` | `f32[2]`: (x, y), normalised −1..1 or pixels |
| Second eye | `/vstim_gaze2` | `f32[2]` |
| Custom DAQ | `/vstim_daq` | user-defined, `f32[N]` |

The stimulus server does not impose a naming convention — the `shm_name` field in
`CmdCreateAnimExternalPos` is arbitrary. The convention above is a recommendation for
interoperability between experiment software components.

---

## 7. What vstim_server Should NOT Own

To keep the render server simple and its dependencies minimal:

- **Device drivers**: eye tracker SDKs (EyeLink, Tobii, SR Research) stay in their own
  process. They write to shared memory.
- **Calibration**: gaze calibration is the responsibility of the tracking process or
  the experiment control script. vstim_server receives already-calibrated screen coordinates.
- **Input recording**: timestamped logging of gaze/joystick traces belongs in the
  experiment control software, not the render server.
- **Synchronisation signals**: photodiode flash output is handled by the server (see PLAN.md §10).
  Reward delivery, TTL pulses, and other DAQ output belong in a separate DAQ process.

---

## 8. Decision Summary

| Scenario | Recommended mechanism |
|---|---|
| Eye tracker on same host | `AnimExternalPos` — shared memory |
| Eye tracker on LAN, latency < 2 frames OK | `AnimZmqPos` — ZMQ SUB |
| Joystick on same host | `AnimExternalPos` — shared memory |
| Testing / demo (no hardware) | `AnimMousePos` — winit cursor events |
| Testing / demo with gamepad | `AnimGamepadPos` — gilrs |
| Script-driven position sequences | ZMQ REQ/REP control command `CmdMoveTo` (not per-frame) |
| Smooth scripted trajectories | `AnimPath` or `AnimLineSeg` (preloaded into server) |

**Short answer to the original question:**  
ZeroMQ is fast enough for everything *except* per-frame position tracking at high refresh rates.
Keep shared memory for that. The hybrid model (ZMQ for commands, shared memory for streaming
position) gives the best of both worlds with no measurable overhead.

---

*End of document. See `PLAN.md` for the full porting plan.*