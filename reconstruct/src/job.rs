//! A job is a directory, and the directory is the only state.
//!
//! ```text
//! <job>/
//!   job.toml          the request; written once, never rewritten
//!   state.json        {stage, status, stage_progress, …}; replaced atomically
//!   log.txt           every tool's output, appended
//!   alignment.json    written by `align`
//!   trained.ply       written by `train`, in the reconstruction's own frame
//!   report.json       written by `finish`
//!   .done-<stage>     one per finished stage: the resume points
//!   .lock             held (flock) by the worker running the job
//!   work/             images, COLMAP database and models, undistorted images,
//!                     trainer output; deleted after success unless keep_work
//! ```
//!
//! `alignment.json` and `trained.ply` sit outside `work/` so that `finish` can
//! be rerun (with `resume --from finish`) after `work/` is gone.

use std::fs::{File, OpenOptions};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::params::JobParams;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum Stage {
    Ingest,
    Sfm,
    Undistort,
    Align,
    Train,
    Finish,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Ingest,
        Stage::Sfm,
        Stage::Undistort,
        Stage::Align,
        Stage::Train,
        Stage::Finish,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Ingest => "ingest",
            Stage::Sfm => "sfm",
            Stage::Undistort => "undistort",
            Stage::Align => "align",
            Stage::Train => "train",
            Stage::Finish => "finish",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Stage::Ingest => "collecting images",
            Stage::Sfm => "recovering camera poses (COLMAP)",
            Stage::Undistort => "undistorting images (COLMAP)",
            Stage::Align => "aligning to the corridor",
            Stage::Train => "training splats",
            Stage::Finish => "cropping and writing the scene",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Succeeded,
    Failed,
}

/// `state.json`. A `running` state whose `.lock` nobody holds was interrupted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobState {
    pub stage: String,
    pub status: Status,
    /// `[0, 1]`, or −1 when the running tool reports no progress.
    pub stage_progress: f32,
    pub started_unix_ms: u64,
    pub updated_unix_ms: u64,
    pub worker_host: String,
    pub worker_pid: u32,
    pub error: String,
}

pub struct Job {
    pub dir: PathBuf,
    pub params: JobParams,
    /// Echo tool output to the terminal as well as the log.
    pub verbose: bool,
    state: JobState,
    log: File,
    last_state_write: Instant,
    interactive: bool,
    _lock: File,
}

pub fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

impl Job {
    /// Create the job directory for `params`, or reopen it if it already holds
    /// the same request.
    pub fn create_or_open(dir: &Path, params: JobParams) -> anyhow::Result<Job> {
        params.validate()?;
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let lock = lock(dir)?;
        let toml_path = dir.join("job.toml");
        if toml_path.exists() {
            let existing = JobParams::read(&toml_path)?;
            if existing != params {
                bail!(
                    "{} already holds a job with different parameters.\n\
                     Continue it with `vstimd-reconstruct resume {}`, or pass another --work.\n\
                     existing:\n{}\nrequested:\n{}",
                    dir.display(),
                    dir.display(),
                    existing.to_toml(),
                    params.to_toml()
                );
            }
        } else {
            write_atomic(&toml_path, params.to_toml().as_bytes())?;
        }
        Self::with_lock(dir, params, lock)
    }

    /// Reopen an existing job from its `job.toml`.
    pub fn open(dir: &Path) -> anyhow::Result<Job> {
        let toml_path = dir.join("job.toml");
        if !toml_path.exists() {
            bail!("{} is not a job directory (no job.toml)", dir.display());
        }
        let lock = lock(dir)?;
        let params = JobParams::read(&toml_path)?;
        params.validate()?;
        Self::with_lock(dir, params, lock)
    }

    fn with_lock(dir: &Path, params: JobParams, lock: File) -> anyhow::Result<Job> {
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("log.txt"))
            .context("opening log.txt")?;
        let now = unix_ms();
        let mut job = Job {
            dir: dir.to_path_buf(),
            params,
            verbose: false,
            state: JobState {
                stage: String::new(),
                status: Status::Running,
                stage_progress: -1.0,
                started_unix_ms: now,
                updated_unix_ms: now,
                worker_host: hostname(),
                worker_pid: std::process::id(),
                error: String::new(),
            },
            log,
            last_state_write: Instant::now(),
            interactive: std::io::stderr().is_terminal(),
            _lock: lock,
        };
        job.log_line(&format!(
            "=== worker {} pid {} started",
            job.state.worker_host, job.state.worker_pid
        ));
        Ok(job)
    }

    pub fn work(&self) -> PathBuf {
        self.dir.join("work")
    }

    fn stamp(&self, stage: Stage) -> PathBuf {
        self.dir.join(format!(".done-{}", stage.name()))
    }

    pub fn is_done(&self, stage: Stage) -> bool {
        self.stamp(stage).exists()
    }

    /// Forget `stage` and every later one, so they run again.
    pub fn clear_from(&self, stage: Stage) -> anyhow::Result<()> {
        for s in Stage::ALL.into_iter().filter(|s| *s >= stage) {
            match std::fs::remove_file(self.stamp(s)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        Ok(())
    }

    /// Seconds each finished stage took, from the stamps.
    pub fn stage_seconds(&self) -> serde_json::Map<String, serde_json::Value> {
        Stage::ALL
            .into_iter()
            .filter_map(|s| {
                let text = std::fs::read_to_string(self.stamp(s)).ok()?;
                let v: serde_json::Value = serde_json::from_str(&text).ok()?;
                Some((s.name().to_string(), v.get("seconds")?.clone()))
            })
            .collect()
    }

    pub fn begin_stage(&mut self, stage: Stage) {
        let index = Stage::ALL.iter().position(|s| *s == stage).unwrap_or(0) + 1;
        eprintln!(
            "[{index}/{}] {}: {}",
            Stage::ALL.len(),
            stage.name(),
            stage.describe()
        );
        self.log_line(&format!("=== stage {}", stage.name()));
        self.state.stage = stage.name().into();
        self.state.stage_progress = -1.0;
        self.write_state();
    }

    pub fn end_stage(&mut self, stage: Stage, seconds: f64) -> anyhow::Result<()> {
        if self.interactive && self.state.stage_progress >= 0.0 {
            eprintln!();
        }
        eprintln!("      done in {}", format_seconds(seconds));
        let stamp = serde_json::json!({ "seconds": (seconds * 10.0).round() / 10.0, "finished_unix_ms": unix_ms() });
        write_atomic(&self.stamp(stage), stamp.to_string().as_bytes())
    }

    /// Report the running tool's progress; `None` when it gives none.
    /// `state.json` is rewritten at most once a second.
    pub fn progress(&mut self, fraction: Option<f32>) {
        let p = fraction.map_or(-1.0, |f| f.clamp(0.0, 1.0));
        if p == self.state.stage_progress {
            return;
        }
        self.state.stage_progress = p;
        if self.interactive && p >= 0.0 {
            eprint!("\r      {:>5.1} %", 100.0 * p);
        }
        if self.last_state_write.elapsed().as_secs_f32() >= 1.0 {
            self.write_state();
        }
    }

    pub fn log_line(&mut self, line: &str) {
        let _ = writeln!(self.log, "{line}");
        if self.verbose {
            if self.interactive && self.state.stage_progress >= 0.0 {
                eprint!("\r");
            }
            eprintln!("      | {line}");
        }
    }

    /// A note for the user that also belongs in the log.
    pub fn note(&mut self, line: &str) {
        eprintln!("      {line}");
        self.log_line(line);
    }

    pub fn fail(&mut self, error: &anyhow::Error) {
        if self.interactive && self.state.stage_progress >= 0.0 {
            eprintln!();
        }
        self.state.status = Status::Failed;
        self.state.error = format!("{error:#}");
        self.log_line(&format!("=== failed: {error:#}"));
        self.write_state();
    }

    pub fn succeed(&mut self) {
        self.state.status = Status::Succeeded;
        self.state.stage_progress = 1.0;
        self.log_line("=== succeeded");
        self.write_state();
    }

    fn write_state(&mut self) {
        self.state.updated_unix_ms = unix_ms();
        self.last_state_write = Instant::now();
        let json = serde_json::to_vec_pretty(&self.state).expect("state serialises");
        if let Err(e) = write_atomic(&self.dir.join("state.json"), &json) {
            eprintln!("warning: cannot write state.json: {e:#}");
        }
    }
}

fn lock(dir: &Path) -> anyhow::Result<File> {
    let path = dir.join(".lock");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            let holder = std::fs::read_to_string(&path).unwrap_or_default();
            bail!(
                "{} is being run by another worker ({})",
                dir.display(),
                holder.trim()
            );
        }
        Err(std::fs::TryLockError::Error(e)) => {
            return Err(e).with_context(|| format!("locking {}", path.display()));
        }
    }
    file.set_len(0)?;
    writeln!(file, "host {} pid {}", hostname(), std::process::id())?;
    Ok(file)
}

/// Write via a temporary file and a rename, so a reader never sees half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

pub fn format_seconds(s: f64) -> String {
    match s {
        s if s < 60.0 => format!("{s:.1} s"),
        s if s < 3600.0 => format!("{}:{:02} min", (s / 60.0) as u64, (s % 60.0) as u64),
        s => format!(
            "{}:{:02} h",
            (s / 3600.0) as u64,
            ((s % 3600.0) / 60.0) as u64
        ),
    }
}
