# Plan: driving a DisplayLink (evdi) screen directly, without a compositor

Status: **Phase 1 implemented and validated on hardware** (this rig, 2026-08-04).
`--evdi` renders real vstimd scene content and presents it on the DisplayLink
screen with no compositor running. Not timing-sensitive — this backend is
explicitly **not** for stimulus presentation. See "Non-goals".

The actual implementation ended up simpler than the original plan below in
one respect (it reuses `render_frame()`/`VkContext` via a headless swapchain
instead of duplicating the Vulkan pipeline — see "What actually got built")
and required one extra piece of investigation the original plan didn't
anticipate (evdi has no vblank clock at all; `page_flip` backpressure was
needed instead — see "evdi has no real vsync" below). The rest of this
document is left as originally written, as the design record; the deltas are
called out inline.

## Motivation

[Platform notes](platform-notes.md) currently say DisplayLink/evdi is unusable
because "only a running compositor (Xorg/Wayland) can feed it, via dma-buf
hand-off from the real GPU." That's true for the *common* case, but it
overstates the requirement. What evdi actually needs is not "a compositor" —
it's *any* process willing to act as DRM master on its virtual KMS device and
push framebuffers to it. A full desktop compositor is just the easy way to get
that. Nothing stops vstimd from doing it itself, in-process, with no external
compositor package installed.

Precision/timing is explicitly out of scope here (DisplayLink relays frames
over USB via software encode with no GPU vsync — that limitation doesn't go
away just because vstimd drives the KMS device directly). The use case is a
secondary/auxiliary screen (status output, monitoring, a second low-rate
view) that doesn't need frame-accurate timing, not stimulus presentation.

## Non-goals

- No sub-millisecond or vsync-accurate presentation on the DisplayLink output.
  This stays true regardless of who drives the KMS device.
- Not a replacement for `VK_KHR_display` on the primary stimulus screen.
- Not (yet) zero-copy / dma-buf export from the GPU. See "Phase 2" below —
  deferred until Phase 1 proves too slow to be useful.

## Confirmed hardware facts (Raspberry Pi 5, this rig, 2026-08-03)

```
card0  driver=v3d       (compute-only GPU, no connectors — this is what
                          VK_KHR_display currently renders through)
card1  driver=vc4-drm   (bcm2712-vc6, the real HDMI display controller;
                          HDMI-A-1/2 currently disconnected — no monitor
                          plugged into HDMI on this rig right now)
card2  driver=evdi      DVI-I-1, status=connected  ← live DisplayLink dongle
card3  driver=evdi      DVI-I-2, status=unknown (no dongle)
card4  driver=evdi      DVI-I-3, status=unknown (no dongle)
card5  driver=evdi      DVI-I-4, status=unknown (no dongle)
```

`/opt/displaylink/DisplayLinkManager` runs as a root daemon (systemd/init,
not shown as a `.service` unit — started some other way) completely
independent of any compositor. No Xorg/Wayland is running on this box at all,
and DisplayLinkManager has still enumerated the dongle and marked `card2`'s
connector `connected`. That confirms the framing above: DisplayLinkManager
itself doesn't require a compositor to exist — it only needs *something* to
eventually write real frame content to `card2`.

The `drm` crate (already a dependency, v0.15, used today in
`drm_display_guard.rs`/`drm_virtual_terminal.rs`) exposes everything needed
for the KMS side with no new dependency:
`Device::get_driver()` (name-based node detection), `create_dumb_buffer`,
`map_dumb_buffer`, `add_framebuffer`, `set_crtc`, `page_flip`.

## Architecture

### Why `render_frame.rs` can't be reused as-is

`render_frame()` is hard-wired to a `VkSwapchainKHR`: it calls
`ctx.swapchain_loader.acquire_next_image(...)` to get an image and
`queue_present` to hand it back. There is no swapchain path available for
evdi — `VK_KHR_display` is GPU-scoped (can't see `card2`, confirmed by
`card0`/v3d having zero connectors) and there's no windowing-system surface
without a compositor. So evdi needs an **offscreen** render target: a plain
`VkImage` the render pass draws into, with no `VkSwapchainKHR` involved at
all.

`VkContext` itself is unavoidably swapchain-shaped too: `swapchain`,
`swapchain_loader`, `surface`, `surface_loader`, `present_mode`,
`present_wait` are all non-`Option` fields, and its `Drop` impl
unconditionally destroys them — there's no way to hand `render_frame.rs` a
"headless" `VkContext` without either faking swapchain/surface handles (unsound)
or splitting `VkContext` into a core-vs-swapchain-specific structure that
every caller (`SceneRenderer`, `TextRenderer`, `UiRenderer`, `render_frame`,
the egui renderer) would need updating for. That's a real refactor of code
shared by the precision-critical `DrmBackend`/`WinitBackend` paths, and it's
disproportionate risk for a backend that explicitly doesn't need any of the
precision machinery those types exist to support.

Decision: **don't refactor `VkContext`/`render_frame.rs` at all.** The evdi
backend builds its own minimal, separate Vulkan setup — instance, device,
render pass, command pool, framebuffer-backed-by-a-plain-image — reusing the
free functions that already take raw handles rather than `&VkContext`
(`create_vk_instance`, `create_render_pass`, `create_framebuffers`,
`VkPipeline::new`, `VkGratingPipeline::new` all take `&ash::Device` /
`vk::RenderPass` directly, not `&VkContext`). This duplicates a small amount
of setup code but touches nothing the DRM/Winit backends depend on.

### evdi has no real vsync — but `page_flip` gives real backpressure

Checked against the actual kernel driver source
(`DisplayLink/evdi` `module/evdi_modeset.c` / `evdi_painter.c`) and confirmed
on hardware: evdi has **no periodic vblank timer** —
`evdi_enable_vblank()` is a stub returning `1`, and
`DRM_IOCTL_WAIT_VBLANK` against `card2` returns in under 2µs every time with
the frame counter frozen, i.e. it's a no-op here, not a clock.

What evdi *does* have: every atomic commit (`set_crtc`, `page_flip`, and
`dirtyfb` all funnel through the same `drm_atomic_helper_*` path) marks the
frame dirty via `evdi_plane_atomic_update`. If a `page_flip` was submitted
with `DRM_MODE_PAGE_FLIP_EVENT`, `evdi_painter_set_vblank()` only completes
that event once `DisplayLinkManager` (the consumer) has actually drained the
*previous* frame's dirty rects through its `GRABPIX` ioctl — otherwise
completion is deferred until it does. Confirmed empirically: `page_flip` +
waiting for the completion event on `card2` took 1.5–19ms per call,
irregular — not a periodic clock, but real flow control paced to actual
USB/encode consumption.

Plain `set_crtc` with no event requested (what the KMS smoke test used) has
none of this — nothing ever gates it, so a naive per-frame loop overwrites a
buffer DisplayLinkManager may still be mid-read on. That's the almost
certain cause of the tearing/stuttering observed in the animated smoke
test. **`EvdiOutput::present()` uses `page_flip` with an event and blocks on
its completion before returning**, giving proper backpressure — still not
frame-accurate vsync (there's no clock to be accurate to), but presentation
paced to what the consumer can actually keep up with instead of an
unthrottled write race.

### Phase 1 (MVP): CPU-readback presenter, no dma-buf

Simplest correct thing that could work, and the right starting point given
timing is explicitly not a goal:

1. Render each frame via the existing Vulkan pipeline into an offscreen
   device-local `VkImage` sized to the mode evdi reports (instead of a
   swapchain image).
2. `vkCmdCopyImageToBuffer` into a host-visible/coherent staging buffer,
   wait on the frame's fence.
3. `memcpy` the staging buffer into the mmap'd DRM dumb buffer that is the
   current back framebuffer on `card2`.
4. `drmModePageFlip` (first frame: `drmModeSetCrtc`) to flip to it.
5. Double-buffer: two dumb buffers / two `AddFB` framebuffers, alternate.

No new Vulkan extensions, no dma-buf, no dependency on what the V3D Mesa
driver (`v3dv`) does or doesn't support for external memory — that's the
biggest unknown in the zero-copy version, so Phase 1 sidesteps it entirely.
Cost: a GPU→CPU readback and a CPU copy per frame. For an auxiliary
non-timing-critical screen this is very likely fine; Phase 1 includes
measuring it to confirm.

### Phase 2 (optional, only if Phase 1 is too slow to be useful)

Zero-copy path: export the rendered `VkImage` as a dma-buf
(`VK_EXT_external_memory_dma_buf` + `VK_EXT_image_drm_format_modifier`,
if `v3dv` supports them — unconfirmed, needs a feasibility spike first),
import the dma-buf fd into evdi via `drmPrimeFDToHandle` +
`drmModeAddFB2`, and commit that directly — no CPU copy at all. Not
attempted until Phase 1 is working and its overhead is actually measured
and found to matter.

## What actually got built (supersedes "Module layout" below)

`vulkaninfo` turned up `VK_EXT_headless_surface` as available on this rig's
V3D/Mesa driver — a real `VkSwapchainKHR` that isn't tied to any window
system or physical display (Mesa's headless WSI backs it with plain images).
That changes the "why `render_frame.rs` can't be reused" conclusion above:
instead of duplicating the Vulkan pipeline, `evdi_init.rs` creates a headless
surface and calls the *existing* `build_context()` (shared with DRM/Winit)
unchanged, producing a normal `VkContext` with a real swapchain. This means
`render_frame()`, `SceneRenderer`, tessellation, and the egui overlay are
reused **completely as-is** — the only genuinely new Vulkan code is reading
the rendered image back afterward.

The one shared-code touch needed: `FrameTick` gained an `image_index: u32`
field (`server/src/timing.rs`) so the evdi backend knows which swapchain
image `render_frame()` just rendered into and left in `PRESENT_SRC_KHR`
layout — a single additive struct field, not a behavior change for the
existing DRM/Winit callers.

Final module layout, `server/src/render/evdi/`:

```
evdi/
  mod.rs
  evdi_detect.rs        find /dev/dri/cardN where get_driver().name() ==
                        "evdi", pick the first with a connected connector
  evdi_kms.rs           mode/CRTC/encoder selection, two double-buffered
                        XRGB8888 dumb buffers, EvdiOutput::present() —
                        page_flip + block on the completion event (see
                        "evdi has no real vsync" above)
  evdi_init.rs          VK_EXT_headless_surface + build_context() — a real
                        VkContext, no swapchain/render_frame.rs changes needed
  evdi_render_loop.rs   EvdiBackend: builds RenderState (ui: None, no VT/
                        input/vblank-clock machinery — none of it applies),
                        calls render_frame() with screen_clock =
                        Some(Instant::now()) every iteration, then Readback
                        (vkCmdCopyImageToBuffer into a persistent host-
                        visible staging buffer sized to evdi's row pitch,
                        blocking on a fence) hands the bytes to
                        EvdiOutput::present()
```

Wired into `main.rs` as planned: `RenderTarget::Evdi`, `--evdi` flag,
`EvdiBackend::new(data, log_buffer).run(on_ready)`.

Validated on hardware in two stages:

1. `examples/evdi_scene_probe.rs` (constructs a `SceneState` with real rects
   via `handle_request`, same pattern the crate's own integration tests use,
   and drives a real `EvdiBackend` directly — no ZMQ client was available on
   this box at that point) — ~900 frames over 15s, naturally paced to ~60fps
   by `page_flip` backpressure, correctly positioned colored rectangles
   confirmed by direct visual check.
2. The full `client/python` e2e suite (`make test-e2e`, after installing
   `uv`, which wasn't present on this box either) run against a real
   `vstimd --evdi` process over the network/ZMQ path: **126 passed, 6
   xfailed** (pre-existing known-unimplemented features — polygons,
   draw-order reordering — not evdi-related), ~10,500 frames over ~2 minutes,
   no panics or fatal errors. Exercised rects, circles, ellipses, gratings,
   text, animations, and VTL-triggered stimuli, all visibly rendered on the
   physical DisplayLink screen. The e2e fixture's `has_display` skip check
   only knows about `DISPLAY`/`WAYLAND_DISPLAY` and doesn't know about
   `--evdi` as a third way to get real visible rendering — worked around by
   setting a dummy `DISPLAY` value for this run (the fixture then found the
   already-running server immediately and never used the spawn/no-display
   fallback path); a real fix would teach that fixture about `--evdi`.

### Known limitation: dropped-frame log spam

`render_frame()`'s dropped-frame accounting (`FrameStats::on_present` in
`timing.rs`) compares inter-frame intervals against an `expected_frame_ns`
derived from `refresh_hz` — meaningful for the DRM/Winit backends, which are
genuinely paced to a display refresh rate. The evdi backend has no such
clock (see above) and is paced by `page_flip` backpressure instead, so its
frame-to-frame interval is irregular by design. This makes every frame look
like a "dropped frame" to that accounting, producing constant `WARN`-level
log spam. Cosmetic only — not fixed here to avoid adding evdi-specific
branches to shared timing code for a log-noise issue; worth a follow-up if
`--evdi` sees real use (e.g. a `dropped_frames` opt-out on `FrameStats`, or
routing evdi through `log::debug!` instead of `warn!` at the call site).

## Module layout (original plan, superseded above)

New `server/src/render/evdi/` (mirrors the existing `drm/` module):

```
evdi/
  mod.rs
  evdi_detect.rs         done — find /dev/dri/cardN where get_driver().name()
                         == "evdi", pick the first with a connected connector
  evdi_kms.rs            mode/CRTC/encoder selection + dumb buffer alloc/map/
                         AddFB (the logic proven in examples/evdi_probe.rs)
  evdi_vk.rs             standalone headless Vulkan setup: instance, device,
                         render pass, command pool, one device-local color
                         image + one host-visible readback buffer per
                         in-flight frame — no swapchain, no VkContext
  evdi_render_loop.rs    per-frame: tessellate → draw into the offscreen
                         image → vkCmdCopyImageToBuffer → memcpy into the
                         current dumb buffer → page_flip; mirrors
                         drm_render_loop.rs's overall shape but with its own
                         Vulkan objects, not VkContext
  mod.rs                 EvdiBackend, same run(on_ready) shape as
                         DrmBackend/WinitBackend
```

This was the plan before `VK_EXT_headless_surface` was discovered to be
available — see "What actually got built" above for what was actually
implemented instead.

## Wiring into `main.rs`

Add an explicit `--evdi` flag for the MVP rather than folding it into
auto-detection (`detect_render_target`) right away — auto-detecting "is this
an evdi-only box with no real display" is a reasonable follow-up once the
backend is proven, not before. `RenderTarget` gets a new `Evdi` variant,
`EvdiBackend::new(data, log_buffer).run(on_ready)` alongside the existing
`DrmBackend`/`WinitBackend`/`NullBackend` arms.

## Testing plan (on this hardware)

1. `evdi_detect` standalone: confirm it finds `card2` and reads its connector
   state — no Vulkan involved yet, fastest thing to verify on real hardware.
2. Bare KMS smoke test: allocate one dumb buffer, fill it with a solid color
   in a checkerboard test pattern, `set_crtc` onto `card2`'s connector,
   confirm the physical DisplayLink monitor shows it. Proves the KMS
   plumbing end-to-end before any Vulkan work.
3. Wire in the offscreen Vulkan render + readback loop, confirm actual
   vstimd scene content appears and updates.
4. Measure achieved frame rate / CPU cost of the readback+memcpy path.
5. Leave running for a few minutes to check for the "USB-C power switch
   causes toggling" issue already noted in platform-notes and for any
   DRM-master contention with DisplayLinkManager's own internal handling.

## Risks

- **Readback cost unknown until measured.** Could dominate frame time on a
  Pi 5. Acceptable if so — this backend was never going to be fast — but
  worth knowing the actual number.
- **evdi's accepted format/modifier is unconfirmed.** Expect `XRGB8888`
  (matches `DRM_FORMAT_XRGB8888`) but must verify against what `card2`
  actually reports/accepts.
- **DRM-master contention.** Unclear whether DisplayLinkManager itself ever
  wants master on `card2`, or purely watches for framebuffer changes as a
  passive client. Phase 1's smoke test (item 2 above) will surface this
  immediately if it's a problem.
- **This is a genuinely separate render backend**, not a small patch —
  expect several hundred lines of new code. Scoped into phases above so each
  step is independently testable on hardware rather than one large
  untested change.

## Follow-up

- [x] `platform-notes.md` updated with the actual result: works via a
  dedicated backend, ~60fps achieved (backpressure-paced, not a fixed
  clock), still not timing-suitable for stimuli — auxiliary output only.
- [ ] Dropped-frame log spam (see above) — cosmetic, worth fixing if
  `--evdi` sees real use.
- [ ] Phase 2 (dma-buf zero-copy) — not attempted; Phase 1's readback cost
  wasn't measured in isolation (no perf regression observed at ~60fps on a
  Pi 5, and that's already backpressure-capped by DisplayLinkManager, so
  there was no throughput ceiling to hit). Revisit only if a future use case
  needs a higher frame rate than backpressure currently allows.
- [ ] `client/python/tests/e2e/conftest.py`'s `has_display` check (only
  looks at `DISPLAY`/`WAYLAND_DISPLAY`) doesn't know `--evdi` is a third way
  to get real visible rendering — worth teaching it that, so e2e runs
  against an evdi rig don't need the `DISPLAY=:0` workaround used above.
