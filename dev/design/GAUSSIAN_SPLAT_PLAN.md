# Gaussian splat corridors

Exploration for rendering a corridor from a trained 3-D Gaussian splat (3DGS) scene instead of
generated planes. Builds on `dev/3D_ROADMAP.md` §8 (Phase E), which only kept the door open;
this document checks that door against the 3-D code as it landed on `0.3`.

## 0. Status (2026-09-16, branch `feat/gaussian-splat-corridor`)

Phase 1 is implemented, following this plan with the decisions below.

**Where to pick up (2026-09-16):** performance is understood and written up in
§10 — the scene is fill-bound, and the hardware-rasterised path cannot
early-terminate, which is the whole gap. §11 starts the tile rasteriser that
closes it. Stage 1 and the stable GPU radix sort behind stage 3 are written and
tested (§11.1); §11.2 lists what is left. An FX panel (F8) carries the knobs measured so far.
On an RTX 4070 the fill-bound wall does not reproduce even at 9.13 M splats
overlapping (§10) — the tile rasteriser stops being urgent on desktop-class
GPUs and stays needed for the Jetson Orin Nano. A separate, unexplained 2×
frame-time regression turned up moving the camera far from a multi-scene
stack; see §10's last paragraph before relying on any "camera far away" probe.


- **`GaussianSplat3D`** stimulus (`StimulusBody::GaussianSplat`), created from a
  server-side `.ply` or `.splat` path — interim until the asset store. The path is
  probed at create time (bad path → `INVALID_ARGUMENT`) and loaded on a thread.
- **Kept separable.** Splatting is a prototype, so it is additive: everything
  splat-specific lives in a file of its own, and the shared files carry only the
  entries that cannot live anywhere else. Removing splatting is deleting those
  files and the entries below.

  | Own file | |
  |---|---|
  | `proto/vstimd/v1/stimuli/gaussian_splat.proto` | `GaussianSplat3DParams`, `CreateGaussianSplat3DRequest` |
  | `server/src/ipc/gaussian_splat_commands.rs` | the create command |
  | `server/src/ipc/convert/gaussian_splat.rs` | proto <-> scene |
  | `server/src/scene/stimulus/gaussian_splat.rs` | the stimulus |
  | `server/src/render/vk/vk_splat_pipeline.rs`, `.../cache/splat_cache.rs`, `server/shaders/splat.wgsl` | the render path |
  | `client/python/vstimd/stimuli/gaussian_splat_{client,models}.py` | `conn.stimuli.gaussian_splat` |
  | `client/python/tests/e2e/cases/test_gaussian_splat.py` | its e2e case |

  In shared files the whole wire footprint is three lines — one `Request` arm
  (`service.proto` 24), one `StimulusParams` arm (`query.proto` 12) and one
  `StimulusType` value (24) — each marked with a comment pointing back here. The
  Rust match arms that go with them are enforced by exhaustive matches, so the
  compiler lists them if they are ever removed.

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
locked, which throttles the window to 1 Hz. **Still to do:** frame time on the
Jetson Orin Nano (desktop done — §10), a validation-layer run, and the web
client. `.ply` is done: `bonsai-7k.ply` (the 3DGS reference layout) draws
correctly on screen at the same placement the `.splat` captures use, so the
loader is confirmed against a real scene and not only unit tests.

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

## 10. Performance: where the time actually goes (2026-09-16)

Measured on the desktop (GTX 1650, `room-7k.splat`, 1.13 M splats), against the
hardware-rasterised path as it stands. **The scene is fill-bound, not
instance-bound**, and by a wide margin:

| configuration | frame cost |
|---|---|
| 1.13 M splats, camera inside the room | 8.04 ms |
| 1.13 M splats, camera 200 m away — same instances, no fill | **0.36 ms** |

So 95% of the cost is blended fragments. Two corollaries, both measured, both
the opposite of what looked obvious:

- **Shrinking quads to the 1/255 boundary buys nothing** (8.04 → 8.04 ms). The
  fragments it removes were already being `discard`ed, and a discard is nearly
  free. Only fragments that *blend* cost anything.
- **Raising the visibility floor does buy something** (8.04 → 5.70 ms at 8/255,
  ~1% mean pixel difference), because that removes fragments that were blending.
  This is the FX panel's `alpha floor`.
- Clamping the longest drawn axis is weak here: nothing at 128 px, 5% at 32 px.
  This scene's fill is many moderate splats, not a few huge near ones.

Cost is also strongly superlinear in resolution — 1.4 Mpx costs 0.21 ms, 3.6 Mpx
costs 8.04 ms — which is what heavy overdraw into a blender looks like.

**On an RTX 4070** (2026-09-16, windowed 1280×720, hardware-rasterised path, no
FX-panel knobs touched). First, eyeballed: `room-7k.splat` (1.13 M splats)
holds a steady 100 fps — the display's own refresh cap — at ~40% GPU
utilisation, camera inside the room. Then measured with
`SystemClient.query_frame_stats`, three MipNeRF-360 captures loaded
simultaneously and made to overlap at the origin (`room-7k.splat` 1.13 M +
`garden-7k.splat` 4.39 M + `bicycle-7k.splat` 3.62 M = **9.13 M splats**,
`scale=100`):

| configuration | mean frame interval | dropped |
|---|---|---|
| single `garden-7k.splat` (4.39 M), camera walking through | 10.002 ms | 0/320 |
| all three stacked (9.13 M), camera inside, at the origin | 10.002 ms | 0/240 |
| all three stacked (9.13 M), camera moved to `z=20000` cm | 20.004 ms | 240/240 |

The display's nominal frame interval is 10.002 ms (99.981 Hz), so the first two
rows are a perfect lock — the fill-bound wall the GTX 1650 hit at 1.13 M splats
is nowhere near visible here even at 8× the splat count, overlapping. The
headroom means the tile rasteriser is not urgent on this GPU at this
resolution; it stays needed for the Jetson Orin Nano target, and a
denser/closer scene could still find a wall here too.

The third row is the surprise, and is **not yet explained** — flagging it per
the warning just above rather than guessing. The naive story ("far away removes
fill") predicts *faster*, as it did for the room alone (§10 top: 8.04 ms →
0.36 ms on the GTX 1650). Instead it locked to exactly half rate. One fact that
complicates the naive story: `garden-7k.splat` and `bicycle-7k.splat` are
whole MipNeRF-360 captures, not a small room — bounding boxes (scene units,
`×100` = cm) are roughly `[-63, 86]` and `[-67, 85]` on `bicycle` and
`[-43, 53]` and `[-24, 43]` on `garden`, i.e. 50–85 m across at this scale, so
`z=20000` cm (200 m) is not obviously clear of them the way it was clear of the
4 m room. That alone would predict *less* shrinkage than expected, not a clean
2× regression, so something else is likely going on (sort contention across
three simultaneous scenes, a per-instance cost that does not fall off with
distance, or something in the depth/culling path) — worth a follow-up with the
wireframe toggle or a frame capture before trusting either scene at long range.

**Beware a probe that does not do what it says.** An earlier round concluded
"fill is free" from turning the camera 180°. The room capture surrounds the
camera, so turning around renders just as much; it never removed any fill.
Moving the camera far outside the scene is the test that separates the two.

### 10.1 Why this renderer is the slow shape

The reference rasteriser (Kerbl et al.) is tile-based: 16×16 tiles, per-tile
depth sort, **front-to-back** blending, and each tile stops once transmittance
saturates. In a dense scene a pixel is opaque after a few dozen splats and the
hundreds behind it are never touched. vstimd uses the fixed-function pipeline —
one instanced quad per splat, back-to-front `ONE, ONE_MINUS_SRC_ALPHA` — which
**cannot** early-terminate: every fragment must blend. That is the whole gap.

2026 work worth reading: **HiGS** (arXiv 2606.00352) decouples binning
granularity from rasterisation granularity and reports up to 15.8× over 3DGS,
rendering-only, and — directly relevant here — frame time growing 1.6× from
1080p to 4K where the baseline grows 2.8×. **VkSplat** (arXiv 2605.00219) is the
Vulkan-compute reference, with exact Gaussian-tile overlap tests. **LODGE** and
**FilterGS** cover LOD for large scenes. **RadSplat** (arXiv 2403.13806) and
**LightGaussian** prune by per-Gaussian importance scored over the training
views.

### 10.2 The corridor is worth more than it looks

Camera motion is 1-D, which buys three things:

1. **Sorting is already solved, by accident.** §4's order depends only on the
   view *axis*, never position, so a straight walk sorts once and never again.
   This is why sorting never appears in any measurement above.
2. **Pruning can be scored against the actual track.** RadSplat's importance is
   `max` blending weight over the *training views*, because general rendering has
   to allow any view. Here the set of views an experiment will ever render is a
   known 1-D track, so the score is exact rather than a proxy, and the pruning
   can be far more aggressive than any published figure. This belongs in
   `vstimd-scene-from-capture`, which owns the scene end to end.
3. **Per-segment visible sets** drop straight into the existing design: the
   order buffer is already a `u32` index list, so a segment's set is a shorter
   one, and culling becomes a table lookup.

Note which splats actually cost: screen area goes as (size/distance)², so fill
is dominated by *near* splats. Culling removes instances, and instances are
0.36 ms of 8 ms. Pruning helps because it removes overlapping faint splats that
blend — the same reason the alpha floor works — not because it shortens the draw.

### 10.3 The "camera far away" probe, and why it inverted

The 2× regression at `z=20000` in the table above has a mechanism, offered here
as a **hypothesis with a test, not a measurement** — the desktop window was
compositor-throttled to 1 Hz when this was written, so it could not be checked
on the GTX 1650 either.

Two things work against the naive "far away removes fill" story, and both get
worse as the scene gets bigger:

1. **`DILATION = 0.3` px² is a floor, not a tweak.** It is added to the
   projected 2-D covariance precisely so "a splat never covers less than about a
   pixel" (`splat.wgsl`). So the fragment count can never fall below roughly one
   or two pixels *per drawn splat*, however far away the camera goes. Distance
   does not shrink the fill toward zero; it concentrates it into fewer pixels.
2. **Distance removes frustum culling.** Inside a capture the camera is
   surrounded, so a large share of splats are behind it or outside the frustum
   and take `culled()`'s early-out. From outside, the whole scene is in view and
   *every* splat survives to rasterise.

Together: far away there are *more* rasterising splats, each pinned at ~1–2 px
by the dilation, piled into a small screen region — and blending is a serialised
read-modify-write per pixel. For 9.13 M splats that is on the order of 18 M
fragments concentrated into a few tens of thousands of pixels. That is closer to
a worst-case fill probe than a no-fill one. It only behaved on the GTX 1650
because that scene was 1.13 M splats in a 4 m room.

Three predictions, cheap to run, that would confirm or kill it:

- moving *further* (`z = 200000`) should cost the **same**, not less — the
  dilation floor pins per-splat area regardless of distance. This is the sharp
  one: distance-independence implicates the floor directly.
- far-away cost should scale about linearly with total splat count.
- raising the FX panel's alpha floor should cut the far-away case hard.

If it holds, the fix is a **screen-size cull**: drop splats whose projected
extent is below a threshold before they reach the rasteriser. They cost a full
fragment each and contribute almost nothing, and this is what the LOD papers in
§10.1 do by other means. Cheap, and it helps the Orin Nano more per unit of
effort than the tile rasteriser does.

## 11. Tile rasteriser (in progress, WIP)

The long-term fix, chosen over the alpha floor and a half-resolution 3-D pass
because those two buy speed by changing what reaches the screen, which a
calibrated stimulus should not do if it can be avoided. They remain available on
top if this is not enough.

Goes in **alongside** the hardware path, toggled from the FX panel, so the two
can be compared on speed and on image, and so it stays removable like the rest of
the prototype.

| stage | what |
|---|---|
| 1. preprocess (compute) | project each splat once, cull, inverse 2-D covariance, pixel bound, histogram the tiles touched |
| 2. scan | per-tile counts → per-tile offsets |
| 3. scatter (compute) | splat indices into per-tile slots, ordered |
| 4. rasterise (compute) | one workgroup per tile; batch into shared memory, blend front-to-back, stop on saturated transmittance, depth-test against the mesh depth buffer |
| 5. composite | storage image into the 3-D pass |

Stage 1 is written (`shaders/splat_tile_preprocess.wgsl`) and validating. It
reads the existing `order` buffer, so §4's sorter keeps earning its keep.

### 11.1 The per-tile sort — decided, and built

**Decided: write the radix sort** (`shaders/radix_sort.wgsl`,
`render/vk/vk_radix_sort.rs`). Done and tested.

Why it was a decision at all: the global order removes *depth* from the sort
key — that much the corridor gives us — but not the need for the sort to be
**stable**. An `atomicAdd` scatter leaves an arbitrary order inside each tile,
and splats blend, so that is a stimulus whose pixels change between frames for
no reason. A stable counting sort with 14 400 buckets would need a
per-block-per-bucket offset matrix, which is why real GPU radix sorts work in
7–8 bit digits.

Shape: least-significant-digit, 4-bit digits, 256 items per block, three kernels
(histogram, scan, scatter) run once per digit. Stability comes from two places —
within a block, four single-bit splits, each of which preserves the relative
order of equal elements; across blocks, a digit-major scan of the histogram, so
every `(digit, block)` pair gets a global slot that respects block order. The
pass count is rounded to even so the ping-pong ends in the caller's buffers.

It takes a `&ash::Device` and a command buffer and owns nothing else, so the
renderer drives it with its own queue and `server/tests/radix_sort.rs` drives it
with a headless one — no window, no rig, no display. That test asserts
*stability*, not just sortedness: values are input positions, so the assertion
reads as "equal keys kept their input order". It covers a block exactly, a
partial tail block, 100 000 pairs over 14 400 distinct keys (the real tile case),
all-equal keys, all-distinct keys, and a full 32-bit key. It skips rather than
fails where there is no Vulkan.

### 11.2 Still to do

Stages 2–5. The sort is stage 3's engine; what remains around it:

- expand each splat's tile rectangle into `(tile_id, splat_index)` pairs, which
  needs a scan over per-splat tile counts to allocate the pair array, and a cap
  plus a graceful overflow path since the pair count is data-dependent
- per-tile ranges from the sorted pairs (find each tile's first and last)
- the rasterise kernel: one workgroup per tile, batch into shared memory, blend
  front-to-back, stop once transmittance saturates, depth-test against the mesh
  depth buffer (which needs `SAMPLED` usage and a layout transition — splats
  currently test against it through fixed-function state, see
  `vk_splat_pipeline.rs`)
- composite the storage image into the 3-D pass, and an FX-panel toggle to pick
  between this path and the hardware one so they can be compared on speed and on
  image

