//! What a job was asked to do: the contents of `job.toml`.
//!
//! The field names are those of `ReconstructionParams` in the design
//! (`dev/design/SPLAT_RECONSTRUCTION_PLAN.md` §5.4), so the server can write the
//! same file when it takes jobs over. A field left out means "derive it"; TOML
//! has no null, so those are `Option`s rather than the protocol's zeros.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_IMAGE_PX: u32 = 1600;
pub const DEFAULT_TRAIN_STEPS: u32 = 30_000;
pub const DEFAULT_MAX_SPLATS: u32 = 1_000_000;
pub const DEFAULT_MARGIN_CM: f32 = 50.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum CaptureOrder {
    /// Photos taken walking, in file-name order: each is matched with its neighbours.
    #[default]
    Sequential,
    /// Photos in no particular order: every pair is matched (slow beyond a few hundred).
    Unordered,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Mapper {
    /// COLMAP's global mapper (formerly GLOMAP): fast; falls back to incremental
    /// when the installed COLMAP has none.
    #[default]
    Global,
    /// COLMAP's incremental mapper: slower, more robust on hard captures.
    Incremental,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TrainerKind {
    #[default]
    Brush,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alignment {
    /// The distance walked from the first photo to the last. Gives the scene its scale.
    pub capture_path_length_cm: f32,
    /// Camera height above the floor while capturing. Cross-checks the floor
    /// fit, and stands in for it when no floor is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_height_cm: Option<f32>,
}

/// A box in the aligned frame; see [`crate::align::CropBox`]. Unset extents
/// come from the capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_cm: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_cm: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_cm: Option<f32>,
    pub margin_cm: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Self {
            width_cm: None,
            height_cm: None,
            length_cm: None,
            margin_cm: DEFAULT_MARGIN_CM,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobParams {
    /// Every PNG and JPEG directly inside, in natural file-name order.
    pub images_dir: PathBuf,
    /// `.ply` or `.splat`.
    pub output: PathBuf,
    pub max_image_px: u32,
    pub order: CaptureOrder,
    pub mapper: Mapper,
    pub trainer: TrainerKind,
    pub train_steps: u32,
    pub max_splats: u32,
    pub sh_degree: u32,
    pub keep_work: bool,
    pub alignment: Alignment,
    pub crop: Crop,
}

impl JobParams {
    pub fn validate(&self) -> anyhow::Result<()> {
        let a = &self.alignment;
        if !(a.capture_path_length_cm.is_finite() && a.capture_path_length_cm > 0.0) {
            bail!(
                "capture_path_length_cm must be > 0: a reconstruction has no scale of its own, \
                 so measure the distance walked from the first photo to the last"
            );
        }
        let positive = |name: &str, v: Option<f32>| -> anyhow::Result<()> {
            match v {
                Some(v) if !(v.is_finite() && v > 0.0) => bail!("{name} must be > 0, got {v}"),
                _ => Ok(()),
            }
        };
        positive("capture_height_cm", a.capture_height_cm)?;
        positive("crop.width_cm", self.crop.width_cm)?;
        positive("crop.height_cm", self.crop.height_cm)?;
        positive("crop.length_cm", self.crop.length_cm)?;
        if !(self.crop.margin_cm.is_finite() && self.crop.margin_cm >= 0.0) {
            bail!("crop.margin_cm must be ≥ 0");
        }
        if self.sh_degree > 1 {
            bail!(
                "sh_degree {} is not supported yet: aligning a scene rotates its colour, which is \
                 implemented for degrees 0 and 1. vstimd draws degree 0 only",
                self.sh_degree
            );
        }
        if self.max_image_px < 64 {
            bail!("max_image_px must be at least 64");
        }
        if self.train_steps == 0 || self.max_splats == 0 {
            bail!("train_steps and max_splats must be > 0");
        }
        match output_format(&self.output) {
            Some(_) => Ok(()),
            None => bail!(
                "--out must end in .ply or .splat: {}",
                self.output.display()
            ),
        }
    }

    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("job parameters always serialise")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Ply,
    Splat,
}

pub fn output_format(path: &Path) -> Option<OutputFormat> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "ply" => Some(OutputFormat::Ply),
        "splat" => Some(OutputFormat::Splat),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> JobParams {
        JobParams {
            images_dir: "/data/corridor".into(),
            output: "/data/corridor.ply".into(),
            max_image_px: DEFAULT_MAX_IMAGE_PX,
            order: CaptureOrder::Sequential,
            mapper: Mapper::Global,
            trainer: TrainerKind::Brush,
            train_steps: DEFAULT_TRAIN_STEPS,
            max_splats: DEFAULT_MAX_SPLATS,
            sh_degree: 0,
            keep_work: false,
            alignment: Alignment {
                capture_path_length_cm: 500.0,
                capture_height_cm: None,
            },
            crop: Crop {
                width_cm: Some(120.0),
                ..Crop::default()
            },
        }
    }

    #[test]
    fn toml_round_trips() {
        let p = params();
        let text = p.to_toml();
        assert!(text.contains("capture_path_length_cm = 500.0"), "{text}");
        assert!(
            !text.contains("capture_height_cm"),
            "unset fields stay out: {text}"
        );
        assert_eq!(toml::from_str::<JobParams>(&text).unwrap(), p);
    }

    #[test]
    fn refuses_what_cannot_work() {
        let mut p = params();
        p.alignment.capture_path_length_cm = 0.0;
        assert!(
            p.validate()
                .unwrap_err()
                .to_string()
                .contains("distance walked")
        );
        let mut p = params();
        p.sh_degree = 3;
        assert!(p.validate().is_err());
        let mut p = params();
        p.output = "/data/corridor.obj".into();
        assert!(p.validate().is_err());
        assert!(params().validate().is_ok());
    }
}
