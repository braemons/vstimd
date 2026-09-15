//! The pipeline, one function per stage, run in order and skipped when stamped.
//!
//! Every stage starts by removing whatever an interrupted run of it left
//! behind, so rerunning a stage is always safe.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use vsplat::Gaussians;

use crate::align::{self, Alignment};
use crate::colmap::Model;
use crate::job::{Job, Stage, unix_ms, write_atomic};
use crate::params::{CaptureOrder, Mapper, OutputFormat, output_format};
use crate::tools::{Tools, colmap_has_global_mapper, parse_bracket_counter, run_tool};
use crate::trainer::trainer;

const MIN_IMAGES: usize = 3;
const FEW_IMAGES: usize = 30;

/// Run every stage of `job` not yet done, then mark it succeeded or failed.
pub fn run(job: &mut Job, tools: &Tools) -> anyhow::Result<Report> {
    let result = run_stages(job, tools);
    match &result {
        Ok(_) => job.succeed(),
        Err(e) => job.fail(e),
    }
    result
}

fn run_stages(job: &mut Job, tools: &Tools) -> anyhow::Result<Report> {
    for stage in Stage::ALL {
        if job.is_done(stage) {
            continue;
        }
        job.begin_stage(stage);
        let t0 = Instant::now();
        match stage {
            Stage::Ingest => ingest(job),
            Stage::Sfm => sfm(job, tools),
            Stage::Undistort => undistort(job, tools),
            Stage::Align => align_stage(job),
            Stage::Train => train(job, tools),
            Stage::Finish => finish(job),
        }
        .with_context(|| format!("stage {}", stage.name()))?;
        job.end_stage(stage, t0.elapsed().as_secs_f64())?;
    }
    read_json(&job.dir.join("report.json"))
}

fn fresh_dir(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    }
    std::fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    write_atomic(path, &serde_json::to_vec_pretty(value).expect("serialises"))
}

// ── ingest ────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct IngestedImage {
    name: String,
    source: PathBuf,
}

/// `a2.jpg` before `a10.jpg`: runs of digits compare as numbers.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, run)) if *d == digit => run.push(c),
                _ => out.push((digit, c.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for ((da, ra), (db, rb)) in ca.iter().zip(&cb) {
        let ord = if *da && *db {
            let (ta, tb) = (ra.trim_start_matches('0'), rb.trim_start_matches('0'));
            ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb))
        } else {
            ra.to_lowercase().cmp(&rb.to_lowercase())
        };
        if ord.is_ne() {
            return ord;
        }
    }
    ca.len().cmp(&cb.len()).then_with(|| a.cmp(b))
}

fn ingest(job: &mut Job) -> anyhow::Result<()> {
    let src = job.params.images_dir.clone();
    let entries = std::fs::read_dir(&src).with_context(|| format!("reading {}", src.display()))?;
    let mut files: Vec<(String, PathBuf)> = entries
        .filter_map(|e| {
            let path = e.ok()?.path();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            if !(path.is_file() && matches!(ext.as_str(), "jpg" | "jpeg" | "png")) {
                return None;
            }
            Some((path.file_name()?.to_str()?.to_string(), path))
        })
        .collect();
    files.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    if files.len() < MIN_IMAGES {
        bail!(
            "{} holds {} JPEG or PNG images; at least {MIN_IMAGES} are needed",
            src.display(),
            files.len()
        );
    }
    if files.len() < FEW_IMAGES {
        job.note(&format!(
            "warning: only {} images; a corridor usually needs a hundred or more",
            files.len()
        ));
    }

    let images = job.work().join("images");
    fresh_dir(&images)?;
    let mut ingested = Vec::with_capacity(files.len());
    for (i, (_, source)) in files.iter().enumerate() {
        let source = source
            .canonicalize()
            .with_context(|| format!("resolving {}", source.display()))?;
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_ascii_lowercase();
        // Numbered in capture order: COLMAP's sequential matcher and the
        // alignment both read order from names.
        let name = format!("{:05}.{ext}", i + 1);
        let dst = images.join(&name);
        // A link keeps EXIF (COLMAP reads the focal length from it) and costs
        // no space; a Samba share may refuse links, so fall back to copying.
        std::os::unix::fs::symlink(&source, &dst)
            .or_else(|_| std::fs::hard_link(&source, &dst))
            .or_else(|_| std::fs::copy(&source, &dst).map(|_| ()))
            .with_context(|| format!("linking {} into the job", source.display()))?;
        ingested.push(IngestedImage { name, source });
        job.progress(Some((i + 1) as f32 / files.len() as f32));
    }
    job.note(&format!("{} images", ingested.len()));
    write_json(&job.dir.join("images.json"), &ingested)
}

// ── sfm ───────────────────────────────────────────────────────────────────────

fn colmap_cmd(colmap: &Path, command: &str) -> Command {
    let mut cmd = Command::new(colmap);
    cmd.arg(command);
    cmd
}

fn sfm(job: &mut Job, tools: &Tools) -> anyhow::Result<()> {
    let colmap = tools.colmap()?.to_path_buf();
    let work = job.work();
    let (images, db, sparse, sparse_txt) = (
        work.join("images"),
        work.join("database.db"),
        work.join("sparse"),
        work.join("sparse_txt"),
    );
    for suffix in ["", "-shm", "-wal"] {
        let _ = std::fs::remove_file(work.join(format!("database.db{suffix}")));
    }
    fresh_dir(&sparse)?;
    fresh_dir(&sparse_txt)?;

    let mut extract = colmap_cmd(&colmap, "feature_extractor");
    extract
        .arg("--database_path")
        .arg(&db)
        .arg("--image_path")
        .arg(&images)
        // One walk, one camera: shared intrinsics are better constrained.
        .args(["--ImageReader.single_camera", "1"]);
    run_tool(job, extract, &parse_bracket_counter, &mut || None)?;

    let matcher = match job.params.order {
        CaptureOrder::Sequential => "sequential_matcher",
        CaptureOrder::Unordered => "exhaustive_matcher",
    };
    let mut matching = colmap_cmd(&colmap, matcher);
    matching.arg("--database_path").arg(&db);
    job.progress(None);
    run_tool(job, matching, &parse_bracket_counter, &mut || None)?;

    let global = job.params.mapper == Mapper::Global && colmap_has_global_mapper(&colmap);
    if job.params.mapper == Mapper::Global && !global {
        job.note("this COLMAP has no global_mapper; using the incremental mapper");
    }
    let mut mapping = colmap_cmd(&colmap, if global { "global_mapper" } else { "mapper" });
    mapping
        .arg("--database_path")
        .arg(&db)
        .arg("--image_path")
        .arg(&images)
        .arg("--output_path")
        .arg(&sparse);
    job.progress(None);
    run_tool(job, mapping, &|_| None, &mut || None)?;

    // The mapper may split the capture into several models; keep the largest.
    let mut best: Option<(usize, String)> = None;
    let mut models: Vec<String> = std::fs::read_dir(&sparse)?
        .filter_map(|e| {
            let e = e.ok()?;
            e.path()
                .is_dir()
                .then(|| e.file_name().to_string_lossy().into_owned())
        })
        .collect();
    models.sort_by(|a, b| natural_cmp(a, b));
    for name in &models {
        let txt = sparse_txt.join(name);
        std::fs::create_dir_all(&txt)?;
        let mut convert = colmap_cmd(&colmap, "model_converter");
        convert
            .arg("--input_path")
            .arg(sparse.join(name))
            .arg("--output_path")
            .arg(&txt)
            .args(["--output_type", "TXT"]);
        run_tool(job, convert, &|_| None, &mut || None)?;
        let registered = Model::read_text(&txt)?.images.len();
        if best.as_ref().is_none_or(|(n, _)| registered > *n) {
            best = Some((registered, name.clone()));
        }
    }
    let Some((registered, name)) = best else {
        bail!(
            "COLMAP reconstructed nothing: the photos may overlap too little or show too little texture"
        );
    };
    let total = read_json::<Vec<IngestedImage>>(&job.dir.join("images.json"))?.len();
    job.note(&format!(
        "{registered} of {total} images registered{}",
        if models.len() > 1 {
            format!(" (largest of {} models)", models.len())
        } else {
            String::new()
        }
    ));
    if registered * 10 < total * 8 {
        job.note("warning: under 80 % of the images registered; the scene will have gaps");
    }
    write_atomic(&work.join("sparse_best"), name.as_bytes())
}

fn best_model(job: &Job) -> anyhow::Result<String> {
    let path = job.work().join("sparse_best");
    Ok(std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?
        .trim()
        .to_string())
}

// ── undistort ─────────────────────────────────────────────────────────────────

fn undistort(job: &mut Job, tools: &Tools) -> anyhow::Result<()> {
    let colmap = tools.colmap()?.to_path_buf();
    let work = job.work();
    let dense = work.join("dense");
    fresh_dir(&dense)?;
    let mut cmd = colmap_cmd(&colmap, "image_undistorter");
    cmd.arg("--image_path")
        .arg(work.join("images"))
        .arg("--input_path")
        .arg(work.join("sparse").join(best_model(job)?))
        .arg("--output_path")
        .arg(&dense)
        .args(["--output_type", "COLMAP"])
        .arg("--max_image_size")
        .arg(job.params.max_image_px.to_string());
    run_tool(job, cmd, &parse_bracket_counter, &mut || None)?;
    if !dense.join("sparse").is_dir() {
        bail!(
            "image_undistorter wrote no sparse/ into {}",
            dense.display()
        );
    }
    Ok(())
}

// ── align ─────────────────────────────────────────────────────────────────────

fn align_stage(job: &mut Job) -> anyhow::Result<()> {
    let model = Model::read_text(&job.work().join("sparse_txt").join(best_model(job)?))?;
    let a = align::estimate(&model, &job.params.alignment, &job.params.crop)?;
    job.note(&format!(
        "{:.1} cm per unit; camera {:.0} cm above the floor{}; path {:.0} cm",
        a.scale,
        a.capture_height_cm,
        if a.floor.is_some() { "" } else { " (given)" },
        a.path_length_cm
    ));
    for w in &a.warnings {
        job.note(&format!("warning: {w}"));
    }
    write_json(&job.dir.join("alignment.json"), &a)
}

// ── train ─────────────────────────────────────────────────────────────────────

fn train(job: &mut Job, tools: &Tools) -> anyhow::Result<()> {
    let trainer = trainer(job.params.trainer, tools)?;
    let work = job.work();
    let out = work.join("train");
    fresh_dir(&out)?;
    let params = job.params.clone();
    let cmd = trainer.command(&work.join("dense"), &params, &out);
    run_tool(job, cmd, &|_| None, &mut || trainer.progress(&out, &params))?;
    let result = trainer.result(&out, &params)?;
    let trained = job.dir.join("trained.ply");
    std::fs::rename(&result, &trained)
        .or_else(|_| std::fs::copy(&result, &trained).map(|_| ()))
        .with_context(|| format!("moving {} to {}", result.display(), trained.display()))
}

// ── finish ────────────────────────────────────────────────────────────────────

/// `report.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub output: PathBuf,
    pub splats_trained: usize,
    pub splats_outside_crop: usize,
    pub splats_over_cap: usize,
    pub splats_written: usize,
    pub sh_degree: u32,
    pub images_total: usize,
    pub alignment: Alignment,
    pub stage_seconds: serde_json::Map<String, serde_json::Value>,
    pub finished_unix_ms: u64,
}

fn finish(job: &mut Job) -> anyhow::Result<()> {
    let alignment: Alignment = read_json(&job.dir.join("alignment.json"))?;
    let mut scene = Gaussians::read_ply(&job.dir.join("trained.ply"))?;
    let trained = scene.len();

    scene.truncate_sh(job.params.sh_degree);
    scene
        .apply_similarity(
            alignment.scale,
            alignment.rotation_quat(),
            alignment.translation_cm.into(),
        )
        .map_err(anyhow::Error::msg)?;

    scene.retain(|_, g| alignment.crop.contains(g.position));
    let outside = trained - scene.len();
    let over_cap = cap(&mut scene, job.params.max_splats as usize);
    if scene.is_empty() {
        bail!(
            "no splats are left inside the crop box {:?}; check alignment.json",
            alignment.crop
        );
    }

    let output = job.params.output.clone();
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let name = output
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("scene");
    let tmp = output.with_file_name(format!(".{name}.tmp"));
    match output_format(&output) {
        Some(OutputFormat::Ply) => scene.write_ply(&tmp),
        Some(OutputFormat::Splat) => scene.write_splat(&tmp),
        None => unreachable!("validated with the parameters"),
    }
    .with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &output).with_context(|| format!("replacing {}", output.display()))?;

    job.note(&format!(
        "{} splats written ({} trained, {} outside the crop, {} over the cap)",
        scene.len(),
        trained,
        outside,
        over_cap
    ));
    let report = Report {
        output: output.clone(),
        splats_trained: trained,
        splats_outside_crop: outside,
        splats_over_cap: over_cap,
        splats_written: scene.len(),
        sh_degree: scene.sh_degree,
        images_total: read_json::<Vec<IngestedImage>>(&job.dir.join("images.json"))
            .map(|v| v.len())
            .unwrap_or(0),
        alignment,
        stage_seconds: job.stage_seconds(),
        finished_unix_ms: unix_ms(),
    };
    write_json(&job.dir.join("report.json"), &report)?;

    if !job.params.keep_work && job.work().exists() {
        std::fs::remove_dir_all(job.work()).context("removing work/")?;
    }
    Ok(())
}

/// Keep the `max` splats that contribute most — opacity × volume — in their
/// original order. Returns how many were dropped.
fn cap(scene: &mut Gaussians, max: usize) -> usize {
    let n = scene.len();
    if n <= max {
        return 0;
    }
    let score = |g: &vsplat::Gaussian| g.opacity() * g.volume();
    let mut ranked: Vec<(f32, usize)> = scene
        .splats
        .iter()
        .enumerate()
        .map(|(i, g)| (score(g), i))
        .collect();
    // Highest first; ties broken by index so the result is deterministic.
    ranked.select_nth_unstable_by(max, |a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut keep = vec![false; n];
    for &(_, i) in &ranked[..max] {
        keep[i] = true;
    }
    scene.retain(|i, _| keep[i]);
    n - max
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn natural_order_counts() {
        let mut v = vec!["IMG_10.jpg", "IMG_2.jpg", "img_1.JPG", "IMG_02b.jpg"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["img_1.JPG", "IMG_2.jpg", "IMG_02b.jpg", "IMG_10.jpg"]);
    }

    #[test]
    fn the_cap_keeps_the_most_visible_splats_in_order() {
        let g = |opacity_logit: f32, log_scale: f32| vsplat::Gaussian {
            position: Vec3::ZERO,
            log_scale: Vec3::splat(log_scale),
            rotation: Quat::IDENTITY,
            opacity_logit,
            f_dc: [0.0; 3],
        };
        let mut scene = Gaussians {
            splats: vec![g(-5.0, 0.0), g(5.0, 0.0), g(5.0, -3.0), g(5.0, 1.0)],
            sh_degree: 0,
            sh_rest: Vec::new(),
        };
        assert_eq!(cap(&mut scene, 2), 2);
        assert_eq!(scene.splats, vec![g(5.0, 0.0), g(5.0, 1.0)]);
        assert_eq!(cap(&mut scene, 5), 0);
    }
}
