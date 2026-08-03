# Platform Notes

## Rasbperry Pi 5 (Raspbian OS)

- Networking issues (dropping connections)
  ```
  sudo ethtool --set-eee eth0 eee off
  ```
- Status LED turns red after UEFI: Are you using a 27 W power supply?
- USB DisplayLink adapters (evdi) are unsuitable for stimulus output — not just hard to wire up. The virtual output sits on its own GPU-less DRM device; only a running compositor (Xorg/Wayland) can feed it, via dma-buf hand-off from the real GPU. vstimd's headless `VK_KHR_display` path (`server/src/render/drm/`) enumerates connectors on the GPU only, so it can't reach a DisplayLink screen at all. And even through a compositor, DisplayLink relays frames over USB via software encode/decode with no direct vsync from the GPU, so frame timing is not deterministic. Don't use DisplayLink for anything timing-sensitive.
- DisplayLink USB displays keep toggling on and off? Dont use a USB-C power switch
