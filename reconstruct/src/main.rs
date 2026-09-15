use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

use reconstruct::job::{Job, Stage, format_seconds};
use reconstruct::params::{
    Alignment, CaptureOrder, Crop, DEFAULT_MARGIN_CM, DEFAULT_MAX_IMAGE_PX, DEFAULT_MAX_SPLATS,
    DEFAULT_TRAIN_STEPS, JobParams, Mapper, TrainerKind,
};
use reconstruct::stages::{self, Report};
use reconstruct::tools::{self, Tools};

/// Turn photos of a corridor into a Gaussian splat scene for vstimd.
///
/// Capture: walk the corridor once, start to end, holding the camera upright at
/// the height the virtual camera will use, looking forward, with fixed exposure.
/// Measure the distance walked. The result is in centimetres with the floor at
/// y = 0 and the corridor running down −Z from the first photo, so it loads
/// with an identity transform.
#[derive(Parser)]
#[command(name = "vstimd-reconstruct", version = option_env!("VSTIMD_VERSION").unwrap_or("dev"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Reconstruct a folder of photos. Run it again with the same arguments to resume.
    Run(RunArgs),
    /// Continue a job from its directory.
    Resume {
        /// The job directory (holds job.toml).
        job_dir: PathBuf,
        /// Rerun this stage and every later one, e.g. `finish` to rewrite the
        /// output from the trained scene.
        #[arg(long, value_enum)]
        from: Option<Stage>,
        #[command(flatten)]
        common: CommonArgs,
    },
    /// Report which tools are usable, as JSON. Exits non-zero when something is missing.
    Check {
        #[command(flatten)]
        tools: ToolArgs,
    },
}

#[derive(Args)]
struct ToolArgs {
    /// COLMAP binary [default: $VSTIMD_COLMAP, else `colmap` on PATH].
    #[arg(long)]
    colmap: Option<PathBuf>,
    /// Brush binary [default: $VSTIMD_BRUSH, else `brush_app` or `brush` on PATH].
    #[arg(long)]
    brush: Option<PathBuf>,
}

impl ToolArgs {
    fn find(self) -> Tools {
        Tools::find(self.colmap, self.brush)
    }
}

#[derive(Args)]
struct CommonArgs {
    #[command(flatten)]
    tools: ToolArgs,
    /// Echo the tools' output as well as writing it to log.txt.
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Args)]
struct RunArgs {
    /// Folder of JPEG or PNG photos, in capture order by file name.
    images_dir: PathBuf,
    /// The scene to write: .ply (every viewer reads it) or .splat.
    #[arg(long)]
    out: PathBuf,
    /// Distance walked from the first photo to the last. Required: it is the scene's scale.
    #[arg(long, alias = "path-length-cm")]
    capture_path_length_cm: f32,
    /// Camera height above the floor while capturing: checks the floor fit, and
    /// replaces it when no floor is found.
    #[arg(long)]
    capture_height_cm: Option<f32>,
    /// Job directory [default: <out>.reconstruction].
    #[arg(long)]
    work: Option<PathBuf>,
    /// Long edge of the images used for training.
    #[arg(long, default_value_t = DEFAULT_MAX_IMAGE_PX)]
    max_image_px: u32,
    #[arg(long, value_enum, default_value_t = CaptureOrder::Sequential)]
    order: CaptureOrder,
    #[arg(long, value_enum, default_value_t = Mapper::Global)]
    mapper: Mapper,
    #[arg(long, value_enum, default_value_t = TrainerKind::Brush)]
    trainer: TrainerKind,
    #[arg(long, default_value_t = DEFAULT_TRAIN_STEPS)]
    train_steps: u32,
    /// Most splats to keep after cropping; the least visible go first.
    #[arg(long, default_value_t = DEFAULT_MAX_SPLATS)]
    max_splats: u32,
    /// Spherical-harmonic degree to train and keep (0 or 1). vstimd draws degree 0.
    #[arg(long, default_value_t = 0)]
    sh_degree: u32,
    /// Crop width, centred on the walked line [default: from the sparse points].
    #[arg(long)]
    crop_width_cm: Option<f32>,
    /// Crop height above the floor [default: from the sparse points].
    #[arg(long)]
    crop_height_cm: Option<f32>,
    /// Crop length down the corridor [default: the walked distance].
    #[arg(long)]
    crop_length_cm: Option<f32>,
    /// Added on every side of the crop box.
    #[arg(long, default_value_t = DEFAULT_MARGIN_CM)]
    crop_margin_cm: f32,
    /// Keep work/ (COLMAP database, undistorted images) after success.
    #[arg(long)]
    keep_work: bool,
    #[command(flatten)]
    common: CommonArgs,
}

fn absolute(path: &Path) -> anyhow::Result<PathBuf> {
    std::path::absolute(path).with_context(|| format!("resolving {}", path.display()))
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Commands::Check { tools } => {
            let report = tools::check(&tools.find());
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(if report.ready {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Commands::Run(a) => {
            let output = absolute(&a.out)?;
            let dir = match &a.work {
                Some(w) => absolute(w)?,
                None => {
                    let mut name = output.clone().into_os_string();
                    name.push(".reconstruction");
                    PathBuf::from(name)
                }
            };
            let params = JobParams {
                images_dir: absolute(&a.images_dir)?,
                output,
                max_image_px: a.max_image_px,
                order: a.order,
                mapper: a.mapper,
                trainer: a.trainer,
                train_steps: a.train_steps,
                max_splats: a.max_splats,
                sh_degree: a.sh_degree,
                keep_work: a.keep_work,
                alignment: Alignment {
                    capture_path_length_cm: a.capture_path_length_cm,
                    capture_height_cm: a.capture_height_cm,
                },
                crop: Crop {
                    width_cm: a.crop_width_cm,
                    height_cm: a.crop_height_cm,
                    length_cm: a.crop_length_cm,
                    margin_cm: a.crop_margin_cm,
                },
            };
            let mut job = Job::create_or_open(&dir, params)?;
            execute(&mut job, a.common)
        }
        Commands::Resume {
            job_dir,
            from,
            common,
        } => {
            let mut job = Job::open(&absolute(&job_dir)?)?;
            if let Some(stage) = from {
                job.clear_from(stage)?;
            }
            execute(&mut job, common)
        }
    }
}

fn execute(job: &mut Job, common: CommonArgs) -> anyhow::Result<ExitCode> {
    job.verbose = common.verbose;
    let tools = common.tools.find();
    eprintln!("job {}", job.dir.display());
    let report = stages::run(job, &tools)?;
    print_summary(&report);
    Ok(ExitCode::SUCCESS)
}

fn print_summary(r: &Report) {
    let a = &r.alignment;
    let total: f64 = r.stage_seconds.values().filter_map(|v| v.as_f64()).sum();
    eprintln!();
    eprintln!(
        "wrote {} ({} splats) in {}",
        r.output.display(),
        r.splats_written,
        format_seconds(total)
    );
    eprintln!(
        "  load it with an identity transform; the corridor starts at the origin and runs down −Z"
    );
    eprintln!(
        "  camera height while capturing: {:.0} cm — the scene is sharp only near it",
        a.capture_height_cm
    );
    eprintln!(
        "  walked path: {:.0} cm (use it as LinearNav3D track_length_cm)",
        a.path_length_cm
    );
    for w in &a.warnings {
        eprintln!("  warning: {w}");
    }
}
