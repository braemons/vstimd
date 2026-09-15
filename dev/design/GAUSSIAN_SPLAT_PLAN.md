# Gaussian splat corridors

Exploration for rendering a corridor from a trained 3-D Gaussian splat (3DGS) scene instead of
generated planes. Builds on `dev/3D_ROADMAP.md` §8 (Phase E), which only kept the door open;
this document checks that door against the 3-D code as it landed on `0.3`.

## 0. Status (2026-09-15, branch `feat/gaussian-splat-corridor`)

Phase 1 is implemented, following this plan with the decisions below:

- **`GaussianSplat3D`** stimulus (`StimulusBody::GaussianSplat`), created from a
  server-side `.ply` or `.splat` path — interim until the asset store. The path is
  probed at create time (bad path → `INVALID_ARGUMENT`) and loaded on a thread.
- **CPU sort on a worker thread** (§4), instanced draw from storage buffers, SH degree 0.
- **Finite track with a fade** (§6 option 1): `LinearNav3D.track_length_cm` +
  `fade_frames`. At the end the 3-D view fades to the background (a veil drawn last
  in the 3-D pass, so 2-D is untouched), the camera jumps back to its start
  position and heading, and the view fades in.
- Sample scene for development: `room-7k.splat` (1.13 M splats) from
  `huggingface.co/datasets/dylanebert/3dgs` (Inria MipNeRF-360 capture; research
  licence — do not commit or ship it). Placement:
  `rotation_deg = [0, 180, 0]`, `scale = 100`.

Measured on the desktop (GTX 1650, `room-7k.splat`): read 60 ms, upload 100 ms,
one sort 16 ms. Frame time could not be measured — the session's screen was
locked, which throttles the window to 1 Hz. **Still to do:** frame time on this
desktop and on the Jetson Orin Nano, a validation-layer run, `.ply` checked
against a real scene on screen (only unit-tested so far), and the web client.

## 1. Verdict

**The 3-D stack is a good host, but it cannot render splats today.** Everything around the
renderer — the pass structure, camera, navigation, conditions, 2-D overlay — carries over
unchanged. The renderer itself is missing five things, all additive: none of them touches the
mesh3d pipeline or the 2-D path.

| Area | Status on `0.3` | Needed for splats |
|---|---|---|
| Lazy `[colour, depth]` 3-D pass, 2-D overlaid after | ✅ `vk_pass3d.rs` | Splats draw in the same pass, after opaque meshes |
| `Camera3D` (pose, `fov_y_deg`, near/far) | ✅ | Also view matrix and focal length in px in the uniform (§3.2) |
| Navigation: `LinearNav3D`, device-driven transforms, treadmill, camera zones | ✅ | Unchanged — splats only see the camera |
| Conditions, opacity, visibility | ✅ generic over `Stimulus` | Unchanged |
| Colour space | ✅ `B8G8R8A8_UNORM` swapchain, blending in display space | Matches how 3DGS is trained — no conversion |
| WGSL → SPIR-V via naga in `build.rs` | ✅ | Compute shaders compile the same way (only for §4 phase 2) |
| Per-instance data | ❌ one draw + push constants per instance | Instanced draw from a vertex-rate buffer (the dots pipeline already does this) |
| Transparent pipeline | ❌ depth write on, `dst_alpha = ZERO` | Depth test on, depth write off, back-to-front "over" blend |
| Sorting | ❌ none (depth buffer sorts opaque geometry) | Per-view depth sort of every splat |
| Large buffers | ⚠️ one `VkDeviceMemory` per buffer | Fine: a splat scene is a handful of large buffers, not thousands of small ones |
| Loading a scene file | ❌ | `.ply` (and ideally `.spz`) parser; the file is an **asset** (§5) |
| Endless corridor | ⚠️ camera wraps in `[0, L)` — needs periodic content | A scanned corridor is not periodic (§6) |

## 2. What a splat scene is

Per Gaussian, from the reference `.ply`: position (3), scale (3, log), rotation quaternion (4),
opacity (1, logit), SH DC colour (3), SH rest (45 at degree 3). Rendering: project each 3-D
covariance to a 2-D screen ellipse, draw a quad sized to ~3σ, evaluate the Gaussian in the
fragment shader, composite back to front.

Sizes: a room-scale corridor is typically 0.3–2 M Gaussians. At SH degree 3 that is ~250 B each
(≈ 250–500 MB); at degree 0 ~60 B (≈ 60–120 MB). Jetson Orin Nano shares 8 GB between CPU and
GPU, so **ship SH degree 0 or 1 first**. View-dependent colour matters little in a corridor seen
from one height and heading.

## 3. Renderer changes

### 3.1 A new body arm, not a mesh3d variant

`StimulusBody::GaussianSplat` — its own pipeline and cache, per the rule that the body enum is the
renderer's taxonomy. User-facing type `GaussianSplat3D`. Fields, following the naming rules:
`transform` (`Transform3D`: `position_cm`, `rotation_deg`, `scale` — a scan's units are arbitrary,
so the scale to centimetres is required, not optional), the file path (an asset reference later),
and the shared `common` opacity multiplying every Gaussian's alpha. `sh_degree` waits until
view-dependent colour is wanted; phase 1 reads only the DC term.

### 3.2 Pipeline

- **Cache** (`cache/splat_cache.rs`): on first sight of an asset, parse on a loader thread,
  upload once to device-local buffers. Never on the render thread.
- **Draw**: one unit quad, `instance_count = N`, instance data read through a sorted index
  buffer (`instance → splat id`) from a read-only storage buffer. A read-only storage buffer in the
  vertex stage needs no extra device feature on Vulkan 1.1.
- **Vertex shader**: 3-D covariance from scale + rotation (precompute on upload to save per-frame
  work), EWA projection with the Jacobian. It needs the model-view matrix and the projection's
  scale terms, which fit in push constants (96 bytes), so `SceneUniform` and mesh3d are untouched.
- **Blend**: depth test `LESS_OR_EQUAL` against meshes drawn first, depth write off,
  `src = ONE, dst = ONE_MINUS_SRC_ALPHA` with premultiplied colour.
- **Opaque meshes coexist**: a sphere cue inside a splat corridor occludes correctly; splats do not
  occlude meshes behind them (no depth write). Acceptable for cues; say so in the docs.

## 4. Sorting — the one real decision

**Phase 1: CPU sort on a worker thread, stale by a frame or two.** Every browser splat viewer
does this. A 32-bit depth key, 16-bit-pass radix sort of 1.13 M entries measured 16 ms on one
desktop core, off the render thread.

The order depends only on the direction of the view axis in the cloud's frame, never on the
camera's position: view depth is `dot(row₂(V·M), p) + const`. **A camera walking straight down a
corridor never re-sorts**; only turning does. The render thread posts the view axis and copies
the newest finished order into its mapped index buffer, both behind `try_lock`, skipping rather
than waiting when contended — no blocking, and no allocation once running. At mouse walking speeds (< 1 m/s) the order changes far slower than the frame
rate; artefacts show only on fast rotation. Needs no compute pipeline at all.

**Phase 2, only if Phase 1 misses frame timing on the Orin: GPU radix sort in compute.** Adds a
compute pipeline, storage-buffer descriptor sets and a compute dispatch before the 3-D pass. Port
the WGSL from `web-splat` (MIT). Measure first.

A per-frame record of *which* sort the frame used is cheap and belongs in the frame log, since
this is the one place a frame shows stale content by design.

## 5. Loading: waits on the asset store, like textures

A splat scene is a file of tens to hundreds of MB. It is an asset, and joins the asset store
(`dev/design/ASSET_STORE_PLAN.md`) when that lands. Until then — decided for this branch — the
create request carries a server-side path, with no cargo feature; the cache is keyed by stimulus
handle, so the switch to `AssetRef` changes the key and the request field, nothing else.

Formats: `.ply` (reference, uncompressed) first; `.spz` (Niantic, ~10× smaller, MIT) second — it
matters for moving scenes to a Jetson.

## 6. The corridor problem: a scan is not periodic

The corridor today is endless because the camera wraps in `[0, L)` and the geometry repeats every
`L`. A splat scan of a real corridor has neither property. Options:

1. **Finite track** (chosen, with a fade on the jump back). Non-wrapping `LinearNav3D`, trial ends at the end of the
   scan. The roadmap already names this as a separate stimulus; a scan is the natural content for
   it, including unique landmarks.
2. **Tiled segment.** Scan (or crop) one period of length `L`, draw it at `z + k·L` offsets as
   the mesh corridor does. Works with the wrapped camera, but the seam is visible unless the
   segment was captured or edited to be seamless. Instancing copies multiplies the sort size.
3. **Synthetic splats** from the corridor generator. Proves the pipeline without a capture rig,
   but has no reason to exist beyond that — useful as a test fixture only.

## 7. Content pipeline (outside vstimd)

Walk-through video of the corridor → COLMAP poses → train with `gsplat` / nerfstudio `splatfacto`
or Postshot → crop to the corridor → export `.ply`. Two things vstimd users must be told:
the scale is arbitrary (measure one known length and set `transform.scale`), and COLMAP's
axes are Y-down / Z-forward while vstimd's camera looks down −Z with Y up, so a 180° rotation
about X is the usual import fix. Baked colours are not calibrated luminance.

## 8. Order

1. ✅ Instanced splat pipeline, CPU sort, synthetic test splats (the `3D-09` e2e case).
2. ✅ `.ply` and `.splat` loaders; a public scene renders on the desktop.
3. ✅ Finite track with a fade (option 1 of §6).
4. ✅ Proto (`GaussianSplat3D`, `LinearNav3D.track_length_cm`/`fade_frames`), Python client.
5. ⏳ Frame time on the desktop GTX 1650 and the Jetson Orin Nano at 0.5 M and 1 M Gaussians.
6. ⏳ A corridor scanned for real; asset store integration when it lands.
7. GPU sort, `.spz`, tiled periodic segments — each only if measured need.

## 9. Open questions

- Answered: a finite track is fine, with a fade on the jump back; paths are used until the asset
  store (no cargo feature); a public sample scene stands in until a corridor is scanned.
- Target Gaussian count — sized once a real corridor is scanned.
- Should the jump back emit an event or pulse a trigger line, so a lap boundary lands in the
  experiment record? Not done; `distance_travelled_cm` never counts the jump.
