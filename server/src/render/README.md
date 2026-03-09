# render module

All GPU and wgpu interaction is isolated here. No other module in the codebase should depend on wgpu directly.

## Responsibilities

- **Surface/device/queue lifecycle** (`state.rs`) — creates the wgpu instance, adapter, device, and surface; configures vsync and present mode.
- **Render pipeline** (`pipeline.rs`) — WGSL shader source and `RenderPipeline` construction.
- **GPU buffer management** (`gpu_buffers.rs`) — uploads tessellated vertex/index data to GPU buffers, keyed by stimulus handle.
- **Tessellation** (`tess.rs`) — converts `scene::Stimulus` objects into triangulated vertex/index arrays in NDC space (CPU-side, no wgpu dependency itself, but tightly coupled to the vertex format).
- **egui overlay** (`overlay.rs`, feature-gated behind `overlay`) — diagnostic frame-timing HUD rendered on top of the scene. Currently a thin rendering-only wrapper, so it lives here. When it grows into a real GUI (input handling, panels, application state), move it to its own top-level `ui/` module — potentially a separate workspace crate so it can be shared between the server and a connected client.

## Data flow

```
SceneState (owned by RenderState)
    │
    ▼
tess::tessellate_stimulus()  →  (Vec<Vertex>, Vec<u32>)
    │
    ▼
GpuBuffers::upload()         →  wgpu vertex/index buffers
    │
    ▼
RenderState::render()        →  draw calls + present
```

`RenderState` owns the `SceneState` and is the only consumer of scene data for rendering. The scene module defines stimulus types and logic without any GPU awareness.
