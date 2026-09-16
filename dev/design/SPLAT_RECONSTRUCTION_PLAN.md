# Reconstructing splat scenes from image assets

**Status:** phases 0 and 1 implemented (2026-09-15, branch `feat/gaussian-splat-corridor`):
the `vsplat` crate and the `vstimd-reconstruct` CLI (`reconstruct/`). Run end to end on one
real capture (§12, "First real run"); not yet on a phone capture of a corridor. Server-side jobs, the protocol and the asset types come later, once
the asset store exists (phases 2–4). Where the implementation departs from this document, §12
says so.
**Depends on:** the `GaussianSplat3D` stimulus (`dev/design/GAUSSIAN_SPLAT_PLAN.md`, #148) now;
the asset store (`dev/design/ASSET_STORE_PLAN.md`, phases 1–2) later.
**Goal:** a user uploads photos of a real corridor into a project, asks vstimd to reconstruct
it, and gets back a splat asset that a `GaussianSplat3D` stimulus and a finite-track
`LinearNav3D` can use unchanged — already in centimetres, Y-up, running down −Z from the origin.

---

## 1. What this is, and what it is not

A **job system and an alignment step**, wrapped around existing reconstruction tools.
vstimd does not implement structure-from-motion or Gaussian training. Both are large,
fast-moving research codebases, and competing with them is not this project's job.

What vstimd adds, and no off-the-shelf tool does:

1. **Jobs that live in a project.** Inputs are image assets, the output is a splat asset,
   and the intermediate data and report sit beside them, so a study stays one folder (asset
   store §2).
2. **Alignment into vstimd's world.** A trained scene has arbitrary scale, orientation and
   origin. A corridor is only useful when its floor is at `y = 0`, it runs down −Z from the
   origin, and one unit is a centimetre, so the track length and camera height mean what
   they say. §6 is the part of this design that is specific to vstimd.
3. **Never costing a frame.** Training saturates a GPU for minutes to hours. On a rig that is
   also presenting stimuli, that is a timing failure. §4 is the policy that prevents it.

Not in scope: pose-free or learned reconstruction (MASt3R, VGGT and similar — licences are
mostly non-commercial), NeRFs, meshing, view-dependent colour beyond what the trainer
exports, and editing splats by hand.

## 2. Constraints

- **The render thread must never block, and the rig's GPU belongs to the stimulus.**
  Reconstruction never runs in the vstimd process. Where it may run on the same GPU is
  a policy decision (§4).
- **The Jetson Orin Nano is the weakest 3-D target:** 8 GB of memory shared by CPU and GPU.
  Training there is slow at best and may not fit at all. The design must work when
  compute happens on another machine.
- **The client is on another machine** (asset store §1). Inputs arrive as uploaded assets or
  over the Samba share, never as client-side paths.
- **The filesystem is the source of truth** (asset store §4). Job state is files in the
  project, not server memory, so a job survives a restart and can be inspected over the share.
- **Licences.** vstimd is AGPL-3.0. The Inria reference 3DGS code is non-commercial and is
  excluded. The tools below are BSD or Apache-2.0, and are called as subprocesses.

## 3. The pipeline

```
images/<capture>/*.jpg                                         (asset type images/, input)
  │
  ├─ 1. ingest      validate, EXIF-orient, downscale to max_image_px, exposure check
  ├─ 2. sfm         COLMAP: feature_extractor → sequential_matcher → global_mapper
  ├─ 3. undistort   COLMAP: image_undistorter  (pinhole cameras for the trainer)
  ├─ 4. align       vstimd: up, forward, origin, scale, floor (§6) → a similarity transform
  ├─ 5. train       Brush: COLMAP dataset → .ply
  ├─ 6. finish      vstimd: bake the transform, crop to the corridor box, drop floaters,
  │                 cap the splat count, write the output format
  ▼
splats/<name>.ply                                              (new asset type splats/, output)
reconstructions/<name>/                                        (new asset type, work dir + report)
```

**Tools.**

| Stage | Tool | Why |
|---|---|---|
| SfM | [COLMAP](https://github.com/colmap/colmap) (BSD), global mapper | The standard. Since GLOMAP was merged in, the global mapper is part of COLMAP itself and much faster than incremental mapping on sequential captures. Ships CUDA and CPU builds, including aarch64 |
| Train | [Brush](https://github.com/ArthurBrussee/brush) (Apache-2.0) | Rust, on Burn/wgpu: trains on AMD, NVIDIA and Intel GPUs with no CUDA stack, reads COLMAP datasets, exports PLY. The only trainer that matches vstimd's toolchain and Vulkan-first hardware story |
| Train (alternative) | gsplat (Apache-2.0, CUDA, Python) | Better studied, but CUDA-only and a Python environment on the rig. Kept as a second trainer backend (§5.3), not the default |

**Stage 4 before stage 5**, even though training does not need the alignment: aligning
first lets the trainer's own scene bounds and densification work in the corridor's frame,
and it reports a bad capture (no clear up direction, a trajectory with no dominant
direction) in minutes rather than after an hour of training.

## 4. Where compute runs

**v1 is the CLI alone:** the user runs `vstimd-reconstruct` on whatever machine has the GPU,
usually a workstation or a desktop rig between sessions, and copies or loads the result. The
table below is where this goes once vstimd runs jobs itself. Three deployments, one worker binary:

| Deployment | Who runs the worker | Default policy |
|---|---|---|
| **Desktop rig with a discrete GPU** | vstimd spawns it | `idle` — only while no experiment is running |
| **Jetson / Pi rig** | vstimd refuses (`NOT_SUPPORTED`, naming the policy) | `never` |
| **Offload** | a user runs `vstimd-reconstruct run` on a workstation against the project folder on the rig's Samba share | the rig does nothing but serve files |

Offload needs **no protocol at all**, because job state is files (§5.2): the workstation
worker reads and writes the same job directory, and the rig's `ListReconstructions` sees its
progress. This stays the recommended path for every Jetson rig after the server can spawn jobs.

**The `idle` policy** (rig-config `[reconstruction] run = "idle" | "always" | "never"`):

- A job starts only when no animation is `Running` or `Armed` and no stimulus is enabled in a
  scene with 3-D content. Otherwise it waits in `Queued`.
- If an animation is armed while a job runs, the worker's process group gets `SIGSTOP`, and
  `SIGCONT` once the scene is idle again. A `reconstruction.paused` warning event is emitted.
  **A stopped process still holds its GPU memory.** On a 4 GB card that may be enough to
  break 3-D setup, so a pause is logged with the VRAM the worker holds, and `idle` is
  documented as "no drops in practice", not as a guarantee. Rigs that need the guarantee use
  offload.
- Workers run at `nice 19`, `SCHED_IDLE`, and off the render thread's pinned cores
  (`process/` already knows the pinning).

## 5. The worker, and later the server

§5.3 (the worker) is what gets built now. §5.1, §5.2's server-watching parts, §5.4 and §5.5
wait for the asset store; they are specified here so the CLI's job directory and report format
do not have to change when the server takes them over.

### 5.1 Asset types

Two new types in the asset store's fixed set:

| Type | Contents | Over the wire |
|---|---|---|
| `splats/` | `.ply`, `.splat`; later `.spz` | an **input**: uploadable, and what `GaussianSplat3D` references. Its `path` field becomes an `AssetRef` — the change #148 already plans |
| `reconstructions/` | one directory per job (§5.2) | an **output**, like `logs/`: listable, downloadable, deletable, never uploadable |

Input images stay in `images/`, in a subfolder per capture (`images/corridor_a/0001.jpg`).
No `captures/` type: a capture is images, and fewer types is the asset store's rule.
Video input is an open question (§11).

### 5.2 A job is a directory

```
projects/<project>/reconstructions/<name>/
  job.toml          the request, as submitted: inputs, parameters, output ref. Never rewritten
  state.json        {stage, status, progress, started, updated, worker_host, error}
                    written atomically (temp + rename) by the worker only
  log.txt           the tools' stdout/stderr, appended
  report.json       written by `finish`: splat count, alignment, corridor extent, timings,
                    registered/total images, reprojection error
  preview.png       a render of the result from the capture start pose
  work/             colmap db, sparse model, undistorted images, trainer checkpoints
    .done-<stage>   stamp per finished stage — the resume points
```

- **Resume** — the worker skips any stage whose stamp exists, so a job killed by a reboot,
  or cancelled and restarted, continues from its last finished stage.
- **Ownership** — whoever holds `work/.lock` (an advisory `flock`, with host and pid inside)
  is the worker. A second worker, or vstimd trying to spawn one, backs off. That is what makes
  offload safe while the rig is also watching the directory.
- **vstimd's own state** is only a list of job directories and the child processes it
  spawned. `state.json` is read on request and polled at 1 Hz for jobs it is watching,
  which drives the events. Nothing about a job lives only in memory.
- **Cleanup** — `keep_work = false` (the default) deletes `work/` after a successful
  `finish`, leaving the request, the log, the report and the preview. `work/` is the large part.

### 5.3 The worker: `vstimd-reconstruct`

A new workspace binary crate `reconstruct/`, separate from the server. **This is the v1
deliverable, used by hand:**

```
vstimd-reconstruct run <images-dir> --out <file.ply> --path-length-cm 500 [--work <dir>] [options]
                                        create a job directory (default: <out>.reconstruction/)
                                        and run it; run again with the same --work to resume
vstimd-reconstruct resume <job-dir>     resume a job from its job.toml
vstimd-reconstruct check                report which tools and GPUs are usable, as JSON
```

The options are the fields of `ReconstructionParams` (§5.4) spelled as flags
(`--max-image-px`, `--order unordered`, `--train-steps`, `--max-splats`, `--capture-height-cm`,
`--crop-width-cm`, …), and `job.toml` stores exactly those values. When the server takes over,
`StartReconstructionRequest` writes the same file. Paths are plain filesystem paths in v1; with
the asset store they become refs, which resolve to the same directories.

The tool prints stage progress to the terminal and writes `state.json` as it goes, so the same
job can be watched from another shell (`jq . state.json`).

- **Stages are a table** of `{name, run: fn(&Job) -> Result<()>}`. The COLMAP and trainer
  stages build a command line and stream the tool's output into `log.txt`, parsing progress
  where the tool prints it (COLMAP's image counters, the trainer's step counter). A stage that
  cannot parse progress reports only "running".
- **Trainer backends** sit behind one trait, `Trainer { fn train(dataset, params, out) }`,
  with `Brush` as the default and `Gsplat` behind a config switch. Adding one is a command
  line and a progress parser.
- **Tools are found** via the rig-config (`colmap = "/usr/bin/colmap"`, `brush = …`), else
  `PATH`. None are bundled: the `.deb` gets `Suggests: colmap`, and `check` names what is
  missing. `QueryServerInfo` reports reconstruction as `available` / `unavailable(reason)`, so
  clients can grey it out.
- **Subprocesses, not linking.** Each training run is a process that frees all of its GPU
  memory on exit, a trainer crash cannot take anything else down, and there is no wgpu or
  Burn version to keep in lockstep with vstimd's `ash`. Linking Brush as a library is a later
  option, not a v1 one.
- **Shared splat code** — `align` and `finish` read and write splat files, which
  `server/src/splat/` already does. That module moves to a small `vsplat` crate used by both
  the server and the worker, in the same way `vtl` and `vinput` are shared today.

### 5.4 Protocol

New `proto/vstimd/v1/reconstruction.proto`. All commands are system-target, handled in
`ipc/reconstruction_commands.rs`, and none holds the scene lock beyond validation (§5.5).

```proto
message ReconstructionParams {
  // Input: an images/ prefix, relative or absolute. Every PNG/JPEG under it.
  string images_ref = 1;
  // Output asset; "" → splats/<name>.ply in the job's project.
  string output_ref = 2;

  uint32 max_image_px = 3;       // long edge after ingest (0 → 1600)
  CaptureOrder order = 4;        // SEQUENTIAL (walked; default) | UNORDERED (exhaustive matching)
  uint32 train_steps = 5;        // (0 → the trainer's default)
  uint32 max_splats = 6;         // cap after cropping (0 → 1 000 000)
  uint32 sh_degree = 7;          // 0–3; what finish keeps (0 → 0, see GAUSSIAN_SPLAT_PLAN §2)

  Alignment alignment = 8;
  Crop crop = 9;
  bool keep_work = 10;
}

// §6. Every field has a default derived from the capture; set one to override it.
message Alignment {
  // Scale: the true distance between the first and last camera positions, as walked.
  // Required: a reconstruction has no scale of its own, and a corridor without one is
  // not a stimulus. (0 → INVALID_ARGUMENT)
  float capture_path_length_cm = 1;
  float capture_height_cm = 2;   // camera height above the floor while capturing (0 → from the floor fit)
  bool keep_colmap_up = 3;       // skip the up-direction estimate
}

message Crop {
  // A box in the aligned frame: x ∈ ±width/2, y ∈ [below_floor, height], z ∈ [−length − margin, margin].
  // 0 for any extent → from the capture path and the fitted floor, plus margin.
  float width_cm = 1;
  float height_cm = 2;
  float length_cm = 3;
  float margin_cm = 4;           // (0 → 50)
}

message StartReconstructionRequest {
  string name = 1;               // [A-Za-z0-9._-]{1,64}; the job directory
  string project = 2;            // "" → active project
  ReconstructionParams params = 3;
  bool restart = 4;              // an existing job: false → RECONSTRUCTION_EXISTS; true → resume it
}

message QueryReconstructionRequest { string project = 1; string name = 2; }
message ReconstructionStatus {
  string name = 1;
  ReconstructionStage stage = 2;   // INGEST … FINISH
  ReconstructionState state = 3;   // QUEUED | RUNNING | PAUSED | SUCCEEDED | FAILED | CANCELLED
  float stage_progress = 4;        // [0, 1], or −1 when the tool reports none
  string worker_host = 5;          // this rig, or the offload workstation
  string error = 6;
  string output_ref = 7;           // set on SUCCEEDED
  string report_json = 8;          // report.json verbatim, once written
  int64 updated_unix_ms = 9;
}

message ListReconstructionsRequest { string project = 1; }
message CancelReconstructionRequest { string project = 1; string name = 2; }   // SIGTERM; stamps survive
message DeleteReconstructionRequest { string project = 1; string name = 2; }   // refuses while running
```

- **Events:** `reconstruction.state` on every state change and `reconstruction.progress` at
  most once a second, on the existing event stream. Both are for UIs; nothing times against them.
- **Errors:** `RECONSTRUCTION_EXISTS`, `RECONSTRUCTION_NOT_FOUND`,
  `RECONSTRUCTION_UNAVAILABLE` (tools missing or `run = "never"`, with the reason),
  `RECONSTRUCTION_BUSY` (one job at a time per host in v1), plus the asset store's
  `ASSET_*` codes for the refs.

### 5.5 Threads

`StartReconstruction` runs on the ZMQ thread. It validates the refs, writes `job.toml`, and
hands the job to a `ReconstructionSupervisor` thread, then returns. The supervisor owns the
queue, the idle policy (it reads a `scene_is_idle` flag the render thread publishes through an
atomic, never the scene lock) and the child processes. The render thread never learns that
reconstruction exists.

## 6. Alignment — the part that is vstimd's

SfM output is a similarity transform away from anything useful. `align` estimates that
transform from the capture itself and writes it to `work/alignment.json`. `finish` then bakes
it into the splats.

**The capture protocol this assumes** (documented in `docs/stimuli/splat-scenes.md`):
walk the corridor once, from start to end, holding the camera at the height the virtual
camera will use, looking forward, with fixed exposure and white balance. Measure how far you
walked.

1. **Up (+Y).** Take the median of the registered cameras' *up* vectors. A handheld forward
   walk is overwhelmingly upright. Cross-check it against COLMAP's `model_orientation_aligner`
   (Manhattan-world estimate); if the two disagree by more than 10°, fail the stage and say so
   rather than guess. `keep_colmap_up` skips the check.
2. **Forward (−Z).** The first principal component of the camera centres, projected onto the
   plane perpendicular to up, signed so that it points from the first image to the last. A
   capture whose second component is more than 30 % of the first did not walk a corridor;
   warn in the report.
3. **Floor (y = 0).** RANSAC plane fit on the sparse points whose normal is within 15° of up,
   restricted to points below the cameras. This gives the capture height. If
   `capture_height_cm` was given, the scale is cross-checked against it (step 5).
4. **Origin.** The first camera centre, projected onto the floor.
5. **Scale.** `capture_path_length_cm` divided by the path length measured along forward,
   first to last camera. Report the relative disagreement with the floor-derived capture
   height when both are known; above 10 % is a warning.
6. **Crop box** in the aligned frame. Width and height default to the extent of the sparse
   points between the 5th and 95th percentiles, length to the path length, and each gets the
   margin. Everything outside is discarded in `finish`, which also removes most floaters.

`finish` applies `x' = s·R·x + t` to positions, `Σ' = s²·R·Σ·Rᵀ` to covariances (the
rotation composes with each splat's quaternion, and log-scales shift by `ln s`), and rotates
the SH coefficients when `sh_degree > 0` (degree-0 colour is rotation-invariant). It then drops
splats by opacity × volume until `max_splats` is met and writes the output.

**The result is used with an identity transform:**

```python
conn.stimuli.gaussian_splat.create("splats/corridor_a.ply")
conn.system.set_camera(Camera3D(position_cm=Vec3(0, report.capture_height_cm, 0)))
conn.animations.create_linear_nav_3d(20.0, track_length_cm=report.path_length_cm, fade_frames=30)
```

**The limitation to document loudly:** a splat scene is only sharp near the capture
trajectory (the room sample in #148 blurs 90° off the capture views). A corridor captured at
human eye height and viewed from a mouse's eye height will look wrong. The capture height is
therefore part of the report, and the client warns when the camera is placed more than
`0.5 × capture_height_cm` away from it.

## 7. Clients

- **Python** — `conn.reconstruction.start(name, images_ref, capture_path_length_cm=…, …)`
  returns a handle. `.status()`, `.wait(on_progress=…)`, `.cancel()`, `.report()`, and
  `conn.reconstruction.list()`. `wait` follows the event stream, falling back to polling
  `status`.
- **CLI** — `vstimd-client reconstruct start <name> images/corridor_a --path-length 500`, plus
  `status`, `list`, `cancel` and `rm`. Combined with `asset push -r` from the asset store, the
  end-to-end command is two lines.
- **Web** — a Reconstructions panel inside the project view: pick an image folder, enter the
  walked distance, start, watch stage progress and the log tail, and see `preview.png` when
  done, with "create stimulus" as one button.
- **Overlay** — one status line while a job runs or is paused on this host ("reconstructing
  corridor_a: train 43 %"), because an operator at the rig must be able to see that the GPU is
  shared.

## 8. Testing

- **Pure functions** (`reconstruct/` unit tests): the alignment estimators on synthetic camera
  sets (known rotation, scale and noise; degenerate captures fail with the right message),
  the transform bake (a rotated and scaled cloud round-trips its covariances), and crop and cap.
- **Stages without tools:** fixture COLMAP sparse models checked into `reconstruct/tests/`
  (a few kilobytes of text format) drive `align`. A fake trainer script stands in for Brush.
- **Job directory semantics:** resume from each stamp, lock contention between two workers,
  cancel, and a restart mid-stage.
- **Server:** `handle_request` integration tests for the commands and the idle policy, with a
  fake worker binary that walks `state.json` through its stages.
- **End to end:** an `#[ignore]`d test and a Make target that reconstruct a small public
  capture (tens of images) on a machine with COLMAP and Brush installed, then check the report's
  scale and floor against the dataset's known geometry, and render the preview through
  `CaptureFrame`.

## 9. Phasing

| # | Phase | Depends on | Useful on its own |
|---|---|---|---|
| 0 | Move `server/src/splat` to a `vsplat` crate; add `.ply` writing | — | — |
| 1 | **Now.** `vstimd-reconstruct`: every stage, job directory, resume, `check`, report, docs for the capture protocol. A CLI on a workstation or desktop rig, pointed at a folder of images | 0 | **Yes** — reconstruct a corridor by hand and load the result through #148's path field |
| 2 | **Later, with the asset store.** Asset types `splats/` and `reconstructions/`; `GaussianSplat3D.path` → `AssetRef`; the CLI accepts a project folder | asset store 1–2 | Running the CLI against a project on the Samba share needs no further code |
| 3 | `reconstruction.proto`, supervisor, spawning, idle policy, events, errors | 1, 2 | Desktop rigs reconstruct on request |
| 4 | Python, CLI, web panel, overlay line, docs including the capture protocol | 3 | — |
| 5 | Extras, each on demand: video input, gsplat backend, `.spz`, marker-based scale | 4 | — |

**Phase 1 first**: it answers the questions this document cannot — whether Brush's quality is
good enough on corridors, how long a capture takes to train on the GTX 1650, and whether the
alignment heuristics hold on real walked captures — before any wire format is committed.

## 10. Risks

- **Brush quality versus CUDA trainers on indoor, texture-poor corridors.** Plain walls are
  hard for SfM and for training. Mitigations: the gsplat backend, and a capture protocol
  that recommends adding texture (posters, tape) if the walls are blank — which also helps
  the animal.
- **Frame timing on shared-GPU rigs.** §4's `idle` policy reduces the risk but does not remove
  it. Offload is the answer for any rig where a single dropped frame matters.
- **Numbers this document does not have:** SfM and training time for a typical capture,
  VRAM needed at 1600 px, and whether Brush runs on the Orin's Vulkan driver at all. Phase 1
  measures them before phase 3 sets the defaults.
- **Tool CLIs change.** COLMAP renamed its mappers when GLOMAP was merged in. Pin tested
  versions in `check`, and warn on others.

## 11. Open questions

1. **Video input** — extract frames in the worker (needs ffmpeg on the host, and picks frames
   by sharpness), or client-side before upload (keeps the rig lean)? Recommend **the worker**:
   uploading one video beats uploading 400 JPEGs over ZMQ, and sharpness-based selection
   belongs with the pipeline that cares about it.
2. **More than one job at a time per host** — no in v1. A second one queues.
3. **Should the capture's scale come from markers** (AprilTags of known size) instead of a
   walked distance? More accurate, but more setup at capture time. Recommend the walked
   distance for v1 and markers as a phase-5 alternative.
4. **Is a preview render worth a GPU dependency in the worker?** The worker could instead ask
   vstimd to render the preview with `CaptureFrame` when the job finishes on the rig. Simpler,
   but it does nothing for offload. Recommend: vstimd renders it on first query when
   `preview.png` is missing.
5. **Where does the report's recommended camera height go** — only into the report, or also
   into a generated scene-config (`scene-configs/<name>.config.json` with the splat, camera and
   track already set up)? The generated scene-config is one step from "images in, experiment
   out", and is probably worth it in phase 4.

## 12. As implemented (phase 1)

Departures from the sections above, each deliberate:

- **Job layout** (§5.2): the stamps (`.done-<stage>`) and the lock (`.lock`) sit in the job
  root, not `work/`, and so do `alignment.json`, `trained.ply` and `images.json`. `work/` is
  then only disposable data, and `resume <job> --from finish` can recrop or recap after
  `work/` is deleted. No `preview.png` yet (§11 question 4).
- **Ingest** (§3 stage 1) only collects: every JPEG/PNG directly in the folder, in natural
  file-name order, linked (else copied) into `work/images/00001.jpg…`. No EXIF rotation, no
  downscaling and no exposure check — COLMAP's `image_undistorter --max_image_size` and
  Brush's `--max-resolution` do the downscaling. Feature extraction uses
  `--ImageReader.single_camera 1` and no version-specific flags.
- **Up** (§6 step 1) is levelled by the floor, not taken from the cameras. The mean camera up
  is only the first guess (refused when the cameras spread by more than 20°): a camera held
  pitched down tilts every up vector the same way, which no spread check sees — the first real
  run was 19° off this way. The floor is searched along the current up, a plane is fitted to
  it, its normal becomes the next up, and this repeats until up moves less than 0.05°. A floor
  more than 40° from the cameras' up is not trusted. `up_correction_deg` in the report is
  roughly how far the camera was pitched. There is no `model_orientation_aligner`
  cross-check and no `keep_colmap_up`.
- **Floor** (§6 step 3): not a free RANSAC plane. A free plane tilts to take floor and the bottom rows of
  wall texture together, and with no floor texture it takes a wall row for the floor. Up is
  already known, so the floor is the lowest dense band of point heights (at least half as
  dense as the densest band) whose points spread across the corridor (≥ 20 % in the middle
  half of its width); the plane through it is refitted on its own residuals. What tilt is left
  after levelling is reported, and warned about above 5°. No floor → `--capture-height-cm` places it, else the stage fails.
- **Scale** is the walked distance over the first-to-last camera distance along forward.
- **SH** (§6 bake): degrees 0 and 1 only — degree 1 rotates as a vector, degrees 2–3 would need
  Wigner matrices. `sh_degree` > 1 is refused at job creation, and Brush is told
  `--sh-degree` so it does not train bands that would be thrown away.
- **Brush progress**: its CLI shows a progress bar only on a terminal, so it is run with
  `--export-every total/10` and progress is read from the newest `export_<step>.ply`; older
  exports are deleted as newer ones appear.
- **Tools** come from `--colmap`/`--brush`, then `VSTIMD_COLMAP`/`VSTIMD_BRUSH`, then `PATH`
  (`colmap`; `brush_app` or `brush`). `--mapper global` falls back to `mapper` when
  `colmap global_mapper -h` fails.
- **Interruption**: Ctrl-C leaves `state.json` at `running`; a running state whose `.lock`
  nobody holds was interrupted. `run` with the same arguments, or `resume`, continues it.

### First real run

TUM RGB-D `fr3/structure_texture_far` (handheld, 640×480, every 8th frame → 118 images,
motion-capture ground truth), on the development desktop: COLMAP 4.3 in Docker on the CPU
(`colmap/colmap`, no NVIDIA runtime, `--FeatureExtraction.use_gpu 0`), Brush 0.3.0 natively
on the GTX 1650, 7 000 steps.

| Stage | Time |
|---|---|
| sfm (118/118 registered, 0.59 px) | 53 s |
| undistort | 2.4 s |
| train | 2:54 min → 37.7 k splats, 34.8 k after cropping |

Aligned cameras against ground truth: height above the floor off by 1.2 cm (median), pitch
21.4° vs 21.9°, horizontal path 0.7 cm RMS after fitting only heading and offset, path
460.5 vs 459.6 cm. The scene loads upright with an identity transform. The capture is not a
corridor walk (the camera looks sideways at a structure), and the report says so.

Still to do in phase 1: a phone capture of a real corridor, VRAM at 1600 px and 30 000 steps,
and packaging. The capture guide is `docs/stimuli/splat-scenes.md`.
