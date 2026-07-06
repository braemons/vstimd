# Frame Timing Verification

Precise frame timing is essential for neuroscience experiments — every skipped
frame is a measurement error.  Unit tests cannot catch timing failures caused
by OS jitter, GPU scheduling, or display misconfiguration.  This document
describes the timing verification built into the `vstimd` project: two
implemented layers (always-on Rust `FrameStats` and an egui HUD overlay), plus
planned phases for higher-precision hardware-in-the-loop measurement.

---

## What is measured

The primary metric is **render-to-photon latency**: the elapsed time from when
the server calls `wgpu::SurfaceTexture::present()` to when photons appear on
screen.  This is the quantity that matters for neuroscience — not the
round-trip ZMQ command latency (which adds ≥ 1 frame by design in deferred
mode).

Secondary metrics:

| Metric | Description |
|---|---|
| `ipi_std_ms` | Jitter — standard deviation of inter-presentation intervals |
| `dropped_count` | Frames where the server missed its presentation deadline |
| `frame_rate_measured_hz` | Measured rate vs. nominal rate |
| `zmq_cmd_to_photon_latency_ms` | Reference: time from Python command send to photon |

---

## Layer 1 — Rust `FrameStats` (always-on) — IMPLEMENTED

> Implemented in `src/timing.rs`. Called from `RenderState::render()` in
> `src/render/state.rs` immediately after `present()`.

`FrameStats` tracks frame durations in a 120-entry ring buffer (2 s at 60 Hz)
with no heap allocation after construction.  It is called immediately after
`SurfaceTexture::present()` in the render loop.

```
present()  ←── frame N rendered
  │
  └── frame_stats.on_present()
        ├── latch Instant
        ├── compute duration since last present
        ├── detect drop if duration > 1.5 × expected_frame_ns
        └── update ring buffer
```

### `FrameStats` struct

```rust
const FRAME_HISTORY: usize = 120;  // 2 s at 60 Hz

struct FrameStats {
    frame_index:       u64,
    last_present:      Option<Instant>,
    durations_ns:      [u64; FRAME_HISTORY],   // ring buffer
    ring_head:         usize,
    valid_count:       usize,
    drop_count:        u64,
    expected_frame_ns: u64,                    // from nominal Hz
}
```

### `FrameSummary` (computed O(N), N ≤ 120)

```rust
struct FrameSummary {
    fps:          f64,
    mean_ms:      f64,
    std_ms:       f64,
    min_ms:       f64,
    max_ms:       f64,
    drop_count:   u64,
    frame_index:  u64,
}
```

---

## Layer 2 — egui HUD overlay (feature = `"overlay"`) — IMPLEMENTED

> Implemented in `src/render/overlay.rs`. `OverlayRenderer` is created in
> `RenderState::new()` and called from `RenderState::render()`.

An optional in-window HUD renders timing metrics as a second render pass with
`LoadOp::Load` (composited over the stimulus).

Enable with:
```bash
cargo run --features overlay
```

Toggle visibility at runtime with **F1**.

### HUD metrics and color coding

| Metric | Green | Yellow | Red |
|---|---|---|---|
| FPS | ≥ 58 | 55–58 | < 55 |
| Jitter (std ms) | < 0.3 | 0.3–1.0 | > 1.0 |
| Drop count | 0 | 1–2 | ≥ 3 |

Also shown: mean frame ms, min/max frame ms, frame index.

### Dependencies (`egui-wgpu 0.33` targets wgpu 27)

```toml
[features]
overlay = ["dep:egui", "dep:egui-wgpu", "dep:egui-winit"]

egui       = { version = "0.33", optional = true }
egui-wgpu  = { version = "0.33", optional = true }
egui-winit = { version = "0.33", optional = true }
```

---

## Phase 3 — ZMQ PUB frame events (planned)

Requires the server to implement the `get_time` REQ/REP command and emit
`FrameFlip` events on a ZMQ PUB socket (`tcp://*:5556`).

### `FrameFlip` event

```rust
FrameFlip {
    frame_index:       u64,
    flip_timestamp_ns: u64,   // wall-clock ns since session_start
    duration_ns:       u64,   // frame that just completed
    mean_ns:           u64,
    std_ns:            u64,
    drop_count:        u64,
}
```

### Clock calibration

Before the test, 10 round-trip `get_time` commands measure the Python↔server
clock offset:

```python
t_before = time.time_ns()
reply = conn.send({"cmd": "get_time"})   # → {"server_ns": <ns since UNIX epoch>}
t_after = time.time_ns()
clock_offset_ns = reply["server_ns"] - (t_before + t_after) // 2
```

After applying the average offset, Python can compare `flip_timestamp_ns`
from `FrameFlip` events directly against DAQ timestamps.

---

## Phase 4 — Hardware trigger output (planned, highest precision)

The server writes a monotonically-incrementing `frame_counter: AtomicU64` into
a named shared memory region immediately after `present()`.  A separate trigger
process busy-polls this region and asserts a DAQ digital output on change.

```
server:          present() → shm.frame_counter.fetch_add(1)
trigger process: poll shm → DAQ digital out HIGH → 1 ms later LOW
DAQ:             records digital trigger + photodiode analog input
```

Latency budget:

| Step | Latency |
|---|---|
| shm write after `present()` | < 100 ns |
| Cache coherency (same machine) | < 1 µs |
| Trigger process detection | < 1 µs |
| USB DAQ digital output (ljm/nidaqmx) | 100–500 µs |
| **Total trigger latency** | **< 1 ms** |

This is well below the display response time (1–20 ms), so hardware-trigger
latency introduces negligible error into the render-to-photon measurement.

With Phase 4, `latency_source == "hardware_trigger"` in the JSON output and
no clock synchronisation is required — both the trigger and the photodiode are
recorded in the same DAQ hardware timebase.

See `docs/INPUT_LATENCY.md` for the shared memory layout.
