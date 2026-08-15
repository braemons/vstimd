# 3-D Stimulus Roadmap

> Companion document to `PLAN.md` and `STIMULUS_DATA_MODEL.md`.  
> Covers the planned evolution from the current 2-D stimulus set toward full 3-D scene
> rendering, including corridors/mazes, 3-D primitives, mesh models, and — as a long-horizon
> research target — Gaussian splatting.

> **Status.** Phases A and B are broken down into implementation issues: epic
> [#76](https://github.com/braemons/vstimd/issues/76), sub-issues #68–#75. Where this document
> and those issues disagree, the issues win — they were written against the current code.
>
> **This document was originally drafted against `wgpu`.** The renderer is hand-written Vulkan
> via `ash`, with WGSL compiled to SPIR-V at build time by `naga` (`server/build.rs`). The text
> below has been corrected, but treat any remaining `wgpu::` type name as a bug in this file.

---

## Table of Contents

1. [Guiding Principles](#1-guiding-principles)
2. [Rendering Architecture Evolution](#2-rendering-architecture-evolution)
3. [Coordinate Systems](#3-coordinate-systems)
4. [Phase A — 3-D Infrastructure](#4-phase-a--3-d-infrastructure)
5. [Phase B — 3-D Primitives](#5-phase-b--3-d-primitives)
6. [Phase C — Corridor and Maze Stimuli](#6-phase-c--corridor-and-maze-stimuli)
7. [Phase D — Mesh Model Stimuli](#7-phase-d--mesh-model-stimuli)
8. [Phase E — Gaussian Splatting (Long Horizon)](#8-phase-e--gaussian-splatting-long-horizon)
9. [Impact on the Stimulus Enum and Data Model](#9-impact-on-the-stimulus-enum-and-data-model)
10. [Impact on the Scene State and Protocol](#10-impact-on-the-scene-state-and-protocol)
11. [Impact on Animations](#11-impact-on-animations)
12. [Crate Dependencies for 3-D](#12-crate-dependencies-for-3-d)
13. [Open Questions](#13-open-questions)

---

## 1. Guiding Principles

### 1.1 2-D must never regress

The 2-D stimulus pipeline (flat shapes, bitmaps, pixel shaders) is the production-critical
path. All 3-D work is additive. The 2-D render pass must remain:

- Pixel-perfect in placement (centre-origin, Y-up, pixel coordinates).
- Frame-synchronised with vsync (no dropped frames due to heavy 3-D scenes).
- Unaffected by the presence or absence of 3-D stimuli in the scene.

### 1.2 2-D and 3-D coexist in the same frame

Many experiments will mix 2-D overlays (fixation cross, photodiode flash, reward cue) with a
3-D background scene. The render loop must composite both layers every frame. The draw order is:

```
[3-D pass]  → clear depth, draw 3-D world
[2-D pass]  → draw flat stimuli on top (no depth test, screen-aligned)
[overlay]   → egui debug overlay (feature-gated)
```

### 1.3 The stimulus enum stays closed; the 3-D variants are just more arms

The `Stimulus` enum design from `STIMULUS_DATA_MODEL.md` extends naturally. 3-D stimulus
variants follow exactly the same composition rules as 2-D ones: explicit component structs,
`Deferred<T>` for all deferrable parameters, no inheritance.

### 1.4 The camera is a first-class scene object, not a global

In a 3-D scene the camera pose (position, orientation, field of view) directly determines what
the animal sees. It must be controllable frame-by-frame from the same animation system that
moves 2-D stimuli. It therefore lives in `SceneState` as a named object, not as a global
rendering parameter.

### 1.5 Complexity is introduced in phases; each phase is independently shippable

Each phase below delivers a working system. Later phases build on earlier ones but do not
require rewriting them.

### 1.6 Geometry is shared; instances are cheap

**The mesh (shared geometry) is separate from the instance (transform + material).** Primitives
are tessellated at *unit* size — a unit cube, a unit sphere, a unit quad — and their nominal size
(`radius`, `size`) is folded into the model matrix as scale. The GPU mesh cache is keyed by a
**geometry descriptor**, not by stimulus handle, so one cube mesh and one sphere mesh serve every
instance in the scene.

This is the one decision in Phase A/B that is expensive to reverse. Every other cache in the
codebase (`SolidMeshCache`, `TextMeshCache`) is keyed by stimulus handle; copying that pattern
here would make a corridor of N tiles allocate N identical vertex buffers. It is also what lets
Phase C avoid a scene graph entirely (§6).

### 1.7 Shared stimulus properties stay shared, and sizes are full extents

Two conventions were settled for 2-D and bind the 3-D variants as well.

**`StimulusCommon` is the state every stimulus has**, flattened into each variant's struct and
its config JSON (`server/src/scene/stimulus/stimulus_common.rs`). Today it is
`{ flags, transform: Deferred<Transform2D>, opacity }` — every stimulus is 2-D, so all three
genuinely are common. A 3-D variant gets `flags` and `opacity` for free and must not redeclare
either.

`transform` is the field that does not survive the move to 3-D: a position, an orientation and a
scale in world space cannot be a `Vec2` and one angle. **Adding the first 3-D variant means
taking it out of `StimulusCommon`** and onto each variant as `Deferred<Transform2D>` or
`Deferred<Transform3D>`, behind the split accessors in §9.3, leaving `{ flags, opacity }` shared.
That is a small edit today and a wide one after three 3-D variants exist, so do it as the first
step of Phase B rather than after. The wire is already shaped for it:
`QueryStimulusResponse.placement` is a `oneof` with a single `transform_2d` arm.

**Opacity is a multiplier, not a colour.** `effective_a = color.a * opacity`, clamped to
`[0, 1]`, and it is set by the shared `SetAlpha` command for every stimulus type. For 3-D that
means `material.albedo.a * opacity` — see §B.5 for where it enters the push constants and §B.6
for what the renderer owes in return.

**Sizes on the wire and in the config are full extents.** `CreateRect{width, height}` and the
saved `"size": [w, h]` are the same numbers; the renderer halves them at tessellation. 3-D
follows: a cube carries `size: Vec3` (full extents in cm) and folds `size * 0.5` into the model
scale. A sphere keeps `radius`, matching `Circle`, where API and config already agree. Do not
reintroduce a `half_size` field — that split is exactly what the v3 config format removed.

### 1.8 There is no scene graph, and the corridor does not need one

`SceneState` holds a flat `IndexMap<u32, Stimulus>`. An endless corridor is built from **periodic
geometry and a wrapped camera** (§C.0), not from tiles recycled past the camera — only the latter
requires parent transforms. A one-level `Group` becomes worth its cost when the **maze** arrives
(§C.2), and at that point it is purely additive.

### 1.9 Target platforms for 3-D

3-D targets **Jetson Orin Nano** and **x86 desktops with a discrete GPU**. **Raspberry Pi 4/5 is
not a 3-D target** — if its tile-based V3D cannot hold frame timing, we do not chase it.

The Pi remains a first-class **2-D** target. §1.1 therefore still applies to it, and is discharged
structurally rather than by measurement: the 3-D render pass, its framebuffers and the depth image
are all created **lazily**, on the first frame a 3-D stimulus exists. A server that never sees one
records the same command buffer it does today.

Performance criteria for 3-D bind on the Jetson — the weakest supported 3-D target.

---

## 2. Rendering Architecture Evolution

### 2.1 Current state (2-D only)

```
SceneState
└── stimuli: IndexMap<u32, Stimulus>   (Rect, Ellipse, Circle, Grating, Text)

Render thread  (ash / raw Vulkan)
├── render_pass       [colour]  loadOp=CLEAR   → PRESENT_SRC_KHR
│   ├── solid_pipeline      (shapes tessellated with lyon, CPU→NDC)
│   ├── grating_pipeline    (unit quad + analytic fragment shader, push constants)
│   ├── text_pipeline       (glyph atlas, R8_UNORM, descriptor set)
│   └── (+ wireframe variants of solid and grating)
│
└── egui_render_pass  [colour]  loadOp=LOAD    → PRESENT_SRC_KHR   (overlay, conditional)
```

All 2-D geometry is transformed to clip space **on the CPU** (`render/tess.rs::px_to_ndc`, with
`z = 0.0`); `solid.wgsl`'s vertex shader is a pass-through with an empty pipeline layout. There is
no depth buffer, no matrix library, and backface culling is disabled (`CullModeFlags::NONE`)
everywhere.

### 2.2 Target state (2-D + 3-D)

```
SceneState
├── stimuli:       IndexMap<u32, Stimulus>   (2-D and 3-D variants)
├── camera:        Deferred<Camera3D>
├── ambient_light / sun_direction / sun_colour
└── (no world/scene-graph object — see §1.8)

Render thread  (ash / raw Vulkan)
├── depth_image: vk::Image (D32_SFLOAT)   ← allocated lazily, only if a 3-D stimulus exists
│
├── [mesh3d_render_pass]  [colour, depth]  loadOp=CLEAR,CLEAR → COLOR_ATTACHMENT_OPTIMAL
│   ├── mesh3d_pipeline     (depth test+write, cull BACK, front CCW)
│   └── wireframe_mesh3d
│
├── [render_pass]         [colour]  loadOp = CLEAR if no 3-D, else LOAD
│   ├── solid_pipeline      ← unmodified
│   ├── grating_pipeline    ← unmodified
│   └── text_pipeline       ← unmodified
│
└── [egui_render_pass]    [colour]  loadOp=LOAD                      (overlay, conditional)
```

The 3-D pass writes and tests depth; the 2-D pass does not attach depth at all, so 2-D stimuli
always appear in front regardless of their notional Z position.

**Why the 2-D pipelines are untouched.** A Vulkan pipeline is created against a render pass but
need only be *compatible* with the one it runs in, and compatibility depends solely on attachment
formats and sample counts — load ops, store ops and image layouts are irrelevant. The codebase
already relies on this: `create_egui_render_pass` differs from `create_render_pass` only in
`CLEAR` → `LOAD`, and `render_frame.rs` begins it against the *main* pass's framebuffer.

So the 2-D pass simply gains a second, compatible flavour (`CLEAR` when the scene is pure 2-D,
`LOAD` when the 3-D pass has already drawn), selected per frame on `scene.has_3d()`. The same
pipeline objects serve both.

Adding a depth attachment to the *existing* render pass instead would change its attachment list,
break compatibility, force all four 2-D pipelines to be recreated, and — because Vulkan then
requires a non-null `pDepthStencilState` — mean editing `vk_render_pipeline.rs`,
`grating_pipeline.rs` and `vk_text_pipeline.rs`. Exactly the churn §1.1 forbids.

**Known bug to fix before the 3-D pass lands:** `create_render_pass` sets
`final_layout(PRESENT_SRC_KHR)` while `create_egui_render_pass` sets
`initial_layout(COLOR_ATTACHMENT_OPTIMAL)`. These disagree. Nothing in the tree enables
`VK_LAYER_KHRONOS_validation`, which is why it has gone unnoticed. Adding a third pass in front
multiplies the variant matrix (clear/load × which pass is last). Longer term,
`VK_KHR_dynamic_rendering` (core in Vulkan 1.3; `vk_instance.rs` requests 1.1) removes
`VkRenderPass`/`VkFramebuffer` and the matrix with them.

---

## 3. Coordinate Systems

Three coordinate systems are in play. Their relationship must be documented and kept consistent
throughout the codebase.

### 3.1 Stimulus space (2-D, existing)

- Origin at screen centre.
- X right, Y up.
- Units: pixels.
- Used by all existing 2-D stimuli and animations.
- **Unchanged by 3-D work.**

### 3.2 World space (3-D)

- Right-handed, Y-up convention (matches glTF, Blender export defaults, and most neuroscience
  VR literature).
- Origin is arbitrary; by convention the animal's nominal starting position.
- Units: centimetres (chosen because typical corridor widths are 20–100 cm — keeps numbers
  human-readable and avoids floating-point precision issues at metre scale).
- The camera lives in world space.

### 3.3 Clip space / NDC (Vulkan)

- Vulkan NDC has **Z in [0, 1]** (not OpenGL's [−1, 1]) and **Y pointing down**.
- `glam::Mat4::perspective_rh` produces the correct `z ∈ [0, 1]` depth range.
- **The Y flip must be applied explicitly** — negate `m[1][1]` of the projection, or post-multiply
  by `Mat4::from_scale(vec3(1.0, -1.0, 1.0))`.

⚠ The existing 2-D path defines its clip space as **Y-up** (`render/tess.rs`: *"The renderer's
clip space is Y-up (top of screen = +1)"*). The 3-D projection must match that convention rather
than fight it, or the 3-D scene renders vertically mirrored relative to 2-D overlays. This is the
single most likely source of a lost day in Phase A. **Unit-test it** — assert that a world point
above the camera lands in the upper half of the screen. Do not eyeball it.

### 3.4 Conversion utilities

```rust
/// Convert a 2-D stimulus-space position to a world-space point on the
/// near plane (for overlay sprites attached to 3-D scenes).
pub fn stimulus_to_world(pos: [f32; 2], screen_size: [f32; 2], camera: &Camera3D) -> glam::Vec3 { ... }

/// Project a world-space point to stimulus space (for HUD labels, gaze overlays).
pub fn world_to_stimulus(world: glam::Vec3, screen_size: [f32; 2], camera: &Camera3D) -> [f32; 2] { ... }
```

---

## 4. Phase A — 3-D Infrastructure

> **Prerequisite:** Phases 1–7 of `PLAN.md` (core 2-D system) complete.

This phase introduces the machinery that all subsequent 3-D stimuli depend on.
No visible 3-D stimuli are added yet — only the scaffolding.

### A.1 `Camera3D` as a scene object

```rust
#[derive(Clone, Copy)]
pub struct Camera3D {
    pub position:   glam::Vec3,   // world space, cm
    pub yaw:        f32,          // degrees, rotation around Y axis
    pub pitch:      f32,          // degrees, tilt up/down
    pub roll:       f32,          // degrees, bank (usually 0)
    pub fov_y:      f32,          // vertical field of view, degrees
    pub near:       f32,          // near clip plane, cm (e.g. 1.0)
    pub far:        f32,          // far clip plane, cm (e.g. 100_000.0)
}

impl Camera3D {
    pub fn view_matrix(&self) -> glam::Mat4 { ... }
    pub fn proj_matrix(&self, aspect: f32) -> glam::Mat4 { ... }
    pub fn view_proj(&self, aspect: f32) -> glam::Mat4 {
        self.proj_matrix(aspect) * self.view_matrix()
    }
}
```

`Camera3D` lives in `SceneState` as `Deferred<Camera3D>` so it participates in the deferred
flip exactly like any other parameter. It can be assigned an animation handle, allowing gaze-
locked or trajectory-driven camera movement through the existing animation system.

The camera is initialised to a sensible default: positioned at the origin, looking down the
negative Z axis, 60° FoV, near=1 cm, far=50 000 cm.

### A.2 Scene uniform buffer

One UBO at descriptor set 0, binding 0, updated once per frame before the 3-D pass. It carries
both the camera and the lighting (§10.1) so the layout does not churn between Phase A and Phase B.

```wgsl
struct Scene {
    view_proj:  mat4x4<f32>,
    camera_pos: vec3<f32>,  _pad0: f32,
    ambient:    vec3<f32>,  _pad1: f32,
    sun_dir:    vec3<f32>,  _pad2: f32,
    sun_color:  vec3<f32>,  _pad3: f32,
}
@group(0) @binding(0) var<uniform> scene: Scene;
```

`vec3` fields align to 16 bytes under `std140` — hence the explicit padding. Mirror it with a
`#[repr(C)] bytemuck::Pod` struct and assert `size_of::<SceneUniform>() == 128`.

Copy the descriptor pool / layout / set / `update_descriptor_sets` pattern from
`GlyphAtlas::create_descriptor_set` (`render/vk/vk_text_atlas.rs`). Note the glyph atlas gets away
with **one** set because it is written outside the frame loop; the scene UBO is written every
frame and needs one set per in-flight frame slot, or an explicit fence wait.

### A.3 Depth image

A `vk::Image`, same extent as the swapchain. **Allocated lazily** — on the first frame a 3-D
stimulus exists, not at startup (§1.9). Recreated on swapchain resize. Attached only by the 3-D
render pass.

Select the format by querying support rather than hardcoding; `D32_SFLOAT` is not universally
guaranteed as an optimal-tiling depth attachment:

```rust
// D32_SFLOAT → D32_SFLOAT_S8_UINT → D24_UNORM_S8_UINT
for candidate in [..] {
    let props = instance.get_physical_device_format_properties(pdev, candidate);
    if props.optimal_tiling_features.contains(DEPTH_STENCIL_ATTACHMENT) { .. }
}
```

If no candidate is supported, return `ERROR_CODE_NOT_SUPPORTED` from the 3-D create commands and
keep serving 2-D. Never panic the render thread.

### A.4 New render pass structure

Three passes, recorded into the one command buffer that `render_frame` already builds:

```
3-D pass    [colour, depth]   CLEAR, CLEAR   — skipped entirely when !scene.has_3d()
2-D pass    [colour]          CLEAR if !has_3d, else LOAD
egui pass   [colour]          LOAD           — conditional, unchanged
```

Within the 3-D pass, bind the scene descriptor set **once** for the whole batch (it does not vary
per stimulus), then per stimulus: bind the shared mesh's vertex/index buffers, push the per-object
constants (§B.5), `cmd_draw_indexed`. Extend `render_frame.rs`'s existing `Bound` enum with a
`Mesh3d` arm so the lazy pipeline-rebinding optimisation still applies.

Because the 3-D pass is created lazily and skipped when absent, **a pure-2-D scene records a
byte-identical command buffer to the pre-3-D code.** That, not a benchmark, is what discharges
§1.1 (§1.9).

⚠ Allocating the depth image and render pass on the first 3-D frame means a device allocation on
the render thread, which must never block. Pre-create them on the ZMQ thread inside
`cmd_create_sphere_3d` — an already-allocating, off-thread command — rather than eating a dropped
frame.

### A.5 Render-thread-private 3-D resources

```rust
pub struct Mesh3dCache {
    /// Keyed by *geometry*, not by stimulus handle (§1.6). One cube mesh and one
    /// sphere mesh per tessellation quality, shared by every instance.
    meshes:   HashMap<MeshKey, VkMesh>,
    refcount: HashMap<MeshKey, u32>,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum MeshKey {
    Cube,
    Sphere { rings: u32, sectors: u32 },
    Plane,                                 // Phase C
}

/// Keyed by source path, so N stimuli sharing a wall texture upload it once.
pub struct Texture3dCache {
    textures: HashMap<String, VkTexture>,
    refcount: HashMap<String, u32>,
}
```

`Mesh3dCache` and `Texture3dCache` become fields on `SceneCache`
(`render/vk/cache/scene_cache.rs`), whose doc comment already reserves the slot: *"new categories
(3-D meshes, video frames, …) add a field here."*

`VkMesh` already exists. Unlike `SolidMeshCache`, which reallocates a `HOST_VISIBLE | HOST_COHERENT`
buffer on every upload (2-D shapes are small and re-tessellate on resize), 3-D meshes are static
for the lifetime of the `MeshKey` and belong in **device-local memory via a staging buffer**.
`GlyphAtlas` already has that machinery — `begin_one_time_cb` / `end_and_submit_one_time_cb` /
`alloc_bind_device_local`. Lift it into `render/vk/buffers.rs` rather than copy-pasting.

Pipelines (`mesh3d_pipeline`, `wireframe_mesh3d`) live on `SceneRenderer` beside the existing
2-D ones. Textured and untextured share **one** pipeline: an untextured stimulus binds a 1×1 white
texture, so `albedo` tinting is uniform across both cases and there is a single descriptor set
layout.

### A.6 Protobuf additions for Phase A

```protobuf
// proto/vstimd/v1/vec3.proto — already written in unmerged commit 6311d41
message Vec3 { float x = 1; float y = 2; float z = 3; }

message SetCameraRequest {
    Vec3  position  = 1;
    float yaw_deg   = 2;
    float pitch_deg = 3;
    float roll_deg  = 4;
    float fov_y_deg = 5;
    float near_cm   = 6;
    float far_cm    = 7;
}
message QueryCameraRequest {}
```

There is no `SystemCmd` message. Add these to the single `oneof body` in `service.proto`, with a
`SystemTarget` target: `set_camera = 77`, `query_camera = 85`. See §10.2 for the full field-number
allocation, and register every new `.proto` in **both** lists in `server/build.rs` (the
`rerun-if-changed` block *and* the `compile_protos` call — forgetting the second is a silent
no-op).

### A.7 New crate dependencies for Phase A

```toml
glam  = { version = "0.29", features = ["bytemuck"] }  # 3-D math
```

`glam` is the de-facto standard for Rust graphics work. Its `Mat4`, `Vec3`, `Quat` types are
`bytemuck`-compatible and map directly to WGSL types. It is not currently in `Cargo.lock`.

---

## 5. Phase B — 3-D Primitives

> **Prerequisite:** Phase A complete.

Procedurally generated 3-D shapes, defined in world space, rendered in the 3-D pass.

### B.1 New stimulus variants

```rust
// Added to the Stimulus enum:
Stimulus::Sphere3D(Sphere3DStimulus),     // first slice
Stimulus::Cube3D(Cube3DStimulus),         // first slice
Stimulus::Plane3D(Plane3DStimulus),       // bounded quad; arrives with Phase C
// Stimulus::Cylinder3D — unscheduled
```

### B.2 Component structs

```rust
/// 3-D placement — equivalent of Transform2D but in world space.
#[derive(Clone, Copy)]
pub struct Transform3D {
    pub position:    glam::Vec3,    // world space, cm
    pub orientation: glam::Quat,   // rotation
    pub scale:       glam::Vec3,   // non-uniform scale (default [1,1,1])
}

impl Transform3D {
    pub fn model_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(
            self.scale, self.orientation, self.position)
    }
}

/// Surface appearance for 3-D stimuli (simple Phong / unlit model — never PBR).
#[derive(Clone, Copy)]
pub struct Material3D {
    pub albedo:   Color,      // crate::Color, not [f32;4] — see below
    pub emissive: [f32; 3],   // self-illumination (for stimuli that must hit a specific luminance)
    pub shading:  Shading3D,
}

#[derive(Clone, Copy, Default)]
pub enum Shading3D {
    #[default]
    Unlit,    // albedo only, no lighting — most psychophysics stimuli want this
    Phong,    // Lambert diffuse + Blinn-Phong specular, one directional light
}
```

Both `Transform3D` and `Material3D` are wrapped in `Deferred<T>` on each stimulus struct.
Neither carries visibility or opacity: those come from the shared `StimulusCommon` (§1.7), so
`Material3D` has no alpha of its own beyond `albedo.a`, which the shared opacity multiplies.

Two deliberate choices:

- **`albedo: Color`, not `[f32; 4]`.** `crate::Color` (`server/src/color.rs`) is the established
  colour type across the codebase and is already `Pod`. It keeps 3-D stimuli consistent with the
  gamma work in [#55](https://github.com/braemons/vstimd/issues/55).
- **No `roughness` field.** An earlier draft of this document carried one, annotated "unused in
  unlit mode". We do not do PBR, and shipping a dead field into the wire format and the config
  JSON is a liability. The Blinn-Phong specular exponent is hardcoded to 32; if a per-material
  exponent is ever wanted, add `shininess: f32` then — there are 32 spare bytes in the
  push-constant budget (§B.5).

### B.3 Concrete structs

The first slice ships **`Cube3D` and `Sphere3D` only**. `Plane3D` arrives with the corridor
(§6); `Cylinder3D` is unscheduled.

```rust
pub struct Cube3DStimulus {
    #[serde(flatten)]
    pub common:       StimulusCommon,         // flags + opacity (transform moved out — §1.7)
    pub transform:    Deferred<Transform3D>,
    pub material:     Deferred<Material3D>,
    pub size:         Deferred<glam::Vec3>,   // FULL extents in cm → folded into model scale
    pub texture_path: Option<String>,         // server-side path; None = untextured
}

pub struct Sphere3DStimulus {
    #[serde(flatten)]
    pub common:       StimulusCommon,
    pub transform:    Deferred<Transform3D>,
    pub material:     Deferred<Material3D>,
    pub radius:       Deferred<f32>,          // cm → folded into model scale
    pub rings:        u32,                    // tessellation quality — selects the MeshKey,
    pub sectors:      u32,                    //   so not deferrable
    pub texture_path: Option<String>,
}
```

Per §1.6, `radius` and `size` do **not** change the mesh. They are pre-multiplied into the
model matrix:

```rust
Mat4::from_scale_rotation_translation(
    transform.scale * radius,         // sphere
    // cube: transform.scale * size * 0.5 — `size` is the full extent (§1.7), and the unit
    // cube is 2 units across, so the halving happens here rather than in the API or config.
    transform.orientation,
    transform.position,
)
```

Only `rings` / `sectors` select geometry. Document the `radius × transform.scale` composition —
it is the one place a user could be surprised.

### B.4 Vertex format for 3-D

**No new vertex type is needed.** `server/src/geom.rs::Vertex` already is:

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],   // present today, written as [0,0,1] by 2-D tessellation
    pub uv:       [f32; 2],   // present today, written as [0,0]
    pub color:    Color,
}
```

Its doc comment already anticipates this: *"A single type covers all stimulus geometry (2-D flat
shapes, billboards, 3-D meshes)."* 3-D tessellation is the first code to populate `normal` and
`uv` meaningfully. Textured stimuli set `color = Color::WHITE` so the tint multiply is a no-op.

### B.5 Per-object data → push constants

Vulkan guarantees 128 bytes of push constants. Budget:

| field | bytes |
|---|---|
| `model: mat4x4<f32>` | 64 |
| `albedo: vec4<f32>` | 16 |
| `emissive: vec3<f32>` + `shading: u32` | 16 |
| **total** | **96** |

The shared opacity (§1.7) needs **no field of its own**: multiply it into `albedo.a` when
building the push constants, the way `build_grating_push_constants` already folds state into its
constants. Budget unchanged.

Fits, with 32 spare. Follow the `GratingPushConstants` precedent
(`scene/stimulus/grating/grating_pipeline.rs`): `#[repr(C)]`, `bytemuck::Pod`, and a doc comment
giving the byte offset of every field.

An earlier draft proposed a per-object UBO at `@group(1)`. Push constants are simpler, cheaper,
and match how `grating` and `text` already work. Revisit only if per-instance counts reach the
thousands, at which point per-instance data belongs in a storage buffer indexed by
`@builtin(instance_index)` anyway.

**Normals.** Because `radius` / `size` become scale (§B.3), **non-uniform scale is the normal
case, not an edge case** — any cube with unequal half-extents, and any ellipsoid, has it. Deriving
the normal matrix as `mat3(model)` shades both visibly wrong. An inverse-transpose `mat3` does not
fit in push constants (48 B under std430; 96 + 48 = 144 > 128), so compute it in the vertex shader:

```wgsl
// WGSL has no inverse() builtin — naga rejects it. Hand-write the 3×3 (adjugate / determinant).
fn inverse3(m: mat3x3<f32>) -> mat3x3<f32> { /* ~15 lines */ }
```

Per-vertex cost is negligible (~1.1k vertices for a 32×32 sphere). Always `normalize()` the
interpolated normal in the fragment shader — a scaled normal matrix does not preserve length.

### B.6 Tessellation

All 3-D primitives are tessellated on the CPU into **unit** geometry (§1.6), lazily on first
reference to a `MeshKey`, and refcounted. No library is needed. New module
`server/src/render/tess3d.rs`, sibling of `tess.rs` — but emitting **object-space** positions,
where `tess.rs` bakes NDC on the CPU.

- **Cube**: 6 faces × 2 triangles × **24 vertices** (4 per face — not 8; each face needs its own
  normal and its own UV square), 36 indices.
- **Sphere**: UV sphere, `rings × sectors` quads, default 32 × 32. `normal = position` (unit
  sphere). Two traps: **duplicate the seam column** so `u = 0` and `u = 1` are distinct vertices,
  or the texture wraps backwards across one column; and emit **triangles, not zero-area quads**, at
  the poles.
- **Plane** (Phase C): a single unit quad, sized by scale, with a `uv_tile` material factor so wall
  textures repeat rather than stretch.

**Winding must be counter-clockwise viewed from outside** — the 3-D pipeline sets
`cull_mode = BACK, front_face = COUNTER_CLOCKWISE`. Every existing pipeline uses
`CullModeFlags::NONE`, so the codebase has never had to get winding right and no existing test
would catch a mistake. Assert it: for every triangle, `(b−a) × (c−a)` dotted with the outward
direction must be positive.

Dirty-tracking changes meaning relative to 2-D. A stimulus becoming dirty because its `radius`,
`position` or `albedo` changed **does not** re-tessellate — those live in the model matrix and the
push constants. Only a change of `MeshKey` does. Screen resize does not invalidate 3-D meshes at
all (unlike 2-D, whose NDC vertices are screen-dependent), so skip them in the
`last_uploaded_size` sweep.

Textures are decoded on the **ZMQ thread** at create time (via the `image` crate, `png` + `jpeg`
features only) and uploaded as `R8G8B8A8_SRGB` so the sampler linearises for free. The render
thread must never block or heap-allocate. A missing or undecodable path returns
`ERROR_CODE_FILE_NOT_FOUND` / `ERROR_CODE_FILE_FORMAT`.

### B.6 Transparency

`SetAlpha` is a shared command that every stimulus type accepts (§1.7), so a 3-D stimulus can
be told to be half transparent before the renderer knows what to do about it. The scene
state is trivially correct; the ordering is not. Draws are issued in `IndexMap` order and
correctness comes from the depth buffer, which is order-independent only for opaque geometry.

Phase B therefore owes a two-bucket draw:

1. **Opaque** (`albedo.a * opacity == 1.0`): current behaviour — any order, depth test and depth
   write on.
2. **Transparent** (`< 1.0`): drawn after the opaque bucket, sorted back-to-front by distance
   from `Camera3D.position`, depth test on and **depth write off**.

That is the standard pragmatic answer and it is right for the stimuli this server draws
(a handful of convex primitives). It is still wrong for intersecting or concave transparent
geometry, and for those the honest fix is order-independent transparency — out of scope. Say so
in the user docs rather than pretending otherwise.

Until the two-bucket draw lands, a 3-D stimulus with `opacity < 1` renders order-dependently:
accept the command, document the limitation, and do not special-case `SetAlpha` to reject 3-D —
a shared property that silently is not shared is worse than one with a known caveat.

---

## 6. Phase C — Corridor and Maze Stimuli

> **Prerequisite:** Phase B complete.

Corridors and mazes are the primary use case for 3-D visual stimuli in rodent / primate
navigation experiments. They are **procedural environments**, not mesh files, making them easy
to parameterise and update from the control script.

### C.0 Endless corridors: move the camera, not the world

There are two ways to make a corridor endless, and they are not equivalent.

**Recycle tiles behind the camera to the front.** As the camera advances, the tile that fell out of
view is teleported ahead. This is the approach that forces a scene graph on you: a tile's walls,
floor and objects must move together, so you need parent transforms and per-frame reparenting. The
scene also mutates every few seconds, so every wrap must flip atomically or the animal sees a torn
frame.

**Wrap the camera inside one period.** The corridor is periodic with period `L`. `LinearNav3D`
(§11.2) keeps `camera.position.z` in `[0, L)` while accumulating true distance separately. Geometry
is built once, covering enough tiles to fill the frustum, and **never moves again**. Nothing
mutates per frame except the camera pose.

**Take the second.** Fewer moving parts, exactly periodic by construction, keeps `f32` world
coordinates small — and, per §1.8, it removes the need for a `Node` abstraction entirely. With the
geometry-keyed mesh cache (§1.6), drawing the same wall panel at eleven `z` offsets costs eleven
push-constant writes and one vertex-buffer bind.

The constraint this imposes: the corridor's *appearance* must be periodic with period `L`. Cues
and objects repeat every `L`. **An experiment needing a finite track with unique landmarks along
its length is a different stimulus** — a long, non-wrapping corridor. Document this; do not let a
user silently get a repeating track when they wanted a unique one.

With `far = 5000 cm` and `period_cm = 500`, you draw ~11 tiles × 3 panels plus objects — roughly
40 draw calls of one shared unit quad, occupying **one** vertex buffer. Start with N draw calls
and per-instance push constants; true GPU instancing (`instance_count > 1`, per-instance data in a
buffer) earns its keep in the thousands, not at 40. Frustum culling is not needed at this scale.
Do not build it.

### C.1 Corridor stimulus

A corridor is an axis-aligned tube with configurable cross-section, length, wall texture, and
visual cues (stripes, landmarks) placed at specified positions along its length.

It is best implemented as a **generator, not a mesh**: it owns `CorridorParams` and, on `rebuild`,
emits a list of `(MeshKey, Transform3D, Material3D)` instances for the tiled floor and walls, all
of which are `MeshKey::Plane`. One handle, one `retain` entry, one dirty flag — many draws. This
is the seam a `Node` would otherwise occupy, and it is narrower: the corridor knows its own
periodicity and regenerates its instance list without a general parent-transform system.

Objects inside the corridor stay ordinary `Sphere3D` / `Cube3D` stimuli at world positions in
`[0, L)`. To make one appear in every visible tile rather than once per lap, the corridor generator
also emits copies at `z + k·L` — the same instance-list mechanism. Expose as
`repeat_with_corridor: bool`.

```rust
#[derive(Clone, Copy)]
pub struct CorridorParams {
    pub width:      f32,          // cm
    pub height:     f32,          // cm
    pub length:     f32,          // cm
    pub floor_tex:  Option<u32>,  // texture_id, None = solid colour
    pub wall_tex:   Option<u32>,
    pub ceil_tex:   Option<u32>,
    pub floor_col:  [f32; 4],     // used if no texture
    pub wall_col:   [f32; 4],
    pub ceil_col:   [f32; 4],
    pub cue_period: f32,          // cm between visual cues (0 = no cues)
    pub cue_col:    [f32; 4],
}

pub struct CorridorStimulus {
    #[serde(flatten)]
    pub common:    StimulusCommon,          // flags + opacity, transform below (§1.7)
    pub transform: Deferred<Transform3D>,   // position/orientation of corridor entrance
    pub params:    Deferred<CorridorParams>,
    pub rebuild:   bool,
    // The animal's position along the corridor is usually driven by ExternalPosition2D
    // mapped to camera position — the corridor itself does not move.
}
```

### C.2 Maze stimulus

A maze is a collection of corridor segments connected at junctions. It is described by a graph:

```rust
pub struct MazeNode {
    pub position: glam::Vec3,    // junction centre in world space, cm
    pub radius:   f32,           // junction room radius
}

pub struct MazeCorridor {
    pub from:   usize,           // index into nodes
    pub to:     usize,
    pub width:  f32,
    pub height: f32,
    pub wall_col: [f32; 4],
    pub floor_col: [f32; 4],
}

pub struct MazeStimulus {
    #[serde(flatten)]
    pub common:     StimulusCommon,
    pub transform:  Deferred<Transform3D>,
    pub nodes:      Vec<MazeNode>,       // not deferrable individually — rebuild on change
    pub corridors:  Vec<MazeCorridor>,
    pub rebuild:    bool,
}
```

The maze is tessellated once at creation and re-tessellated only when the topology changes.
Wall colour and texture changes can be applied without a full rebuild by updating per-segment
push constants.

**This is where a `Group` earns its cost.** Junctions and corridor segments at arbitrary angles
genuinely want a parent transform, and by this point the corridor's instance list (§C.1) has
already proved that `(mesh, transform, material)` is the right unit. Add it then, as a purely
additive change: a `parent: Option<u32>` on `StimulusSceneEntry` plus a
`groups: IndexMap<u32, Group>` map, composing `model = parent.model * local.model`.

**Keep it one level deep.** Arbitrary nesting buys cycle detection, transform-propagation dirty
flags and a serialization headache, for a use case nobody has yet.

### C.3 Camera navigation in a corridor

The camera is the animal's viewpoint. It is driven by `ExternalPosition2D` (mapped to world X/Z
position) or by dedicated 3-D animation types (see §11). A typical setup:

1. Create a `CorridorStimulus` (static, no animation).
2. Create `ExternalPosition2D` reading from `/vstimd_treadmill` shared memory (linear position
   along corridor), or `LinearNav3D` if the treadmill reports velocity rather than position.
3. Assign the animation to the **camera**, not to a stimulus — via
   `AnimationTarget::Camera` (§11.1), **not** a sentinel handle.

### C.4 Tessellation approach for corridors

A corridor is just a box with the front and back faces open. Each wall/floor/ceiling panel is
a `PlaneStimulus3D` internally, but they are grouped under a single handle. The tessellator
generates the full corridor mesh in one call, taking `CorridorParams` as input.

For corridors with texture, UV coordinates are set so that a repeating wall texture tiles
naturally: U maps to the length axis, V maps to the height axis, with scale factors derived
from `length / texture_width_cm` and `height / texture_height_cm` (both configurable).

---

## 7. Phase D — Mesh Model Stimuli

> **Prerequisite:** Phase B complete.

Load and render static or animated 3-D mesh models from files. The primary use cases are:

- Object recognition tasks (a 3-D object rotates; the animal must identify it).
- Reward indicators (a 3-D icon appears at the goal location in a maze).
- Avatars / conspecifics for social neuroscience paradigms.

### D.1 File format

**glTF 2.0** (`.gltf` / `.glb`) is the recommended format:

- Open standard, widely supported by Blender, Maya, and all major 3-D tools.
- Embeds materials, textures, and optionally skeletal animation in a single file.
- The `gltf` crate provides a pure-Rust parser with no native dependencies.
- glTF uses Y-up, right-handed coordinates — consistent with our world-space convention.

**OBJ** (`.obj` + `.mtl`) is supported as a fallback for legacy assets via the `tobj` crate.

### D.2 Stimulus struct

```rust
pub struct MeshStimulus {
    #[serde(flatten)]
    pub common:     StimulusCommon,
    pub transform:  Deferred<Transform3D>,
    pub material_override: Deferred<Option<Material3D>>,  // None = use material from file
    pub anim_frame: Deferred<f32>,   // for glTF skeletal animation: time in seconds
    pub anim_speed: Deferred<f32>,   // playback rate multiplier (0 = paused)
    // GPU resource handles — set at load time, owned by Mesh3dCache
    pub mesh_id:    u32,
    pub skin_id:    Option<u32>,     // for skinned meshes
}
```

### D.3 Loading pipeline

```
File on disk (glTF/OBJ)
  ↓  (blocking, at creation time, on the ZMQ thread or a background thread)
CPU geometry: Vec<geom::Vertex>, Vec<u32>
  ↓  (render thread, next frame after load completes)
staged upload → device-local VkMesh → Mesh3dCache[MeshKey::File(path)]
```

Loading is done on a background thread to avoid blocking the render loop. The stimulus is
created immediately with `flags.enabled = false`; once loading completes a flag is set and the
render thread uploads the buffers. The stimulus is then automatically enabled (or the client
polls for completion and enables it manually).

### D.4 glTF skeletal animation

glTF skeletal animation is evaluated on the CPU using the `gltf` crate's animation data and a
small skin evaluation loop. The resulting joint matrices are uploaded to a uniform buffer (or
storage buffer for large skeletons) and applied in the vertex shader.

This is optional and can be deferred to a sub-phase. Static meshes are the immediate priority.

### D.5 LOD considerations

For now, single-LOD meshes are sufficient. If experiments require very large or complex models,
a simple manual LOD system (the client loads multiple mesh variants and switches by handle) is
preferable to an automatic system, since the client controls what appears on screen.

---

## 8. Phase E — Gaussian Splatting (Long Horizon)

> **Prerequisite:** Phase D complete. Requires significant GPU compute capability.  
> **Timeline:** Research/experimental, no fixed schedule.

Gaussian splatting (3-D Gaussian Splatting, 3DGS) is a novel-view synthesis technique that
represents a scene as a collection of anisotropic 3-D Gaussians, each with position, covariance,
opacity, and colour (via spherical harmonics). It can reconstruct photorealistic scenes from
photographs and render them in real time.

In the context of visual neuroscience stimuli, the primary motivation is:

- **Photorealistic virtual environments** reconstructed from real-world locations (the animal's
  home cage, a real maze, a natural scene).
- **Natural image statistics** without the manual labour of 3-D modelling.
- **Dynamic novel-view synthesis**: the camera moves freely through a pre-recorded scene.

### E.1 What Gaussian splatting is and why it is hard

A trained 3DGS scene contains millions of Gaussians. Rendering requires:

1. **View-dependent sorting**: Gaussians must be drawn back-to-front (or an alpha-compositing
   order) from the current camera viewpoint. This requires a GPU radix sort every frame.
2. **Tile-based rasterisation**: the reference renderer uses a CUDA kernel for tile-based
   splatting. A Vulkan port requires a compute shader implementation.
3. **Spherical harmonics evaluation**: per-Gaussian colour varies with view direction (up to
   degree 3 = 16 coefficients × 3 channels per Gaussian).

This is a substantial rendering research project, not a straightforward stimulus type. It is
listed here to ensure the architecture does not accidentally close off the possibility.

### E.2 Architectural prerequisites (to not foreclose this option)

The following decisions in earlier phases keep the door open:

1. **Vulkan compute is available.** The sorting and splatting passes need a
   `VK_PIPELINE_BIND_POINT_COMPUTE` pipeline and storage buffers. `ash` exposes both; naga emits
   compute SPIR-V from WGSL, so the existing `build.rs` shader pipeline extends unchanged. Nothing
   in Phase A forecloses it. (3-D targets — Jetson, discrete x86 GPUs, §1.9 — all support compute
   comfortably; the Pi is out of scope regardless.)

2. **The 3-D render pass can be replaced or augmented.** The Phase A architecture separates
   the 3-D pass from the 2-D pass. The splatting renderer replaces the geometry pipeline in
   the 3-D pass, not the whole frame. 2-D overlays still work.

3. **Large GPU buffer support.** A scene with 1–5 million Gaussians needs ~200–1000 MB of GPU
   memory for positions, covariances, and SH coefficients. This requires a real allocator —
   the current `alloc_upload_bytes` / `alloc_bind_device_local` helpers allocate one
   `VkDeviceMemory` per buffer, and Vulkan implementations cap `maxMemoryAllocationCount` (often
   4096). Phase E would want `gpu-allocator` or `vk-mem`. Not a blocker; just not free.

4. **The `Camera3D` struct is already sufficient.** Gaussian splatting needs exactly
   position + orientation + FoV + near/far — identical to Phase A's camera.

### E.3 Proposed stimulus struct (placeholder)

```rust
pub struct GaussianSplatStimulus {
    #[serde(flatten)]
    pub common:     StimulusCommon,
    // The camera is the viewpoint; the splat itself has no transform
    // (the scene is world-aligned at training time).
    // A global offset can be applied via the transform if needed.
    pub transform:  Deferred<Transform3D>,
    // No `opacity_scale` field: fade-in/out is the shared `common.opacity` (§1.7), which
    // multiplies each Gaussian's own alpha during compositing. 3DGS is the one variant where
    // the sorted alpha blend it needs (§E.2) is already required for its own sake.
    pub sh_degree:  u32,                // 0–3; lower = faster, less view-dependent colour
    // GPU resource handle — points to a GaussianSplatScene in the 3-D caches
    pub scene_id:   u32,
}
```

### E.4 Loading and training

Training a 3DGS scene is done offline (using the original 3DGS CUDA code, or `nerfstudio`,
or similar). The trained scene is saved as a `.ply` file (the standard interchange format).
The stimulus server loads the `.ply` at runtime and uploads Gaussian attributes to GPU buffers.

A pure-Rust `.ply` parser exists (`ply-rs` crate). The Gaussian attribute layout in `.ply` is
well-defined by the original paper's reference implementation.

### E.5 Rendering approach

The most practical implementation of 3DGS rasterisation is a **compute-then-draw** pipeline:

```
[compute pass]
  1. Cull Gaussians behind camera or outside frustum (compute shader)
  2. Project Gaussians to 2-D screen-space ellipses (compute shader)
  3. Compute sort keys (depth) (compute shader)
  4. GPU radix sort by depth (compute shader, e.g. using the `wgpu-radix-sort` pattern)

[render pass]
  5. For each Gaussian (indirect draw): splat a screen-aligned quad, alpha-blend
     the Gaussian footprint using the precomputed 2-D covariance
```

Step 4 is the hardest. A portable GPU radix sort in compute shaders exists in research
implementations and is a tractable engineering project. Reference: the `CUDA-Free 3DGS` work
and the `web-splat` open-source wgpu 3DGS viewer (MIT licence) are both good starting points —
their WGSL compute shaders port to this codebase's `naga`-based build, though their wgpu host code
does not.

### E.6 Integration with the experiment control protocol

From the client's perspective, a Gaussian splat scene is just another stimulus handle:

```protobuf
message CmdLoadGaussianSplat {
    string path       = 1;   // path to .ply file
    uint32 sh_degree  = 2;   // 0–3
}
```

The camera is controlled through the existing `SetCameraRequest` / `ExternalPosition2D` mechanism.
No special protocol is needed.

---

## 9. Impact on the Stimulus Enum and Data Model

### 9.1 Extended `Stimulus` enum

```rust
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Stimulus {
    // ── 2-D (as of today) ──────────────────────────────────────────────
    Rect(RectStimulus),
    Ellipse(EllipseStimulus),
    Circle(CircleStimulus),
    Grating(GratingStimulus),
    Text(TextStimulus),

    // ── 3-D primitives (Phase B) ───────────────────────────────────────
    Sphere3D(Sphere3DStimulus),
    Cube3D(Cube3DStimulus),
    Plane3D(Plane3DStimulus),        // arrives with Phase C
    // Cylinder3D — unscheduled

    // ── 3-D environments (Phase C) ─────────────────────────────────────
    Corridor(CorridorStimulus),
    Maze(MazeStimulus),

    // ── 3-D mesh models (Phase D) ──────────────────────────────────────
    Mesh(MeshStimulus),

    // ── Gaussian splatting (Phase E) ───────────────────────────────────
    GaussianSplat(GaussianSplatStimulus),
}
```

An earlier draft of this section listed `Petal`, `Wedge`, `Disc`, `Bitmap`, `BitmapSeq`,
`WgslShader`, `Particle` and `Pixel` as existing 2-D variants. **None of them exist.** The five
above are the whole enum. (`Polygon` has proto messages but no server implementation — see
[#20](https://github.com/braemons/vstimd/issues/20).)

`#[serde(tag = "type")]` means a new variant serializes into the config JSON as
`{"type": "Sphere3D", ...}` automatically. No `io_config.rs` changes; adding a variant is additive
and does not warrant a `CONFIG_VERSION` bump.

### 9.2 Exhaustive matches enforce completeness

There is no `stim_field!` macro (an earlier draft claimed one). `stimulus.rs` has a small
`shape_arm!` macro covering only the three shape variants, and a set of hand-written **exhaustive
`match`es**: `flags`/`flags_mut`, `shape_appearance`/`_mut`, `is_shape`, `reset_phase_accum`,
`make_copy`, `flip`, `type_name`. Exhaustive matches also live in `render_frame.rs` (update and
draw) and `scene/command.rs` (`command_summary`, `handle_system_command`,
`handle_stimulus_command`, `query_stimulus_response`).

The compiler finds every one of them. That is the safety net: the server cannot compile with a
half-wired stimulus type. The **clients** have no such link to the proto schema and are where
drift actually happens.

### 9.3 The `transform()` accessor splits into 2-D and 3-D variants

`StimulusCommon` (§1.7) holds `flags`, `transform` and `opacity` today. `flags` and `opacity`
stay there — `Stimulus::flags()` / `opacity()` / `set_opacity()` are single-line delegations that
need no change when 3-D variants arrive. `transform` moves out onto each variant, and today's
`transform()` / `transform_mut()` are replaced with:

```rust
impl Stimulus {
    /// Returns the 2-D transform for 2-D stimuli. None for 3-D stimuli.
    pub fn transform2d(&self) -> Option<&Deferred<Transform2D>> { ... }

    /// Returns the 3-D transform for 3-D stimuli. None for 2-D stimuli.
    pub fn transform3d(&self) -> Option<&Deferred<Transform3D>> { ... }
}
```

`move_to` on a 3-D stimulus moves its `Transform3D.position.xz` (the horizontal plane), with
Y held fixed unless `move_to_3d` is used. This preserves backward compatibility: an animation
that writes (x, y) to a stimulus still works whether the stimulus is 2-D or 3-D.

### 9.4 `is_3d()` helper for render pass routing

```rust
impl Stimulus {
    pub fn is_3d(&self) -> bool {
        matches!(self,
            Stimulus::Box3D(_) | Stimulus::Sphere3D(_) | Stimulus::Cylinder3D(_) |
            Stimulus::Plane3D(_) | Stimulus::Corridor(_) | Stimulus::Maze(_) |
            Stimulus::Mesh(_)  | Stimulus::GaussianSplat(_))
    }
}
```

The render loop calls `scene.stimuli.values().any(Stimulus::is_3d)` to decide whether to run
the 3-D pass at all. If no 3-D stimuli are present, the depth texture allocation and the 3-D
render pass are skipped entirely — zero overhead for pure 2-D experiments.

---

## 10. Impact on the Scene State and Protocol

### 10.1 `SceneState` additions

```rust
pub struct SceneState {
    // ... existing fields ...
    pub camera:        Deferred<Camera3D>,
    pub ambient_light: Deferred<[f32; 3]>,   // RGB, used by Phong shading
    pub sun_direction: Deferred<glam::Vec3>,  // normalised, world space
    pub sun_colour:    Deferred<[f32; 3]>,
}
```

Lighting parameters are global scene properties, not per-stimulus, matching the conventions
of most real-time rendering systems. An experiment that uses only `Shading3D::Unlit` stimuli
can ignore them entirely.

**These are scene settings, so the clear commands leave them alone.** `ClearStimuli`,
`ClearAnimations` and `ClearAll` take scene *content*; the camera and the lighting survive them,
exactly as the background colour, the default fill/outline and the photodiode patch do. A
`ClearAll` that silently reset the camera would move the viewpoint out from under an experiment
that was only clearing stimuli between trials. Resetting the camera is `SetCamera` with default
values — an explicit act.

What `ClearStimuli` *must* do for 3-D is evict the render-thread caches keyed by the handles it
removed, the way text meshes are already swept (`cache.text.meshes.retain(...)` in
`render_frame.rs`). The geometry-keyed mesh cache (§1.6) is deliberately *not* per-handle, so it
is unaffected — but per-handle texture and splat-scene resources are.

### 10.2 Protobuf additions

There is no `SystemCmd` / `StimulusCmd` split. `service.proto` has a single `Request` with a
`oneof target { SystemTarget system = 1; uint32 stimulus = 2; }` and a single `oneof body`. Field
numbers below are the ones actually claimed by
[#72](https://github.com/braemons/vstimd/issues/72), verified free on `main`. **Field numbers are
the wire contract — never reuse.**

`Request.body` **75 and 76 are taken** by `clear_animations` / `clear_all`, which belong beside
`clear_stimuli = 72` in the system-mutation block. `set_camera` / `set_lighting` move to 77/78
below — nothing has shipped either number, and the clear commands are real today while the
camera is not. 79 is the next free one in that block.

`proto/vstimd/v1/vec3.proto` (`message Vec3 { float x = 1; float y = 2; float z = 3; }`) already
exists in unmerged commit `6311d41`, which also reserved `Request.body` **20–29 for 3-D creation**.
Resurrect it rather than reinventing it.

```protobuf
// Request.body — system target
CreateSphere3DRequest create_sphere_3d = 20;
CreateCube3DRequest   create_cube_3d   = 21;
SetCameraRequest      set_camera       = 77;
SetLightingRequest    set_lighting     = 78;
QueryCameraRequest    query_camera     = 85;

// Request.body — stimulus target
SetTransform3DRequest      set_transform_3d      = 64;
SetMaterial3DRequest       set_material_3d       = 65;
SetSphere3DRadiusRequest   set_sphere_3d_radius  = 66;
SetCube3DSizeRequest       set_cube_3d_size      = 67;   // full extents, per §1.7

// Response.body
QueryCameraResponse camera_info = 19;

// StimulusParams.shape oneof (stimuli/query.proto)
Sphere3DParams sphere_3d = 7;
Cube3DParams   cube_3d   = 8;

// StimulusType enum — 20–29 reserved for 3-D
STIMULUS_TYPE_SPHERE_3D = 20;
STIMULUS_TYPE_CUBE_3D   = 21;
```

```protobuf
message Transform3D {
    Vec3 position       = 1;   // world space, cm
    Vec3 rotation_euler = 2;   // degrees, yaw/pitch/roll (EulerRot::YXZ)
    Vec3 scale          = 3;   // zero → (1,1,1)
}

message Material3D {
    Color     albedo   = 1;
    Vec3      emissive = 2;
    Shading3D shading  = 3;
    reserved 4;   // was `roughness` — dropped, we do not do PBR (§B.2)
}

enum Shading3D { SHADING_3D_UNLIT = 0; SHADING_3D_PHONG = 1; }

// Note the absence of an `opacity` field, deliberately: opacity is a shared property set with
// SetAlpha after creation, exactly as for the 2-D types (CreateGrating lost its own field for
// this reason). Cube's counterpart carries `Vec3 size` — full extents, never half.
message CreateSphere3DRequest {
    string id = 1; string name = 2;          // id is a client-supplied UUID, per convention
    Transform3D transform = 3;
    Material3D  material  = 4;
    float  radius = 5;                       // cm; 0 → default 10.0
    uint32 rings = 6; uint32 sectors = 7;    // 0 → default 32
    string texture_path = 8;                 // server-side path; empty → untextured
}

// Opacity needs no 3-D message: the shared SetAlpha (body field 35) already applies.
// Sizes are full extents, matching CreateRect{width,height} and the v3 config format.
message SetCube3DSizeRequest  { Vec3 size = 1; }   // cm, full extents

message SetCameraRequest   { Vec3 position=1; float yaw_deg=2; float pitch_deg=3; float roll_deg=4;
                             float fov_y_deg=5; float near_cm=6; float far_cm=7; }
message SetLightingRequest { Vec3 ambient=1; Vec3 sun_direction=2; Vec3 sun_color=3; }
```

**Euler angles on the wire, quaternion in memory.** `Transform3D` stores a `glam::Quat`, but a
proto quaternion is miserable to construct from a Python REPL or MATLAB. Convert with
`Quat::from_euler(EulerRot::YXZ, ..)`, and **document the rotation order in the `.proto`** — this
is exactly the kind of thing that silently differs between clients.

**Zero means default.** proto3 cannot distinguish "unset" from "zero" for scalars, and the
codebase has already made its peace with that (`cmd_create_grating`: `if cmd.width == 0.0 { 200.0 }`).

**Textures are `texture_path`, a server-side filesystem path.** No upload-over-the-wire, no asset
manager. Decoding happens on the ZMQ thread at create time (§B.6).

### 10.3 Per-stimulus commands for 3-D

The existing per-stimulus commands (`SetPosition`, `SetEnabled`, `SetOrientation`, `Delete`,
`BringToFront`, …) apply to 3-D stimuli unchanged in semantics, given the `move_to` / `set_angle`
mapping in §9.3. The 3-D-specific additions are the four stimulus-target entries listed above.

`SetCamera` and `SetLighting` are **system-targeted** and must respect `deferred_mode`, writing to
the `copy` slot so they flip atomically with stimulus updates (§13.4).

---

## 11. Impact on Animations

### 11.1 The camera as an animation target

The camera participates in the animation system, so that no new animation types are needed just
for camera control. `ExternalPosition2D` (shared memory), `MoveAlongPath2D`,
`MoveAlongSegments2D` and the rest work on the camera out of the box.

> ⚠ **Do not implement the `CAMERA_HANDLE` sentinel this section used to propose.** It read:
> *"`CAMERA_HANDLE = 0x0000_FFFE` (below the animation range `0x8000` but safely outside the
> stimulus range)"*. Two errors. First, `0xFFFE` is 65534 and `0x8000` is 32768 — the sentinel is
> **above** the range it claims to be below. Second, **no reserved range exists**:
> `alloc_stim_handle` and `alloc_anim_handle` (`scene/scene_state.rs`) are independent counters
> both starting at `1`, and `load_config` in additive mode *sums* the offsets. Nothing stops a
> stimulus from eventually being handed 65534. The `0x8000–0xFFFE` split lives only in the message
> of unmerged commit `6311d41`.

Animations already target a **list** of stimuli — `AnimationEntry.config.stimuli: Vec<u32>`, fanned
out in `advance_one`. Replace that field with an explicit target:

```rust
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "target")]
pub enum AnimationTarget {
    Stimuli(Vec<u32>),   // existing behaviour
    Camera,
}
```

No magic numbers, survives a config round-trip, representable in proto as a `oneof`, and
`advance_one` gets one `match` instead of a handle comparison buried in a loop. Old configs with a
bare `stimuli: [1, 2]` array must still load — cover it in `server/tests/config_compat.rs`.

Mapping a 2-D animation onto the camera: `(x, y)` writes `position.xz`, holding `position.y` — the
same rule §9.3 applies to `move_to` on a 3-D stimulus.

### 11.2 New animation types for 3-D navigation

Two new animation types are valuable for 3-D experiments:

#### `Flythrough3D`

Follows a preloaded camera path (position + orientation keyframes, interpolated with
Catmull-Rom splines). Used for passive viewing paradigms where the animal watches a
pre-recorded trajectory through a virtual environment.

```rust
pub struct Flythrough3D {
    pub keyframes:    Vec<CameraKeyframe>,
    pub time:         f32,            // current time, seconds
    pub speed:        f32,            // playback rate
    pub final_action: FinalActionMask,
    // target is AnimationTarget::Camera (§11.1) — no handle field
}

pub struct CameraKeyframe {
    pub time:    f32,
    pub pos:     glam::Vec3,
    pub yaw:     f32,
    pub pitch:   f32,
}
```

#### `LinearNav3D`

Moves the camera along the local forward axis at a commanded speed. Used for treadmill-driven
linear corridor navigation, where the hardware reports **velocity**, not absolute position — this
is cleaner than `ExternalPosition2D` in that case.

```rust
pub struct LinearNav3D {
    pub speed_cm_s:      Deferred<f32>,   // driven by treadmill via SetAnimParam
    pub wrap_period_cm:  Option<f32>,     // Some(L) → wrap into [0, L); None → unbounded
    pub final_action:    FinalActionMask,
}
```

`SetAnimParam(mode=0, value=speed_cm_s)` updates the speed every frame from treadmill data.

**Wrapping is required, not an optimisation.** An endless corridor cannot have endless geometry
(§C.0). So `LinearNav3D` maintains **two** values:

- `distance_travelled_cm: f64` — the **true**, monotonically increasing distance. This is what the
  experiment logs and what behavioural analysis needs. Exposed via `QueryAnimation`.
- `camera.position` — wrapped into `[0, L)` along the corridor axis. This is what the renderer
  sees.

Keep them separate. Collapsing them — wrapping the logged value, or logging the wrapped value —
silently destroys the trial data, and it is the kind of bug that surfaces months later in analysis.

Use `f64` for the accumulator: `f32` has a 24-bit mantissa, so at 10 000 cm the spacing is already
~1 mm and a session running for kilometres degrades further. The rendered position stays in `f32`
and stays small *because* it is wrapped.

### 11.3 Existing animations that work for 3-D without modification

Names below are the **actual** `AnimationKind` variants (`scene/animation/animation_kind.rs`);
earlier drafts of this table used invented `Anim*` names. The existing variants already carry a
`2D` suffix, so 3-D kinds slot in beside them without a rename.

| Animation | 3-D use case |
|---|---|
| `ExternalPosition2D` | Absolute position from eye tracker / treadmill → `position.xz` |
| `MoveAlongPath2D` | Camera follows a preset path |
| `MoveAlongSegments2D` | Camera follows a piecewise-linear corridor path |
| `FlashForNFrames` | Briefly show a 3-D object |
| `FlickerForNFrames` | Flicker a 3-D object |
| `CoupleVisibilityToTriggerLine` | Gate a 3-D object on a VTL edge |
| `EnableOnTriggerEdge` | Show a 3-D object when a trigger fires |

---

## 12. Crate Dependencies for 3-D

```toml
# Phase A (infrastructure)
glam = { version = "0.29", features = ["bytemuck", "serde"] }

# Phase B (textured primitives)
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }

# Phase D (mesh loading)
gltf = "1"       # glTF 2.0 parser, pure Rust
tobj = "4"       # OBJ/MTL parser, pure Rust, fallback format

# Phase E (Gaussian splatting)
ply-rs = "0.1"        # .ply parser for trained 3DGS scenes
gpu-allocator = "0.27" # sub-allocation; Vulkan caps maxMemoryAllocationCount (§E.2)
# No dedicated 3DGS crate exists yet; the renderer is implemented directly in
# compute shaders (WGSL → SPIR-V via the existing naga build step).
```

`glam` is the only hard new dependency before Phase B. It has no transitive native dependencies
and compiles in seconds. The `serde` feature is required because `Transform3D` is serialized into
the scene config JSON.

`image` needs `default-features = false` — it otherwise pulls in a dozen decoders.

---

## 13. Open Questions

### 13.1 Depth buffer precision and corridor length

Corridors and mazes can be long (tens of metres). The default `near=1 cm, far=50 000 cm`
gives a depth precision of approximately 0.02 cm at 10 m and 5 cm at 500 m with a 32-bit
float depth buffer. This is adequate for most experiments. If longer ranges are needed,
a **reversed-Z** depth buffer (`near=far, far=near` in the projection, `DepthCompare::Greater`)
dramatically improves precision at distance and is straightforward to implement in Vulkan.

### 13.2 Anti-aliasing

The current 2-D pipeline uses no anti-aliasing (not needed for sharp stimulus edges). For 3-D
scenes, especially corridors with sharp wall edges, aliasing can be distracting. Options:

- **MSAA ×4**: `sample_count = 4` on the 3-D pipeline plus a multisampled colour attachment and a
  resolve into the swapchain image. Low complexity, good quality for geometry.
- **FXAA / TAA**: post-process passes. More complex but work on any hardware.
- **None**: may be acceptable for experiments where the animal's perception of aliasing is not
  a confound (e.g. optic flow experiments with textures rather than edges).

**MSAA is more attractive than this section originally judged**, for two reasons that emerged
later. First, 3-D is scoped to Jetson and discrete-GPU x86 (§1.9), where 4× MSAA is cheap — the
cost objection applied mainly to the Pi's tile-based V3D, now out of scope. Second, because the
3-D pass owns its own attachments (§2.2), it can be multisampled **without the 2-D pass changing
sample count**: render-pass compatibility is per-pass, so the 2-D pipelines stay at `TYPE_1` and
stay untouched. The separate-pass structure makes this decision reversible rather than
load-bearing.

Still deferred until there is a corridor edge to look at.

Related: 3-D textures ship **without mipmaps** in Phase B, so a minified textured sphere will
shimmer. Add mipmap generation when it becomes visible.

### 13.3 Mixing 2-D and 3-D stimuli: Z-fighting and ordering

If a 2-D billboard (e.g. a fixation cross) needs to appear embedded in the 3-D scene (not
always on top), the current "2-D pass has no depth test" approach is insufficient. Options:

- **Render the billboard in the 3-D pass** as a camera-facing quad, with a depth value. This
  requires the billboard to be a `PlaneStimulus3D` with `Shading3D::Unlit`.
- **Keep all 2-D stimuli on top** (current design). This works for fixation crosses, cues, and
  overlays. It does not work for stimuli that should be occluded by 3-D geometry.

The current design (2-D always on top) is correct for the majority of use cases and is kept
as the default. A `Stimulus::Billboard3D` variant that renders in the 3-D pass is a possible
Phase B or C addition.

### 13.4 Deferred mode and 3-D

The `Deferred<T>` mechanism works identically for 3-D stimuli. The only additional
consideration is that `Deferred<Camera3D>` must be flipped atomically with all stimulus flips,
so that a batch update of camera + stimulus positions all becomes visible on the same frame.
This is handled automatically: the `pending_flip` loop in the render thread iterates
`SceneState` and calls `scene.camera.flip()` alongside `stimulus.flip()` for each stimulus.

### 13.5 Gaussian splatting and the depth buffer

3DGS rendering uses alpha blending over sorted Gaussians, producing a correct composited
image. However, it does not write to the depth buffer, so 3-D geometry drawn after the splat
pass would appear in front of it. If a scene mixes 3DGS backgrounds with explicit 3-D objects,
a depth pre-pass or screen-space depth reconstruction may be needed. This is an active research
problem and is explicitly out of scope until Phase E is reached.

---

*End of document. See `PLAN.md` for phase ordering, `STIMULUS_DATA_MODEL.md` for the
composition model, and `INPUT_LATENCY.md` for position control design.*