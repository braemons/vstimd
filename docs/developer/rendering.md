# Rendering & DRM internals

This page covers how the render backends are built — the crates, the module layout,
and the vblank-source selection logic. For *operating* a bare-metal rig (kernel
setup, permissions, running without a compositor) see
[Bare-metal Linux](../operations/bare-metal.md); for the frame-timing guarantee from
a user's perspective see [Frame timing](../concepts/frame-timing.md).

## Backends

vstimd renders through raw Vulkan (`ash`) on one of three backends, selected at
startup (see [Architecture → Rendering backends](architecture.md#rendering-backends)):

- **DRM/console** — direct KMS/DRM via `VK_KHR_display`, no compositor.
- **Desktop** — `VK_KHR_surface` via `winit` + `ash-window`.
- **Null** — no display; ZMQ server only.

Shaders are written in GLSL and compiled to SPIR-V at build time.

## Dependencies (DRM backend, Linux-only)

| Crate | Purpose |
|---|---|
| `ash` | Raw Vulkan bindings — all GPU operations |
| `drm` | DRM device open + display/connector/mode enumeration |
| `input` | libinput keyboard/mouse events |
| `libc` | VT switching, ioctl calls (e.g. `DRM_IOCTL_WAIT_VBLANK`) |

On minimal embedded systems without udev, `Libinput::new_from_path` opens
`/dev/input/eventN` devices directly, eliminating the udev transitive dependency.

## Module structure

| Module | Contents |
|---|---|
| `server/src/render/mod.rs` | Backend entry points, auto-detection, `spawn_demo_stimuli` |
| `server/src/render/drm/` | DRM backend: display discovery, render loop, vblank, libinput keyboard |
| `server/src/render/winit_vk/` | Desktop backend: winit event loop and render loop |
| `server/src/render/vk/` | Shared Vulkan code (both backends) |
| `server/src/render/vk/vk_context.rs` | `VkContext`: instance, device, queue |
| `server/src/render/vk/egui/` | Vulkan egui renderer for the overlay UI |
| `server/src/render/overlay_ui/` | Overlay panels (Stimuli / Log / VTL / Animations / System / Config / Benchmarks) and dialogs |
| `server/src/render/render_frame.rs` | Per-frame render logic |

## The vblank-source chain

The render loop blocks on the true display vblank. Sources are tried in priority
order; on error a source is permanently disabled and the next is used. The active
source is shown in the overlay's **System** panel (F5).

1. **DRM vblank** (`DRM_IOCTL_WAIT_VBLANK`) — bare-metal DRM only. A blocking kernel
   ioctl firing at the start of the vertical blanking interval. Most precise; zero
   offset from true vblank.
2. **`VK_EXT_display_control`** (`vkRegisterDisplayEventEXT`) — bare-metal fallback.
   A one-shot Vulkan fence signalling on `FIRST_PIXEL_OUT` (first active scanline
   after blanking; ≈330 µs after true vblank at 120 Hz — a fixed offset). Used on
   Jetson Orin (Tegra nvdisplay), where `DRM_IOCTL_WAIT_VBLANK` is unavailable while
   `VK_KHR_display` holds DRM master.
3. **`VK_KHR_present_wait`** — wakes after the previous frame is confirmed presented.
   Common on desktop (winit) drivers.
4. **GPU fence completion** — last resort. Wakes when the GPU finishes rendering, not
   when the frame is on screen; adds up to a frame of unpredictable latency. The
   overlay labels this *"Clock: GPU-completion (inaccurate)"*.

!!! note "Why FIFO acquire is not used as a vblank source"
    `vkAcquireNextImageKHR` with FIFO blocks at refresh rate, but the block point is
    at image *acquisition* — before tessellation and upload — so scene inputs are
    stale by one full frame relative to sources 1–3, which fire *after* the previous
    frame is visible and *before* tessellation. For input-latency-sensitive work the
    DRM / `VK_EXT_display_control` approach is correct.

### `VK_EXT_display_control` prerequisites

The extension needs three things at init:

1. `VK_EXT_display_surface_counter` enabled as an **instance** extension.
2. `VK_EXT_display_control` enabled as a **device** extension.
3. `VkSwapchainCounterCreateInfoEXT` with `VK_SURFACE_COUNTER_VBLANK_BIT_EXT` chained
   into swapchain creation — without it `vkRegisterDisplayEventEXT` returns
   `ERROR_UNKNOWN`.

These are handled in the DRM init and `vk_context.rs`; the extension is activated
only when the driver advertises every required capability.

## Present mode

Production always uses `VK_PRESENT_MODE_FIFO_KHR` (vsync, no tearing). `MAILBOX_KHR`
is available via `--present-mode mailbox` for benchmarking only — it can shave up to
one frame of vsync wait but makes it impossible to know which frame a deferred flip
first became visible on.

## Implementation status

**✅ Implemented**

- DRM display discovery and mode selection (rig-config display mode is honoured)
- Vulkan init via `VK_KHR_display` (Jetson) and via winit on desktop
- Swapchain (FIFO), render pass, solid-colour and grating pipelines, text rendering
- GPU buffer management
- libinput keyboard/mouse handling; VT switching
- Vsync-locked render loop with the vblank-source chain above
- egui overlay with a Vulkan renderer (F1–F7 panels)
- Auto-detection of console vs desktop mode

**📋 Planned**

- Raspberry Pi 5 support (`VK_EXT_acquire_drm_display` via Mesa v3dv)
- Textured pipeline for bitmap stimuli
- Polygon stimulus (schema exists; renderer not finished)
- 3-D stimuli
