mod bridge;
mod config;

use std::{fs, time::Duration};

use anyhow::{Context, Result};
use log::{error, info};

const CONFIG_PATH: &str = "/etc/braemons/jetsond-daqd/jetsond-daqd.toml";
const OUTPUT_POLL_INTERVAL: Duration = Duration::from_millis(1);
const VTL_OPEN_ATTEMPTS: u32 = 30;

fn main() -> Result<()> {
    env_logger::init();

    let raw = fs::read_to_string(CONFIG_PATH)
        .with_context(|| format!("read config {CONFIG_PATH}"))?;
    let cfg: config::Config =
        toml::from_str(&raw).with_context(|| format!("parse config {CONFIG_PATH}"))?;

    info!(
        "jetsond-daqd starting: {} outputs, {} inputs",
        cfg.outputs.len(),
        cfg.inputs.len()
    );

    let config::Config { vtl: vtl_cfg, gpio, outputs, inputs } = cfg;

    let vtl = bridge::open_vtl_with_retry(&vtl_cfg.shm_name, VTL_OPEN_ATTEMPTS)?;

    // Spawn one blocking thread per input line; each gets its own VtlClient.
    let mut _watchers = Vec::new();
    for inp in inputs {
        let client = vtl::VtlClient::open(&vtl_cfg.shm_name)
            .context("open VTL client for input watcher")?;
        _watchers.push(bridge::spawn_input_watcher(gpio.chip.clone(), inp, client));
    }

    #[cfg(target_os = "linux")]
    sd_notify::notify(false, &[sd_notify::NotifyState::Ready])?;

    // Output polling loop on the main thread — runs until GPIO error.
    if let Err(e) = bridge::run_output_loop(&gpio.chip, &outputs, &vtl, OUTPUT_POLL_INTERVAL) {
        error!("output loop error: {e:#}");
        return Err(e);
    }

    Ok(())
}
