//! Align a COLMAP text model on its own and print the result as JSON — for
//! trying the alignment on a real capture without running a job.
//!
//! `cargo run --release -p vstimd-reconstruct --example align_model -- <sparse_txt_dir> <path_length_cm> [capture_height_cm]`

use reconstruct::align;
use reconstruct::colmap::Model;
use reconstruct::params::{Alignment, Crop};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, length, rest @ ..] = args.as_slice() else {
        anyhow::bail!("usage: align_model <sparse_txt_dir> <path_length_cm> [capture_height_cm]");
    };
    let model = Model::read_text(dir.as_ref())?;
    let params = Alignment {
        capture_path_length_cm: length.parse()?,
        capture_height_cm: rest.first().map(|h| h.parse()).transpose()?,
    };
    let a = align::estimate(&model, &params, &Crop::default())?;
    println!("{}", serde_json::to_string_pretty(&a)?);
    Ok(())
}
