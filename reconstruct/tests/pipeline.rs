//! The whole `vstimd-scene-from-capture` binary against fake `colmap` and `brush`
//! scripts, which hand back a synthetic corridor: stage order, resume, the
//! lock, and that the scene comes out where vstimd expects it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use glam::{DQuat, DVec3, Quat, Vec3};
use reconstruct::colmap::{Image, Model, Point};
use vsplat::{Gaussian, Gaussians};

const FAKE_COLMAP: &str = r#"#!/bin/sh
echo "colmap $*" >> "$FAKE_CALLS"
cmd=$1; shift
case " $* " in *" -h "*)
  if [ "$cmd" = global_mapper ] && [ -n "$FAKE_NO_GLOBAL" ]; then exit 1; fi
  exit 0;;
esac
out=""; db=""
while [ $# -gt 0 ]; do
  case "$1" in
    --output_path) out=$2; shift;;
    --database_path) db=$2; shift;;
  esac
  shift
done
case "$cmd" in
  help) echo "COLMAP 3.12.0 -- fake";;
  feature_extractor) touch "$db"; echo "Processed file [1/2]"; echo "Processed file [2/2]";;
  sequential_matcher|exhaustive_matcher) ;;
  global_mapper|mapper) mkdir -p "$out/0"; touch "$out/0/cameras.bin";;
  model_converter) cp "$FIXTURE"/sparse_txt/* "$out"/;;
  image_undistorter) mkdir -p "$out/sparse" "$out/images"; touch "$out/sparse/cameras.bin";;
  *) echo "unknown command $cmd" >&2; exit 1;;
esac
"#;

const FAKE_BRUSH: &str = r#"#!/bin/sh
echo "brush $*" >> "$FAKE_CALLS"
if [ "$1" = "--version" ]; then echo "brush-cli 0.3.0"; exit 0; fi
if [ -n "$FAKE_BRUSH_FAIL" ]; then echo "out of GPU memory" >&2; exit 3; fi
steps=""; path=""; name=""
shift
while [ $# -gt 0 ]; do
  case "$1" in
    --total-steps) steps=$2; shift;;
    --export-path) path=$2; shift;;
    --export-name) name=$2; shift;;
  esac
  shift
done
mkdir -p "$path"
cp "$FIXTURE/trained.ply" "$path/$(echo "$name" | sed "s/{iter}/$steps/")"
"#;

/// Writes the PGMs the scoring pass reads, then one JPEG per frame the select
/// filter names. Frames 2, 5 and 8 (1-based) are a checkerboard and the rest are
/// flat, so the sharpness pass has an unambiguous winner in each group of three.
const FAKE_FFMPEG: &str = r#"#!/bin/sh
echo "ffmpeg $*" >> "$FAKE_CALLS"
for last; do :; done
out=$last
vf=""
while [ $# -gt 0 ]; do
  case "$1" in -vf) vf=$2; shift;; esac
  shift
done
dir=$(dirname "$out")
mkdir -p "$dir"
case "$out" in
  *.pgm)
    i=1
    while [ $i -le 9 ]; do
      n=$(printf %06d $i)
      printf 'P5\n4 4\n255\n' > "$dir/$n.pgm"
      case $i in
        2|5|8) printf '\000\377\000\377\377\000\377\000\000\377\000\377\377\000\377\000' >> "$dir/$n.pgm";;
        *)     printf '\177\177\177\177\177\177\177\177\177\177\177\177\177\177\177\177' >> "$dir/$n.pgm";;
      esac
      i=$((i+1))
    done
    ;;
  *.jpg)
    echo "$vf" > "$FAKE_SELECT"
    k=$(echo "$vf" | tr '+' '\n' | grep -c 'eq(n')
    i=1
    while [ $i -le "$k" ]; do
      printf %05d.jpg $i | xargs -I{} sh -c 'printf jpeg > "$1/{}"' _ "$dir"
      i=$((i+1))
    done
    ;;
esac
"#;

/// SfM's arbitrary frame: small, turned and displaced.
fn to_model(p: DVec3) -> DVec3 {
    DQuat::from_euler(glam::EulerRot::YXZ, 0.8, 2.6, -0.4) * p / 23.0 + DVec3::new(1.0, 2.0, -3.0)
}

fn to_model_dir(d: DVec3) -> DVec3 {
    DQuat::from_euler(glam::EulerRot::YXZ, 0.8, 2.6, -0.4) * d
}

struct Fixture {
    root: PathBuf,
    images: PathBuf,
    out: PathBuf,
    colmap: PathBuf,
    brush: PathBuf,
    ffmpeg: PathBuf,
    calls: PathBuf,
    /// Splats the fake trainer returns that lie inside the corridor.
    inside: usize,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let root =
            std::env::temp_dir().join(format!("reconstruct-it-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let images = root.join("photos");
        std::fs::create_dir_all(&images).unwrap();
        for i in 1..=40 {
            std::fs::write(images.join(format!("IMG_{i}.jpg")), b"not really a jpeg").unwrap();
        }
        std::fs::write(images.join("notes.txt"), b"ignored").unwrap();

        // 40 cameras walking 500 cm down −Z at 30 cm; a 100 × 100 cm corridor.
        let fixture = root.join("fixture");
        let images_model = (0..40)
            .map(|i| {
                let c = DVec3::new(0.0, 30.0, -500.0 * i as f64 / 39.0);
                Image::looking(
                    i + 1,
                    &format!("{:05}.jpg", i + 1),
                    to_model(c),
                    to_model_dir(DVec3::NEG_Z),
                    to_model_dir(DVec3::Y),
                )
            })
            .collect();
        let mut points = Vec::new();
        for i in 0..400 {
            let z = 40.0 - 1.4 * i as f64;
            let t = (i % 10) as f64 / 10.0;
            for p in [
                DVec3::new(-50.0 + 100.0 * t, 0.0, z),
                DVec3::new(-50.0, 100.0 * t, z),
                DVec3::new(50.0, 100.0 * t, z),
            ] {
                points.push(Point {
                    position: to_model(p),
                    error: 0.7,
                });
            }
        }
        Model {
            images: images_model,
            points,
        }
        .write_text(&fixture.join("sparse_txt"))
        .unwrap();

        // Trained splats: a floor inside the corridor, plus floaters far outside.
        let mut splats = Vec::new();
        let rotation = DQuat::from_euler(glam::EulerRot::YXZ, 0.8, 2.6, -0.4).as_quat();
        let splat = |p: DVec3| Gaussian {
            position: to_model(p).as_vec3(),
            log_scale: Vec3::splat((2.0f32 / 23.0).ln()),
            rotation,
            opacity_logit: 2.0,
            f_dc: [0.5, 0.0, -0.5],
        };
        for i in 0..200 {
            splats.push(splat(DVec3::new(
                -40.0 + (i % 9) as f64 * 10.0,
                0.0,
                -2.5 * i as f64,
            )));
        }
        let inside = splats.len();
        for i in 0..7 {
            splats.push(splat(DVec3::new(0.0, 50.0, -2000.0 - 10.0 * i as f64)));
        }
        Gaussians {
            splats,
            sh_degree: 0,
            sh_rest: Vec::new(),
        }
        .write_ply(&fixture.join("trained.ply"))
        .unwrap();

        let script = |name: &str, body: &str| {
            let path = root.join(name);
            std::fs::write(&path, body).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        Fixture {
            colmap: script("colmap", FAKE_COLMAP),
            brush: script("brush", FAKE_BRUSH),
            ffmpeg: script("ffmpeg", FAKE_FFMPEG),
            calls: root.join("calls.txt"),
            out: root.join("scenes").join("corridor.ply"),
            images,
            root,
            inside,
        }
    }

    fn job_dir(&self) -> PathBuf {
        PathBuf::from(format!("{}.reconstruction", self.out.display()))
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vstimd-scene-from-capture"));
        cmd.args(args)
            .arg("--colmap")
            .arg(&self.colmap)
            .arg("--brush")
            .arg(&self.brush)
            .arg("--ffmpeg")
            .arg(&self.ffmpeg)
            .env("FAKE_SELECT", self.root.join("select.txt"))
            .env("FAKE_CALLS", &self.calls)
            .env("FIXTURE", self.root.join("fixture"))
            .env_remove("FAKE_BRUSH_FAIL")
            .env_remove("FAKE_NO_GLOBAL");
        cmd
    }

    fn run(&self, extra: &[&str], env: &[(&str, &str)]) -> Output {
        let mut args = vec![
            "run",
            self.images.to_str().unwrap(),
            "--out",
            self.out.to_str().unwrap(),
            "--capture-path-length-cm",
            "500",
            "--train-steps",
            "100",
        ];
        args.extend_from_slice(extra);
        let mut cmd = self.command(&args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    fn calls(&self, needle: &str) -> usize {
        std::fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with(needle))
            .count()
    }

    fn state(&self) -> serde_json::Value {
        read_json(&self.job_dir().join("state.json"))
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_run_writes_the_scene_in_vstimds_frame() {
    let f = Fixture::new("full");
    let out = f.run(&[], &[]);
    assert!(out.status.success(), "{}", stderr(&out));

    let scene = Gaussians::read_ply(&f.out).unwrap();
    assert_eq!(scene.len(), f.inside, "the floaters are cropped away");
    for g in &scene.splats {
        let p = g.position;
        assert!(p.y.abs() < 2.0, "floor splat off the floor: {p}");
        assert!(
            p.x.abs() < 45.0 && p.z < 5.0 && p.z > -505.0,
            "outside the corridor: {p}"
        );
        // 2 cm splats, whatever SfM's scale was.
        assert!(
            (g.log_scale.exp().x - 2.0).abs() < 0.05,
            "{}",
            g.log_scale.exp()
        );
        // Trained turned with SfM's frame; aligned, they are unrotated again.
        assert!(
            g.rotation.dot(Quat::IDENTITY).abs() > 0.999,
            "{}",
            g.rotation
        );
    }
    // The first floor splat was 40 cm left of the first camera, on the floor.
    let first = scene.splats[0].position;
    assert!(
        (first - Vec3::new(-40.0, 0.0, 0.0)).length() < 1.0,
        "{first}"
    );

    let report = read_json(&f.job_dir().join("report.json"));
    assert_eq!(report["splats_written"], f.inside);
    assert_eq!(report["splats_outside_crop"], 7);
    assert_eq!(report["images_total"], 40);
    assert_eq!(report["alignment"]["images_registered"], 40);
    let height = report["alignment"]["capture_height_cm"].as_f64().unwrap();
    assert!((height - 30.0).abs() < 2.0, "{height}");
    assert!(report["stage_seconds"]["train"].is_number(), "{report}");

    assert_eq!(f.state()["status"], "succeeded");
    assert!(
        !f.job_dir().join("work").exists(),
        "work/ is removed after success"
    );
    assert_eq!(f.calls("colmap global_mapper --database_path"), 1);
    assert!(stderr(&out).contains("track_length_cm"), "{}", stderr(&out));

    // Images were taken in natural order: IMG_2 before IMG_10.
    let images = read_json(&f.job_dir().join("images.json"));
    assert!(
        images[1]["source"].as_str().unwrap().ends_with("IMG_2.jpg"),
        "{images}"
    );
    assert_eq!(
        images.as_array().unwrap().len(),
        40,
        "notes.txt is not an image"
    );
}

#[test]
fn a_failed_run_resumes_where_it_stopped() {
    let f = Fixture::new("resume");
    let out = f.run(&[], &[("FAKE_BRUSH_FAIL", "1")]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("out of GPU memory"),
        "{}",
        stderr(&out)
    );
    let state = f.state();
    assert_eq!(state["status"], "failed");
    assert_eq!(state["stage"], "train");
    assert!(
        state["error"].as_str().unwrap().contains("brush"),
        "{state}"
    );
    assert!(f.job_dir().join(".done-align").exists());
    assert!(!f.job_dir().join(".done-train").exists());

    let out = f.run(&[], &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        f.calls("colmap feature_extractor"),
        1,
        "finished stages are not rerun"
    );
    assert!(f.out.exists());

    // `--from finish` rewrites the output from trained.ply, with work/ gone.
    std::fs::remove_file(&f.out).unwrap();
    let job = f.job_dir();
    let out = f
        .command(&["resume", job.to_str().unwrap(), "--from", "finish"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(f.out.exists());
    assert_eq!(f.calls("brush /"), 2, "training is not rerun");
}

#[test]
fn a_job_directory_belongs_to_one_request_and_one_worker() {
    let f = Fixture::new("owner");
    assert!(!f.run(&[], &[("FAKE_BRUSH_FAIL", "1")]).status.success());

    let out = f.run(&["--max-splats", "5"], &[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("different parameters"),
        "{}",
        stderr(&out)
    );

    let lock = std::fs::OpenOptions::new()
        .write(true)
        .open(f.job_dir().join(".lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let job = f.job_dir();
    let out = f
        .command(&["resume", job.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("another worker"), "{}", stderr(&out));
}

#[test]
fn the_incremental_mapper_stands_in_for_a_missing_global_one() {
    let f = Fixture::new("mapper");
    let out = f.run(&["--max-splats", "50"], &[("FAKE_NO_GLOBAL", "1")]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(f.calls("colmap mapper --database_path"), 1);
    assert_eq!(f.calls("colmap global_mapper --database_path"), 0);
    let report = read_json(&f.job_dir().join("report.json"));
    assert_eq!(report["splats_written"], 50);
    assert_eq!(report["splats_over_cap"], f.inside - 50);
}

#[test]
fn check_reports_the_tools() {
    let f = Fixture::new("check");
    let out = f.command(&["check"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["ready"], true);
    assert_eq!(report["colmap_global_mapper"], true);
    assert_eq!(report["brush"]["version"], "brush-cli 0.3.0");

    let out = Command::new(env!("CARGO_BIN_EXE_vstimd-scene-from-capture"))
        .args(["check", "--colmap", "/nonexistent/colmap", "--brush"])
        .arg(&f.brush)
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn a_video_capture_keeps_the_sharpest_frame_of_each_group() {
    let f = Fixture::new("video");
    let video = f.root.join("walk.mp4");
    std::fs::write(&video, b"not really a video; the fake ffmpeg never reads it").unwrap();

    let out = f.command(&[
        "run",
        video.to_str().unwrap(),
        "--out",
        f.out.to_str().unwrap(),
        "--capture-path-length-cm",
        "500",
        "--train-steps",
        "100",
        "--fps",
        "3",
        "--frame-oversample",
        "3",
    ])
    .output()
    .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    // Twice over the video: score every candidate, then fetch only the winners.
    assert_eq!(f.calls("ffmpeg"), 2, "expected a scoring pass and a fetch pass");

    // Both passes must write one file per filtered frame. Without -fps_mode,
    // ffmpeg pads back to a constant rate: measured against ffmpeg 9, the fetch
    // pass wrote 8 files for the 3 frames asked for, and the scoring pass's
    // numbering would stop being the index `select` addresses. (-vsync 0 did
    // the same job and was removed in ffmpeg 8.)
    let calls = std::fs::read_to_string(&f.calls).unwrap();
    let ffmpeg_calls: Vec<&str> = calls.lines().filter(|l| l.starts_with("ffmpeg")).collect();
    for call in &ffmpeg_calls {
        assert!(
            call.contains("-fps_mode passthrough"),
            "every ffmpeg pass needs -fps_mode passthrough, got {call}"
        );
        assert!(!call.contains("-vsync"), "-vsync is gone in ffmpeg 8+: {call}");
    }

    // The fake makes frames 2, 5 and 8 (1-based) sharp, so with groups of three
    // the winners are stream indices 1, 4 and 7. This is the assertion that
    // would catch the sharpness pass silently picking by position instead.
    let select = std::fs::read_to_string(f.root.join("select.txt")).unwrap();
    assert!(
        select.contains(r"eq(n\,1)") && select.contains(r"eq(n\,4)") && select.contains(r"eq(n\,7)"),
        "the second pass should ask for the sharp frames, got {select}"
    );
    assert!(
        !select.contains(r"eq(n\,0)") && !select.contains(r"eq(n\,2)"),
        "the second pass should not ask for the blurred ones, got {select}"
    );
    // Candidates are taken at fps x oversample.
    assert!(select.starts_with("fps=9"), "got {select}");

    // images.json records which frame of the video each image came from, so a
    // scene stays traceable to its capture.
    let images: serde_json::Value = read_json(&f.job_dir().join("images.json"));
    let entries = images.as_array().expect("images.json is a list");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["frame"], 1);
    assert_eq!(entries[2]["frame"], 7);
    assert!(
        entries[0]["source"].as_str().unwrap().ends_with("walk.mp4"),
        "each frame's source is the video it came from"
    );
}

#[test]
fn a_video_capture_without_ffmpeg_says_so() {
    let f = Fixture::new("video-no-ffmpeg");
    let video = f.root.join("walk.mp4");
    std::fs::write(&video, b"x").unwrap();

    // No --ffmpeg, and nothing on PATH to find.
    let out = Command::new(env!("CARGO_BIN_EXE_vstimd-scene-from-capture"))
        .args([
            "run",
            video.to_str().unwrap(),
            "--out",
            f.out.to_str().unwrap(),
            "--capture-path-length-cm",
            "500",
        ])
        .arg("--colmap")
        .arg(&f.colmap)
        .arg("--brush")
        .arg(&f.brush)
        .env("PATH", "")
        .env("FAKE_CALLS", &f.calls)
        .env_remove("VSTIMD_FFMPEG")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("ffmpeg"), "{err}");
    assert!(
        err.contains("folder of extracted frames"),
        "the error should offer the way out that needs no ffmpeg: {err}"
    );
}
