# Random Dot Kinematogram Plan

Design for issue #136, family **A** (classic 2-D RDK). Family B (formless dot-field
structure-from-motion, Singer & Sheinberg 2008) is out of scope here; §8 records the
two decisions taken now so that B is not painted out.

The first deliverable is not "an RDK stimulus" in the abstract. It is **a faithful
reproduction of the figure-ground RDK** in
`stimulusStageFigureRDBackgroundComponentsCLaser_BW.m`, the Psychtoolbox stimulus this
lab actually runs. That stimulus is the acceptance test, and §1 reads it closely
because five of its properties are things the issue's parameter table does not yet
express.

---

## 1. The reproduction target

### 1.1 What the MATLAB actually draws

Two independent dot fields, drawn into one frame over a uniform grey background
(`backgroundCol = 128`):

- a **background** field, drawn only *outside* a circle,
- a **figure** field, drawn only *inside* that same circle,

with the same dot size, density and speed in both, differing only in **direction**.
Nothing but motion distinguishes figure from ground — no luminance, density or
texture cue in any single frame. The circle is centred on the receptive field
(`xc`, `yc` = `screenInfo.RFcenter`). MATLAB writes it as `R = 45/2`, a **radius** of
22.5 deg; vstimd sizes everything by its full extent, so `aperture.size_px[0]` is the
**45 deg diameter** (§1.3).

Per field, the generation (stage `load`, once per trial) is:

```matlab
orgPos      = (rand(2, (2N+1)^2) - 0.5) * 2 * dotJitter    % ±200 px
angles      = rand(1, (2N+1)^2) * 2*pi
targetDir   = [cos(dirAngle); sin(dirAngle)]
nonRandInds = a random cohProp fraction of the dots
dirs(:, nonRandInds) = targetDir                            % signal dots
dirs(:, randInds)    = [cos(angles); sin(angles)]           % noise dots
```

and the per-frame position (stage `stim`) is closed-form in the frame index:

```matlab
xtot = centre_x + xism + orgPos(1,:) + frameIndex/FrameRate * dirs(1,:) * vel
ytot = centre_y + yism + orgPos(2,:) + frameIndex/FrameRate * dirs(2,:) * vel
```

`xism`/`yism` is a square lattice of `(2N+1)² = 161² = 25921` sites at
`dotSpacing = 5 deg`. **The lattice is a generation trick, not a percept.** The jitter
is ±200 px while the spacing is ~100 px at a typical 20 px/deg, so each dot is
displaced by about twice the lattice period: the result is indistinguishable from a
uniform random field of density `1 / (5 deg)² = 0.04 dots/deg²`. Reproducing it means
reproducing a uniform field at that density, not reproducing a lattice.

The lattice spans 805 deg — enormously larger than the screen. That is how the
stimulus supplies dots streaming in from off-screen for the whole 2 s trial (120
frames at 50 deg/s = 100 deg of travel) with **no wrapping, no respawn and no
lifetime**. Dots are born once, move in a straight line forever, and are simply culled
when off-screen.

### 1.2 The five properties the issue's table cannot express

| # | Property of the MATLAB stimulus | Status against the issue's parameter table |
|---|---|---|
| 1 | The aperture (circle, 45 deg across) is far smaller than the field the dots live in, and the background field uses the **complement** of that circle | `field_shape`/`field_size_px` do both jobs at once, and there is no invert |
| 2 | A dot is clipped by its **centre**, not per pixel: `if figureMask(y,x)` tests one pixel and then blits the whole dot mask, so dots overhang the figure boundary uncut | not mentioned |
| 3 | `bwSameTrial` assigns each dot black **or** white at birth, on a grey background | `dot_color` is a single RGBA |
| 4 | `noFigureFrames` switches the figure field from the background's direction to the figure's direction *mid-trial, from wherever the dots currently are* | a direction change recomputed from `t = 0` would teleport every dot |
| 5 | Density (dots/deg²) is the invariant; the count follows from the field size | `dot_count` only |

None of these conflict with the issue's design. Two of them — (1) and (4) — change the
data model, so they have to be decided now rather than added later.

### 1.3 Two gotchas when porting the numbers

- **Every MATLAB size is a half-extent; every vstimd size is a full extent.** This is
  the sizing rule in `CLAUDE.md`, and the port crosses it twice:
  - `dotSize` is a radius — `edges = -radius:radius` with `dotMask = ... < radius`, so
    `dotSize = 1.5 deg` is a dot **3 deg across**. `dot_size_px = 2 * 1.5 * deg2pix`.
  - `R = 45/2` is a radius, so the figure circle is **45 deg across**. The original
    already carries the doubling in its own source (`[45]/2`), which is the tell that
    45 is the number the experimenter thinks in: `aperture.size_px = [45 * deg2pix,
    45 * deg2pix]` records what was meant rather than what was typed.

  The doubling happens once, in the Python client's Psychtoolbox port helper, and
  nowhere else. Nothing downstream of the wire ever sees a radius.
- **The direction convention is mirrored.** MATLAB adds `sin(dirAngle)` to a row
  index, which grows *downward*; vstimd is Y-up, CCW, 0° = right (`Transform2D`,
  `drift_angle_deg`). So `dirAngle = 3*pi/2` (270°) is **upward** on screen, which is
  vstimd's **90°**. The mapping is `direction_deg = (-dirAngle_deg) mod 360`; the two
  directions used, 0 and 3π/2, become **0° and 90°**.

### 1.4 The four `params` entries are conditions

`params{1..4}` differ only in which fields move coherently and which are drawn at all
(`figureDotIntensity = 0` / `backgroundDotIntensity = 0` mean "omit this field"):

| | background dir | figure dir | drawn |
|---|---|---|---|
| `params{1}` | 0, 270° | 0, 270° | both |
| `params{2}` | 0° | 0, 270° | figure only |
| `params{3}` | 0, 270° | 0° | background only ("hole") |
| `params{4}` | 0° | 0° | neither (blank) |

That is exactly the protocol-step shape `scene/conditions.rs` already has: two `Dots`
stimuli, each a member of the conditions in which it is drawn, with `direction_deg`
varying per condition. **No new mechanism is needed for the trial structure** — only
for the stimulus.

The laser blocks (`R0G1B2`, `laserStart`, `quickFilter`) are analog-output control for
optogenetics, not display. They are outside vstimd's remit and are ignored here.

---

## 2. Verdict on fit

Family A of #136 covers this stimulus, and the reproduction sharpens rather than
contradicts the design. The changes to the issue's plan are:

1. **Split the aperture from the field** (§3.1) — the one structural change.
2. **Add `aperture_clip`** (§3.2) — dot-centre by default.
3. **Add an optional second dot colour** (§3.3).
4. **Integrate positions incrementally**, so a direction change is a velocity change
   at the current position (§4).
5. **Keep `dot_count` as the stored value, over an explicit field**, which makes
   density expressible without storing a derived quantity (§3.1).

Prerequisite #120 (per-frame math against the nominal refresh rate) has **landed** —
`SceneRuntimeState::nominal_frame_rate_hz`, fed from `timing::nominal_hz()`. Nothing
blocks this.

---

## 3. Parameter surface

```rust
pub struct DotsParams {
    // ── field: where dots live and wrap ──
    pub field_size_px: [f32; 2],
    pub dot_count: u32,

    // ── aperture: where dots are visible ──
    pub aperture: Aperture,

    // ── appearance ──
    pub dot_size_px: f32,          // diameter
    pub dot_color: crate::Color,
    pub dot_color_alt: Option<crate::Color>,
    pub dot_shape: DotShape,       // Round (default) | Square

    // ── motion ──
    pub direction_deg: f32,        // CCW, 0 = right — matches drift_angle_deg
    pub speed_px_per_s: f32,
    pub coherence: f32,            // [0, 1]
    pub signal_rule: SignalRule,   // Same | Different
    pub noise_rule: NoiseRule,     // Position | Direction | Walk

    // ── lifetime & rebirth ──
    pub dot_lifetime_frames: u32,  // 0 = infinite
    pub reinsertion: Reinsertion,  // Wrap | Respawn

    // ── reproducibility ──
    pub seed: u64,
}

pub struct Aperture {
    pub shape: ApertureShape,      // Rect | Circle
    /// Full extents, never half-extents: `[width, height]` for `Rect`, and `[0]`
    /// is the **diameter** for `Circle` — per the sizing rule in `CLAUDE.md`.
    pub size_px: [f32; 2],
    pub offset_px: [f32; 2],       // from the stimulus transform's position
    pub invert: bool,              // draw *outside* the shape
    pub clip: ApertureClip,        // DotCenter (default) | Pixel
}
```

### 3.1 Field vs. aperture — the structural change

The issue's table has `field_shape` / `field_size_px` serving as both "the region the
dots occupy" and "the region you can see them in". For a Newsome-style RDK those are
the same region and the conflation is invisible. For a figure-ground RDK they are not:
the background field must fill the screen while being visible only *outside* a
45 deg circle, and the figure field must be visible only *inside* it. Merging them
makes the target stimulus inexpressible.

So: the **field** is always a rectangle, is where `dot_count` dots live and where
`reinsertion` acts, and is invisible. The **aperture** is a separate mask with a shape,
an offset and an `invert` flag. A classic RDK sets `aperture.shape = Circle` with
`size_px` equal to `field_size_px` and gets the familiar behaviour; a plain field with
no mask sets `shape = Rect` at field size.

This also settles open question 1 in the issue. `dot_count` stays the stored value —
it is what the config must record, and it is what a methods section quotes — but
because the field is now explicit and separate from the aperture, density is a
well-defined derived quantity, `dot_count / area(field_size_px)`. The Python client
gets `dots_for_density(density_per_deg2, field_size_deg, px_per_deg)`; the config
still records exactly what was shown.

For the target stimulus: field = the display extent plus a margin of
`speed_px_per_s * trial_duration_s` is *not* required, because wrapping supplies the
stream that MATLAB got from an 805 deg lattice. A field one screen wide with
`reinsertion = Wrap` is statistically identical to the MATLAB field, seamless (the
wrap boundary lies outside the aperture, so no edge cue reaches the observer), and
repeats exactly once per `field_width / speed` — 1 s at these numbers, which carries
no form because a uniform field is its own translate.

### 3.2 `aperture_clip` — dot-centre vs. per-pixel

MATLAB tests `figureMask(y, x)` at the dot's centre pixel and then blits the entire
dot. Dots therefore straddle the figure boundary uncut, and along the boundary dots
from both fields interleave into a slightly ragged edge.

Per-pixel clipping instead cuts dots in half at the boundary, which draws a crisp
circle — and a crisp circle is a **static form cue**, precisely what a motion-defined
figure is designed not to have. The MATLAB behaviour is the perceptually correct one
and it is what the figure-ground literature does. `ApertureClip::DotCenter` is the
default; `Pixel` is offered because a classic RDK in a hard circular aperture usually
wants it.

Implementation: `DotCenter` is a per-dot test in the CPU update pass (the instance is
simply not emitted); `Pixel` is a discard in the fragment shader against the aperture
in stimulus space. They are different mechanisms, not one parameter of one mechanism,
which is why this is an enum and not a bool on a shader.

### 3.3 `dot_color_alt`

`bwSameTrial = 1` gives each dot black or white with p = 0.5 on a grey background,
which removes the mean-luminance cue that a single-polarity field carries. When
`dot_color_alt` is set, each dot is assigned one of the two colours **at birth** from
the seeded RNG, and keeps it until it is reborn. The target stimulus runs with
`bwSameTrial = 0` (all white), so this is not on the critical path — but it is one
field in the struct and it is in the original, so it ships with the rest.

---

## 4. Motion, lifetime and reproducibility

**Positions are integrated incrementally, in a pre-sized `Vec` allocated at create
time.** The MATLAB is closed-form in `frameIndex`, and closed-form is the more obvious
way to satisfy #120 — but it cannot express `noFigureFrames`, where the figure field
switches direction mid-trial and must continue *from where the dots are*. Recomputing
from `t = 0` with a new direction teleports every dot. Incremental integration handles
a direction change as what it physically is, a change of velocity, and the same
applies to any animation that targets `direction_deg` or `speed_px_per_s`.

Incremental does not cost reproducibility. The requirement from #120 is that frame N
be a function of the config and N alone, not that it be computable in closed form:

- one `rand_chacha::ChaCha8Rng` per stimulus, seeded from `params.seed` at create
  time and advanced **exactly once per frame**, in a fixed order over the dot array;
- the per-frame step is `speed_px_per_s / nominal_frame_rate_hz`, resolved from
  `SceneRuntimeState::nominal_frame_rate_hz` (never the measured rate);
- `f32` positions throughout, rounded only at raster time.

The consequence to accept knowingly: replay must start from frame 0 and step forward.
There is no seek. That is already true of the grating's `phase_accum_cycles`.

**Lifetime is bookkept as groups by birth frame**, not as a per-dot countdown: dots
are partitioned into `dot_lifetime_frames` groups, one group is reborn each frame, and
group membership is fixed at create time. This staggers births uniformly for free —
the "single easiest thing to get wrong" in the issue is not a thing that can be got
wrong under this scheme — and it is the structure family B needs, where a group *is*
a lifetime step. `dot_lifetime_frames = 0` means infinite, which is the target
stimulus's setting and MWorks' convention; PsychoPy's `-1` is a client-side spelling
the Python wrapper translates.

`signal_rule` × `noise_rule` is the Scase, Braddick & Raymond (1996) taxonomy, kept as
two orthogonal enums rather than one six-valued enum (open question 4): they are
independent choices, papers report them independently, and PsychoPy exposes them as
two fields. The target stimulus is `signal = Same`, `noise = Direction` — roles fixed
for the dot's life, noise dots given a random but constant direction — at
`coherence = 1`, so no noise dot is actually drawn.

---

## 5. Rendering

- New `StimulusBody::Dots` arm — one pipeline, one cache, one push-constant layout —
  with `StimulusType::Dots` as the user-facing name.
- Instanced unit quads: one instance buffer of `[x, y]` (plus a colour index byte when
  `dot_color_alt` is set), rewritten in place each frame. Round dots come from a
  distance test in the fragment shader (`length(uv) < 0.5` → keep, else discard),
  which needs no texture; `DotShape::Square` skips the test and is what PTB's
  `dot_type = 0` gives.
- `ApertureClip::Pixel` adds a second discard against the aperture, with `invert`
  flipping the comparison. `ApertureClip::DotCenter` does the test on the CPU and
  emits fewer instances.
- The update is a CPU pass. Even at 20k dots it is negligible, and a CPU pass is
  deterministic across GPUs — which is the whole point. Revisit only for family B.
- **No allocation in the update.** Positions, directions, birth groups and colour
  indices are `Vec`s sized by `dot_count` at create time; the instance buffer is sized
  once and its used length varies. `set_dot_count` reallocates on the ZMQ thread,
  under the write lock, never on the render thread.

---

## 6. Work items

Following the module layout in `CLAUDE.md`:

1. `proto/vstimd/v1/stimuli/dots.proto` — `CreateDotsRequest`, `DotsParams`, `Aperture`,
   the `set_*` mutations, and the `DotShape` / `ApertureShape` / `ApertureClip` /
   `SignalRule` / `NoiseRule` / `Reinsertion` enums.
2. `stimulus_type.proto` — `STIMULUS_TYPE_DOTS = 13`. **Not** `STIMULUS_TYPE_PARTICLE = 9`,
   which means the old C++ `CStimulusPart` point cloud.
3. `scene/stimulus/dots/` — `dots_params.rs`, `dots_stimulus.rs` (config +
   `serde` delegating to `DotsConfig`, with the RNG and the position arrays as runtime
   state outside it, as `Grating` does with `phase_accum_cycles`), `dots_tess.rs`,
   `dots_pipeline.rs`. Mirrors `grating/`.
4. `scene/stimulus/stimulus_type.rs` — the `Dots` arm and `type_name`.
5. `ipc/convert/dots.rs` — `dots_params_from_proto` / `dots_params_to_proto` and
   nothing anywhere else.
6. `ipc/stimulus_commands.rs` arms + `ipc/dispatch.rs` routing.
7. `render/render_frame.rs` — the per-frame update, against `nominal_frame_rate_hz`.
8. `render/overlay_ui/panels/stimuli_panel.rs` — a parameter group.
9. Python client wrapper, `dots_for_density` helper, and the `set_*` mutations wired
   as animation targets.
10. Tests: a config round-trip (seed + params in, byte-identical dot positions out at
    frame N), a staggered-birth test (no frame reborn more than `dot_count /
    dot_lifetime_frames` dots), and an `Aperture { invert }` membership test.
11. `docs/` — the porting note from §1.3. The direction mirror and the two
    radius-to-diameter doublings will bite anyone moving a Psychtoolbox script across,
    and both fail silently: the stimulus renders, at half the intended size or
    mirrored about the horizontal.

The figure-ground reproduction itself is then a scene-config: two `Dots` stimuli, one
`invert`ed, four conditions per §1.4, checked against the MATLAB frame by frame.

---

## 7. Answers to the issue's open questions

1. **`dot_count` or `dot_density`?** `dot_count`, over an explicit `field_size_px`
   that is now separate from the aperture, so density is derivable. Client-side helper
   for the conversion. (§3.1)
2. **Degrees of visual angle?** Ship px; convert client-side. The rig geometry the
   conversion needs is not in the scene, and putting it there is a bigger change than
   this issue. The reproduction needs a documented conversion, not a new unit system.
3. **Wrap or respawn?** Both, as `Reinsertion`. The choice only becomes free once the
   field is separate from the aperture; with that split, `Wrap` is the right default,
   because the wrap boundary is no longer a visible boundary. (§3.1)
4. **One enum or two?** Two orthogonal enums, `signal_rule` and `noise_rule`. (§4)
5. **Does B want its own stimulus type?** Deferred, but §8 keeps both doors open.

---

## 8. What family B needs from this design

Two decisions here exist for B and are worth keeping even though B is not being built:

- **Lifetime as groups by birth frame** (§4). B's dots *are* groups — one per lifetime
  step, the oldest discarded each frame — so a per-dot countdown would have to be
  rewritten.
- **The aperture is a separate thing from the field** (§3.1). B's "aperture" is an
  off-screen ID buffer rendered from the 3-D pass: a per-pixel test that says which
  triangle, if any, a dart hit. That is a third `ApertureClip`-shaped source, and it
  slots in beside `DotCenter` and `Pixel` rather than displacing them.

Everything else in B — the ID buffer, barycentric binding to triangles, the background
camouflage field — is new work, and the parameters it adds (`dot_lifetime_ms`, separate
foreground/background colours, a strobe option) are additive.

---

## 9. Status

Family A has landed. What exists:

- `proto/vstimd/v1/stimuli/dots.proto`, `STIMULUS_TYPE_DOTS = 13`, the `DotsParams`
  query arm, and ten `Set*` mutations on the service oneof (140–149).
- `server/src/scene/stimulus/dots/` — `dots_params.rs`, `dots_stimulus.rs`,
  `dots_pipeline.rs`, `dots_rng.rs`, and `dots_tess.rs` recording that there is
  nothing to tessellate.
- `ipc/convert/dots.rs`, `ipc/dots_commands.rs`, the `dispatch.rs` arms.
- `shaders/dots.wgsl` plus `DotsInstanceCache` — one persistently mapped instance
  buffer per field **per frame-in-flight slot**, since the renderer waits only on
  the fence of the slot it reuses.
- Python: `vstimd.stimuli.dots_client` / `dots_models`, the unit helpers, and
  `examples/figure_ground_rdk.py`.
- Tests: 34 unit, 8 integration (`server/tests/dots.rs`, including `figure_ground`),
  13 Python unit, 8 e2e cases (`DOTS-01`…`DOTS-08`).
- Docs: `docs/concepts/random-dots.md`.

Two decisions changed under test, both because a test made a defect concrete:

- **`speed_px_per_s` and `coherence` carry field presence.** They were on the
  zero-means-default convention, which made `coherence = 0` — a field of pure noise,
  a standard control — inexpressible, and gave a `DotsParams()` a stationary field
  over the wire while the Rust default moved. Zero is meaningful for both, so the
  fallback is on absence.
- **`Aperture::default()` is unbounded.** It was a 400 × 400 rectangle over an
  800 × 600 default field, so `DotsParams::default()` silently hid two thirds of its
  own dots.

Not done, and deliberately:

- **The overlay has a list entry, not a parameter group.** `stimuli_panel.rs` reports
  a field as "`n` dots in `w`×`h`"; editing its parameters live is not wired.
- **No animation targets.** The work item asked for the `set_*` mutations wired as
  animation targets, but `AnimationKind` has no generic parameter-animation arm to
  wire them to — that is #119, and inventing one here would prejudge it. The
  stimulus is designed for it: a direction change is already applied as a change of
  velocity, so an animation driving `direction_deg` will do the right thing on the
  day the mechanism exists.
- **No web client support.** `client/web` does not know the type.
- **No saved scene-config for the reproduction.** `figure_ground` in
  `server/tests/dots.rs` and `examples/figure_ground_rdk.py` build it in code;
  neither has been round-tripped through a `.config.json` on a rig.

One thing that could not be verified in this environment: **the rendered output was
not seen.** Screen capture cannot read the Vulkan swapchain here — a known-good red
rectangle on a blue background captures black too — and no validation layer is
installed. What is verified is that the pipeline builds and runs without fault over
hundreds of frames with two fields present, that the per-frame update executes (the
tessellation phase goes from ~12 µs to ~28 µs when the fields are created), and that
the CPU→shader contract is pinned by tests over `build_dots_push_constants`. The
shader itself wants an eye on a real display: `make test-e2e-interactive` and the
`DOTS-*` cases are written for exactly that.
