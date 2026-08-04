# Platform Notes

## Rasbperry Pi 5 (Raspbian OS)

- Networking issues (dropping connections)
  ```
  sudo ethtool --set-eee eth0 eee off
  ```
- Status LED turns red after UEFI: Are you using a 27 W power supply?
- USB DisplayLink adapters (evdi) are still unsuitable for **stimulus** output — DisplayLink relays frames over USB via software encode/decode with no direct vsync from the GPU (confirmed against the kernel driver source: `evdi_enable_vblank()` is a stub, there is no periodic vblank timer at all), so frame timing is not deterministic. Don't use DisplayLink for anything timing-sensitive.
  However, a compositor is **not** required to drive one — that part of the old note was wrong. evdi just needs *something* acting as DRM master pushing framebuffers to its virtual KMS device (`/dev/dri/cardN`, `driver=evdi`, separate from the real GPU node); a full desktop compositor is merely the common way to get that. vstimd has a dedicated `--evdi` backend (`server/src/render/evdi/`, see `docs/developer/evdi-direct-presentation-plan.md`) that renders through the normal Vulkan pipeline into a headless swapchain (`VK_EXT_headless_surface` — confirmed available on the V3D/Mesa driver), reads the frame back to the CPU, and presents it directly via `page_flip` + evdi's completion-event backpressure (real flow control paced to `DisplayLinkManager`'s actual USB consumption, not a synthetic clock — plain `set_crtc` every frame has no backpressure and visibly tears). Validated on this rig: real vstimd scene content renders and presents correctly, naturally paced to ~60fps by that backpressure. Use `--evdi` for an auxiliary/status screen only, never for stimulus presentation.
- DisplayLink USB displays keep toggling on and off? Dont use a USB-C power switch
