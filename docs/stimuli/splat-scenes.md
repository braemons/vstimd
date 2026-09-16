# Splat scenes from photos

A `GaussianSplat3D` stimulus draws a photorealistic 3-D scene reconstructed from
photos of a real place — typically a corridor the animal walks through in VR.
`vstimd-scene-from-capture` turns a folder of photos into such a scene, already placed
where vstimd expects it: in centimetres, with the floor at `y = 0` and the
corridor running down −Z from where the first photo was taken.

!!! warning "Early"
    `vstimd-scene-from-capture` is new and has been run on one real capture so far. The
    alignment assumes a single straight walk; corners are not supported yet.

The pipeline wraps two external tools — [COLMAP](https://colmap.github.io/)
recovers where each photo was taken, [Brush](https://github.com/ArthurBrussee/brush)
trains the splats — and adds the step neither does: scaling, levelling and
placing the result for vstimd.

## 1. Capture

How you take the photos decides the result far more than how many you take.
In order of importance:

1. **Hold the camera at the height the virtual camera will use.** A splat scene
   is only sharp near the places the photos were taken from. A corridor
   photographed at eye height looks smeared when viewed from a few centimetres
   above the floor, so for a mouse's view put the phone down there — on a cart, a
   board pulled along the floor, or a low gimbal.
2. **A still scene with texture.**
    - No people walking through, no doors moving, no flickering lights.
    - Plain white walls and glossy floors defeat the pose recovery: add posters,
      patterned tape or printed noise. It helps the animal too.
    - Keep a textured floor in view in many photos. The floor is what the scene is
      levelled by and what the camera height is measured from.
3. **Move, don't pivot.** Walk slowly and take a photo every 20–30 cm, so that
   neighbouring photos overlap by roughly 70 %. A 5 m corridor needs about
   150–250 photos. Turning on the spot without moving gives the reconstruction
   nothing to triangulate.
4. **Lock the camera settings.**
    - Lock focus and exposure (long-press on the screen on iPhone, *Pro* mode on
      Android).
    - Turn off HDR and night mode; fix the white balance if the phone allows it.
    - Do not zoom and do not switch lenses during a capture. Use one lens,
      preferably the main (1×) one: the reconstruction treats the whole capture as
      one camera.
5. **No blur.** Bright light and a short exposure. Blurry photos give a blurry
   scene.
6. **One straight walk, start to end.** Look roughly down the corridor; some
   panning left and right is fine, and so is tilting the phone down. Then
   **measure the distance from where you took the first photo to where you took
   the last** — that measurement is the scene's scale. Note the camera height as
   well; it is used as a cross-check.
7. **Files.**
    - The tool reads JPEG and PNG. iPhones save HEIC by default: set
      *Settings → Camera → Formats → Most Compatible*, or export as JPEG.
    - Copy the originals by cable, AirDrop or a file share. Messenger apps and
      some cloud uploads strip the EXIF data, which holds the focal length.
    - Keep the phone's own file numbering: photos are used in file-name order.

**Video instead of photos** is often easier to capture well, and the tool takes
a video directly — pass the file where you would pass the folder. Film at 4K,
30 fps, walking slowly, with exposure locked and electronic stabilisation off.

```bash
vstimd-scene-from-capture run walk.mp4 --out corridor.ply \
    --capture-path-length-cm 500
```

`--fps` (default 3) sets how many frames a second are kept; choose it so frames
land 20–30 cm apart at your walking speed. Frames are **not** taken blindly at
that rate: `--frame-oversample` (default 3) frames are examined for each one
kept, and the sharpest of each group wins. A handheld walk blurs unevenly, and a
fixed rate lands on a blurred frame as often as a sharp one — this is rule 5
applied for you. `--frame-oversample 1` turns the pass off and keeps every frame.

Two things to know about video. It needs **ffmpeg 5.1 or newer** on `PATH`
(Debian/Ubuntu: `apt install ffmpeg`), or `--ffmpeg`/`$VSTIMD_FFMPEG`; on a
machine without one, `reconstruct/scripts/ffmpeg-docker` runs it from a
container, like `colmap-docker` does for COLMAP:

```bash
vstimd-scene-from-capture run walk.mp4 --out corridor.ply \
    --capture-path-length-cm 500 \
    --ffmpeg reconstruct/scripts/ffmpeg-docker
```

And video frames carry **no EXIF focal length**, so COLMAP estimates it from the
frame size; the same walk shot as stills usually reconstructs a little better
for that reason alone.

**Test before the real capture.** Capture one or two metres, run it with
`--train-steps 7000` (a few minutes, see below) and read the report before
capturing the whole corridor.

## 2. Install the tools

Build the tool (release builds only):

```bash
cargo build --release -p vstimd-scene-from-capture
# → target/release/vstimd-scene-from-capture
```

**Brush:** download `brush-app-x86_64-unknown-linux-gnu.tar.xz` from the
[Brush releases](https://github.com/ArthurBrussee/brush/releases) (tested with
0.3.0) and unpack it. It trains on the GPU through Vulkan; no CUDA needed.

**COLMAP**, either installed (`apt install colmap`, or a build with CUDA), or
from Docker with the wrapper script in this repository:

```bash
docker pull colmap/colmap:latest
# then pass --colmap reconstruct/scripts/colmap-docker
```

The wrapper mounts `$HOME` and `/tmp` into the container at the same paths (add
more with `COLMAP_DOCKER_MOUNTS=/data:/mnt/share`). It uses the GPU when the
NVIDIA container runtime is installed, and the CPU otherwise — slower, but about
a minute for 120 photos.

Check that everything is found:

```bash
vstimd-scene-from-capture check --colmap reconstruct/scripts/colmap-docker --brush ~/brush/brush_app
```

Instead of the flags, you can set `VSTIMD_COLMAP` and `VSTIMD_BRUSH`, or put
`colmap` and `brush_app` on `PATH`.

## 3. Run

```bash
vstimd-scene-from-capture run photos/ --out corridor.ply \
    --capture-path-length-cm 500 --capture-height-cm 8 \
    --colmap reconstruct/scripts/colmap-docker --brush ~/brush/brush_app
```

It works through six stages and prints progress:

| Stage | What happens |
|---|---|
| `ingest` | collects the photos in file-name order |
| `sfm` | COLMAP finds where each photo was taken |
| `undistort` | COLMAP removes lens distortion and scales the photos down |
| `align` | levels, scales and places the scene for vstimd |
| `train` | Brush trains the splats — by far the longest stage |
| `finish` | crops to the corridor, caps the splat count, writes `corridor.ply` |

On the development desktop (GTX 1650, COLMAP on the CPU), 118 photos at
7 000 training steps took under four minutes. The default of 30 000 steps
takes proportionally longer and gives a sharper scene.

Useful options (`vstimd-scene-from-capture run --help` lists all):

| Option | Default | Use |
|---|---|---|
| `--capture-path-length-cm` | required | first to last photo, as walked |
| `--capture-height-cm` | from the floor | cross-checks the floor; replaces it when no floor is found |
| `--train-steps` | 30000 | 7000 for a quick look |
| `--max-splats` | 1000000 | fewer for slower GPUs; the least visible splats are dropped |
| `--max-image-px` | 1600 | long edge of the photos used for training |
| `--order unordered` | `sequential` | photos not taken in one walk |
| `--crop-width-cm`, `--crop-height-cm`, `--crop-length-cm`, `--crop-margin-cm` | from the capture, margin 50 | the box kept around the corridor |
| `--keep-work` | off | keep COLMAP's intermediate data |

### The job directory

Everything about a run lives in `corridor.ply.reconstruction/` (or `--work`):

- `job.toml` holds the request.
- `state.json` holds the current stage and progress. Watch it from another shell
  with `jq . state.json`.
- `log.txt` holds every tool's output.
- `report.json` holds the result.

**Resume:** if a run stops — a crash, Ctrl-C, a reboot — run the same command
again, or `vstimd-scene-from-capture resume corridor.ply.reconstruction`. Finished
stages are skipped. Running with different options against the same directory is
refused; choose another `--work`.

**Redo a stage:** `resume <dir> --from <stage>` reruns that stage and everything
after it. For example, `--from finish` rewrites the output without retraining.

## 4. Read the report

`report.json` and the summary at the end of a run tell you whether the capture
worked:

- **`alignment.images_registered` against `images_total`.** Under 80 % means
  gaps: too little overlap, blur or blank walls.
- **`alignment.capture_height_cm`**, the camera height the floor fit found. It
  should match what you measured; the tool warns when it is more than 10 % off
  `--capture-height-cm`.
- **`alignment.up_correction_deg`**, roughly how far the phone was tilted.
- **`alignment.warnings`**:
    - "spreads … sideways" means the walk was not straight.
    - "look sideways" or "look back" means the photos did not face down the
      corridor.
    - "no floor was found" means you should pass `--capture-height-cm`.
- **`splats_written`**, along with how many were cropped away or dropped by the
  cap.

## 5. Use it in vstimd

The scene loads with no transform. The path is a file on the machine vstimd runs
on:

```python
from vstimd import Connection
from vstimd.stimuli import Vec3
from vstimd.system import Camera3D

with Connection() as conn:
    conn.stimuli.shapes3d.create_gaussian_splat("/data/scenes/corridor.ply", name="corridor")
    # Put the camera where the photos were taken from: capture_height_cm in the report.
    conn.system.set_camera(Camera3D(position_cm=Vec3(0, 8, 0)))
    walk = conn.animations.create_linear_nav_3d(
        20.0,                   # cm/s
        track_length_cm=500,    # the walked path
        fade_frames=30,         # fade out, jump back to the start, fade in
    )
    conn.animations.arm(walk)
```

Keep the virtual camera close to the capture height and heading. Views far from
where the photos were taken fall apart into large blurry splats.
