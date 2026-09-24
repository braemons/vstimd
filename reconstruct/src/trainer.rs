//! Splat trainers, behind one trait so a second backend (gsplat) is a command
//! line and a progress reader, not a new stage.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use crate::params::{JobParams, TrainerKind};
use crate::tools::Tools;

pub trait Trainer {
    fn name(&self) -> &'static str;

    /// The command that trains on the COLMAP dataset in `dataset` (an
    /// undistorted model with `images/` and `sparse/`), writing into `out_dir`.
    fn command(&self, dataset: &Path, params: &JobParams, out_dir: &Path) -> Command;

    /// Training progress in `[0, 1]` from what is in `out_dir`, if it shows any.
    fn progress(&self, out_dir: &Path, params: &JobParams) -> Option<f32>;

    /// The finished scene in `out_dir`.
    fn result(&self, out_dir: &Path, params: &JobParams) -> anyhow::Result<PathBuf>;
}

pub fn trainer(kind: TrainerKind, tools: &Tools) -> anyhow::Result<Box<dyn Trainer>> {
    match kind {
        TrainerKind::Brush => Ok(Box::new(Brush {
            binary: tools.brush()?.to_path_buf(),
        })),
    }
}

/// [Brush](https://github.com/ArthurBrussee/brush), tested with 0.3.
///
/// Its CLI draws an indicatif bar only on a terminal, so progress comes from
/// its checkpoints instead: it exports every tenth of the run as
/// `export_<step>.ply`, and all but the newest are deleted as they appear.
pub struct Brush {
    pub binary: PathBuf,
}

const EXPORT_PREFIX: &str = "export_";

impl Brush {
    /// Finished exports in `out_dir`, by step.
    fn exports(out_dir: &Path) -> Vec<(u32, PathBuf)> {
        let Ok(entries) = std::fs::read_dir(out_dir) else {
            return Vec::new();
        };
        let mut found: Vec<(u32, PathBuf)> = entries
            .filter_map(|e| {
                let path = e.ok()?.path();
                let step = path
                    .file_name()?
                    .to_str()?
                    .strip_prefix(EXPORT_PREFIX)?
                    .strip_suffix(".ply")?
                    .parse()
                    .ok()?;
                Some((step, path))
            })
            .collect();
        found.sort();
        found
    }
}

impl Trainer for Brush {
    fn name(&self) -> &'static str {
        "brush"
    }

    fn command(&self, dataset: &Path, params: &JobParams, out_dir: &Path) -> Command {
        let mut cmd = Command::new(&self.binary);
        let every = (params.train_steps / 10).max(1);
        cmd.arg(dataset)
            .arg("--total-steps")
            .arg(params.train_steps.to_string())
            .arg("--sh-degree")
            .arg(params.sh_degree.to_string())
            .arg("--max-resolution")
            .arg(params.max_image_px.to_string())
            .arg("--export-every")
            .arg(every.to_string())
            .arg("--export-path")
            .arg(out_dir)
            .arg("--export-name")
            .arg(format!("{EXPORT_PREFIX}{{iter}}.ply"));
        cmd
    }

    fn progress(&self, out_dir: &Path, params: &JobParams) -> Option<f32> {
        let exports = Self::exports(out_dir);
        let (newest, _) = exports.last()?;
        // The newest may still be being written; the older ones are complete.
        for (step, path) in &exports {
            if step < newest {
                let _ = std::fs::remove_file(path);
            }
        }
        Some(*newest as f32 / params.train_steps as f32)
    }

    fn result(&self, out_dir: &Path, params: &JobParams) -> anyhow::Result<PathBuf> {
        Self::exports(out_dir)
            .into_iter()
            .rev()
            .find(|(step, _)| *step >= params.train_steps)
            .map(|(_, p)| p)
            .with_context(|| {
                format!(
                    "Brush exited without writing its step-{} export into {}",
                    params.train_steps,
                    out_dir.display()
                )
            })
    }
}
