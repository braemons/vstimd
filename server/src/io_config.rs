use crate::scene::SceneConfig;
use crate::vtl_state::VtlConfig;

pub const CONFIG_VERSION: u32 = 2;

/// Reserved config name for the auto-saved last-session slot. Written on quit
/// when `[startup] save_on_quit` is set, and loaded at boot when
/// `[startup] load_config = "last"`. Leading underscore keeps it distinct from
/// user-chosen names.
pub const LAST_SESSION_CONFIG: &str = "_last_session";

/// Path to the file backing a named config inside `dir`. Named configs live at
/// `<dir>/vstimd_<name>.config.json`; this is the one place that layout is
/// defined.
pub fn config_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join(format!("vstimd_{name}.config.json"))
}

/// Borrowed view of I/O config assembled at save time — never stored.
#[derive(serde::Serialize)]
pub struct IoConfigRef<'a> {
    pub vtl: &'a VtlConfig,
}

/// Owned I/O config populated at load time — each field moved to its subsystem owner.
#[derive(serde::Deserialize, Default)]
pub struct IoConfigFile {
    #[serde(default)]
    pub vtl: VtlConfig,
}

/// Borrowed top-level view — used only during save. No allocation or copies.
#[derive(serde::Serialize)]
struct ConfigFileRef<'a> {
    version: u32,
    scene:   &'a SceneConfig,
    io:      IoConfigRef<'a>,
}

/// Owned top-level struct — used only during load. Fields are moved to their
/// owners. `version` is validated separately via `VersionProbe`, so it is not
/// repeated here (unknown JSON keys are ignored).
#[derive(serde::Deserialize)]
struct ConfigFile {
    scene: SceneConfig,
    io:    IoConfigFile,
}

/// Serialize current state to pretty JSON without touching the filesystem.
pub fn retrieve_config_json(scene: &SceneConfig, vtl: &VtlConfig) -> anyhow::Result<String> {
    let view = ConfigFileRef {
        version: CONFIG_VERSION,
        scene,
        io: IoConfigRef { vtl },
    };
    Ok(serde_json::to_string_pretty(&view)?)
}

/// Write scene + I/O config to a file.
pub fn save_config(scene: &SceneConfig, vtl: &VtlConfig, path: &std::path::Path) -> anyhow::Result<()> {
    let json = retrieve_config_json(scene, vtl)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Parse and validate a config JSON string without touching the filesystem.
/// Used by both `load_config` and `UploadConfig` validation.
pub fn parse_config_json(s: &str) -> anyhow::Result<(SceneConfig, IoConfigFile)> {
    // Check the version before the full parse so an older-format file fails with
    // a clear version error rather than a confusing deserialization error.
    #[derive(serde::Deserialize)]
    struct VersionProbe {
        version: u32,
    }
    let probe: VersionProbe = serde_json::from_str(s)?;
    anyhow::ensure!(
        probe.version == CONFIG_VERSION,
        "Unsupported config version {} (expected {})",
        probe.version,
        CONFIG_VERSION
    );
    let f: ConfigFile = serde_json::from_str(s)?;
    Ok((f.scene, f.io))
}

/// Read a config file from disk and parse it.
pub fn load_config(path: &std::path::Path) -> anyhow::Result<(SceneConfig, IoConfigFile)> {
    let s = std::fs::read_to_string(path)?;
    parse_config_json(&s)
}

// ── Demo configs ──────────────────────────────────────────────────────────────

/// Every demo name starts with this, so demos are visible as a group in
/// `config list` and cannot be confused with user-saved configs.
pub const DEMO_PREFIX: &str = "demo_";

/// Demos shipped with the server: `(name, config JSON)`.
///
/// Demos are deliberately *ordinary* configs — there is no demo-specific
/// command, load path or scene builder. They are compiled in (rather than
/// installed as package data) so a dev checkout, a `.deb` install and the
/// Raspberry Pi image all offer the same set, and are written into the config
/// dir at startup by [`seed_demo_configs`]. From that point on a demo is just a
/// file: `config load` it, edit the scene, `config save` it under your own name.
///
/// The trigger lines they use are the ones the Raspberry Pi 5 gpiochip-daqd
/// example wires to physical header pins (`in_pin11`, `out_pin36`, …), so a
/// demo does something measurable on a stock Pi 5 rig with no extra config.
pub const DEMO_CONFIGS: &[(&str, &str)] = &[
    (
        "demo_first_light",
        include_str!("../config/demos/vstimd_demo_first_light.config.json"),
    ),
    (
        "demo_drifting_grating",
        include_str!("../config/demos/vstimd_demo_drifting_grating.config.json"),
    ),
    (
        "demo_gratings_triggered",
        include_str!("../config/demos/vstimd_demo_gratings_triggered.config.json"),
    ),
    (
        "demo_moving_target",
        include_str!("../config/demos/vstimd_demo_moving_target.config.json"),
    ),
    (
        "demo_photodiode_flicker",
        include_str!("../config/demos/vstimd_demo_photodiode_flicker.config.json"),
    ),
    (
        "demo_trigger_gate",
        include_str!("../config/demos/vstimd_demo_trigger_gate.config.json"),
    ),
];

/// Write every [`DEMO_CONFIGS`] entry that is not already present in `dir`.
///
/// Existing files are never touched: an operator who edited (or deleted and
/// re-saved) a demo keeps their version, and a demo the operator deleted comes
/// back on the next start — deleting it for good means saving something else
/// under that name. Returns the names actually written; individual write errors
/// are collected as `Err` names rather than aborting the rest, because failing
/// to seed a demo must never stop the server from starting.
pub fn seed_demo_configs(dir: &std::path::Path) -> (Vec<&'static str>, Vec<(&'static str, std::io::Error)>) {
    let mut written = vec![];
    let mut failed = vec![];
    for (name, json) in DEMO_CONFIGS {
        let path = config_path(dir, name);
        if path.exists() {
            continue;
        }
        match std::fs::write(&path, json) {
            Ok(()) => written.push(*name),
            Err(e) => failed.push((*name, e)),
        }
    }
    (written, failed)
}

/// List bare config names (no path, no extension) from a config directory.
pub fn list_config_names(dir: &std::path::Path) -> anyhow::Result<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(prefixed) = name.strip_suffix(".config.json")
            && let Some(bare) = prefixed.strip_prefix("vstimd_") {
                names.push(bare.to_string());
            }
    }
    names.sort();
    Ok(names)
}

// ── Startup config directory ──────────────────────────────────────────────────

/// Default config directory for a deployed rig. Matches the packaged systemd
/// unit (`StateDirectory=braemons/vstimd`). Used when `--config-dir` is not
/// given; falls back to a home directory if this is not writable.
pub const DEFAULT_CONFIG_DIR: &str = "/var/lib/braemons/vstimd";

/// True if `dir` exists (creating it if needed) and a file can be created in
/// it. Used to pick a writable config dir at startup — a rig running as a
/// non-root/dev user often cannot write under `/var`.
pub fn dir_is_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".vstimd-write-test");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// The first writable directory from `candidates`, or the current directory if
/// none are writable (last-resort so the server still starts).
pub fn first_writable_dir(candidates: &[std::path::PathBuf]) -> std::path::PathBuf {
    candidates
        .iter()
        .find(|d| dir_is_writable(d))
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

// ── Quit-time archive configs ──────────────────────────────────────────────────

/// Warn past this many timestamped archives in one config dir — they are never
/// pruned automatically, so this nudges an operator to clean up.
pub const ARCHIVE_WARN_THRESHOLD: usize = 500;

/// Filesystem- and sort-safe UTC timestamp name for a quit-time archive config,
/// e.g. `20260706T045805Z`. Read from the system clock; see
/// [`format_utc_compact`] for the pure formatter.
pub fn archive_timestamp_name() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_utc_compact(secs)
}

/// Format a UNIX timestamp (seconds since the epoch, UTC) as `YYYYMMDDTHHMMSSZ`.
/// Dependency-free and pure, so it is directly testable.
///
/// NOTE: this is deliberately hand-rolled because the std library has no
/// calendar/datetime support and one filename string doesn't justify a crate.
/// If we grow more time needs — parsing, time zones, local time, richer
/// formatting — replace this (and [`civil_from_days`]) with a real datetime
/// crate (`jiff` is already in the tree transitively via `env_logger`) rather
/// than extending the arithmetic here.
fn format_utc_compact(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs = unix_secs.rem_euclid(86_400);
    let (hh, mm, ss) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// Convert days since 1970-01-01 to a `(year, month, day)` civil date, using
/// Howard Hinnant's public-domain `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (y + if m <= 2 { 1 } else { 0 }, m as u32, d)
}

/// True if `name` looks like an [`archive_timestamp_name`] (`YYYYMMDDTHHMMSSZ`).
/// Used to count archives without confusing them with user-named configs or the
/// last-session slot.
pub fn is_archive_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 16
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'T'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'Z'
}

/// Count timestamped archive configs in `dir` (ignores named configs and the
/// last-session slot). Returns 0 on any read error.
pub fn count_archive_configs(dir: &std::path::Path) -> usize {
    list_config_names(dir)
        .map(|names| names.iter().filter(|n| is_archive_name(n)).count())
        .unwrap_or(0)
}

pub fn is_io_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>().is_some()
}

pub fn is_not_found(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>()
        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
}

pub fn is_format_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<serde_json::Error>().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_timestamp_formatting() {
        // Well-known UNIX epoch anchors.
        assert_eq!(format_utc_compact(0), "19700101T000000Z");
        assert_eq!(format_utc_compact(946_684_800), "20000101T000000Z");
        // The "billennium": 2001-09-09 01:46:40 UTC.
        assert_eq!(format_utc_compact(1_000_000_000), "20010909T014640Z");
    }

    #[test]
    fn archive_names_are_recognised() {
        assert!(is_archive_name("20260706T045805Z"));
        assert!(is_archive_name(&format_utc_compact(1_000_000_000)));
        // A real generated name round-trips through the recogniser.
        assert!(is_archive_name(&archive_timestamp_name()));

        // Not archives: named configs, the last-session slot, malformed stamps.
        assert!(!is_archive_name("center_target"));
        assert!(!is_archive_name(LAST_SESSION_CONFIG));
        assert!(!is_archive_name("20260706T045805")); // missing trailing Z
        assert!(!is_archive_name("2026070aT045805Z")); // non-digit
        assert!(!is_archive_name("20260706X045805Z")); // wrong separator
    }
}
