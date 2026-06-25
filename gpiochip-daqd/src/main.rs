mod bridge;
mod config;

use std::fs;

use anyhow::{Context, Result};
use log::{error, info, warn};
use vtl::VtlOwner;

const DEFAULT_CONFIG_PATH: &str = "/etc/braemons/gpiochip-daqd-config.toml";
const EXAMPLES_DIR: &str = "/usr/share/braemons/gpiochip-daqd/";
const VTL_OPEN_ATTEMPTS: u32 = 30;

struct Args {
    config: String,
    standalone: bool,
    verbose: bool,
}

fn parse_args() -> Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut config = DEFAULT_CONFIG_PATH.to_string();
    let mut standalone = false;
    let mut verbose = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--standalone" => standalone = true,
            "-v" | "--verbose" => verbose = true,
            "-c" | "--config" => {
                config = args.next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a path argument"))?;
            }
            other => anyhow::bail!("unknown argument: {other}\nUsage: gpiochip-daqd [-c <config>] [--standalone] [-v]"),
        }
    }

    Ok(Args { config, standalone, verbose })
}

fn main() -> Result<()> {
    let Args { config: config_path, standalone, verbose } = parse_args()?;

    let default_level = if verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level)).init();

    let raw = match fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            error!(
                "config not found: {config_path}\n\
                 Board-specific examples are in {EXAMPLES_DIR}\n\
                 Copy one to get started:\n\
                 \tcp {EXAMPLES_DIR}jetson-orin-nano.toml {config_path}"
            );
            std::process::exit(1);
        }
        Err(e) => return Err(e).with_context(|| format!("read config {config_path}")),
    };
    let cfg: config::Config =
        toml::from_str(&raw).with_context(|| format!("parse config {config_path}"))?;

    check_gpio_chip(&cfg.gpio.chip)?;

    info!(
        "gpiochip-daqd: VTL={} chip={}  ({} output(s), {} input(s)){}",
        cfg.vtl.shm_name,
        cfg.gpio.chip,
        cfg.outputs.len(),
        cfg.inputs.len(),
        if standalone { "  [standalone]" } else { "" },
    );
    for o in &cfg.outputs {
        info!(
            "  out  {:>3} '{}' → VTL bank={} bit={}",
            o.gpio_line, o.name, o.vtl_bank, o.vtl_bit
        );
    }
    for i in &cfg.inputs {
        info!(
            "  in   {:>3} '{}' → VTL bank={} bit={} edge={:?}",
            i.gpio_line, i.name, i.vtl_bank, i.vtl_bit, i.edge
        );
    }
    if cfg.scheduling.output_cpu_core.is_some() || cfg.scheduling.input_cpu_core.is_some() {
        warn!("scheduling.output_cpu_core / input_cpu_core: parsed but CPU affinity is not yet implemented");
    }

    let config::Config { vtl: vtl_cfg, gpio, outputs, inputs, scheduling: _ } = cfg;

    // In standalone mode create the VTL segment ourselves so gpiochip-daqd
    // can run without vstimd (e.g. for GPIO loopback testing).
    // _owner must stay alive until main returns — dropping it unlinks the shm.
    let _owner: Option<VtlOwner> = if standalone {
        let banks = required_banks_from_config(&outputs, &inputs);
        let owner = VtlOwner::create(&vtl_cfg.shm_name, banks, banks)
            .with_context(|| format!("create VTL segment '{}' in standalone mode", vtl_cfg.shm_name))?;
        info!("standalone: created VTL segment '{}' ({banks} bank(s))", vtl_cfg.shm_name);
        Some(owner)
    } else {
        None
    };

    let vtl = if standalone {
        // The segment now exists; open a client view for the output loop.
        vtl::VtlClient::open(&vtl_cfg.shm_name)
            .context("open VTL client for standalone segment")?
    } else {
        bridge::open_vtl_with_retry(&vtl_cfg.shm_name, VTL_OPEN_ATTEMPTS)?
    };

    // Spawn one blocking thread per input line; each gets its own VtlClient.
    let mut _watchers = Vec::new();
    for inp in inputs {
        let client = vtl::VtlClient::open(&vtl_cfg.shm_name)
            .context("open VTL client for input watcher")?;
        _watchers.push(bridge::spawn_input_watcher(gpio.chip.clone(), inp, client));
    }

    #[cfg(target_os = "linux")]
    sd_notify::notify(false, &[sd_notify::NotifyState::Ready])?;

    // Output loop on the main thread — blocks on the VTL output semaphore
    // instead of polling, giving ~50µs response latency (SCHED_FIFO priority).
    if let Err(e) = bridge::run_output_loop(&gpio.chip, &outputs, &vtl) {
        error!("output loop error: {e:#}");
        return Err(e);
    }

    Ok(())
}

/// Derive the minimum number of VTL banks needed to cover all configured lines.
fn required_banks_from_config(outputs: &[config::OutputLine], inputs: &[config::InputLine]) -> u32 {
    let max_out = outputs.iter().map(|o| o.vtl_bank).max().unwrap_or(0);
    let max_in  = inputs.iter().map(|i| i.vtl_bank).max().unwrap_or(0);
    (max_out.max(max_in) as u32) + 1
}

/// Check that the GPIO chip device exists and is a character device.
///
/// Exits with a clear diagnostic rather than letting gpio-cdev produce an
/// opaque OS error at the point of first use.
fn check_gpio_chip(chip_path: &str) -> Result<()> {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::metadata(chip_path) {
        Ok(meta) if meta.file_type().is_char_device() => Ok(()),
        Ok(_) => {
            anyhow::bail!(
                "'{chip_path}' exists but is not a character device — \
                 check your [gpio] chip setting"
            )
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let available = list_gpio_chips();
            let hint = if available.is_empty() {
                "no /dev/gpiochip* devices found on this system".to_string()
            } else {
                format!("available GPIO chips: {}", available.join(", "))
            };
            error!(
                "GPIO chip '{chip_path}' not found — is this the correct board?\n\
                 {hint}\n\
                 Board-specific configs are in {EXAMPLES_DIR}"
            );
            std::process::exit(1);
        }
        Err(e) => Err(e).with_context(|| format!("check GPIO chip '{chip_path}'")),
    }
}

fn list_gpio_chips() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/dev") else { return vec![] };
    let mut chips: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.starts_with("gpiochip") { Some(format!("/dev/{name}")) } else { None }
        })
        .collect();
    chips.sort();
    chips
}
