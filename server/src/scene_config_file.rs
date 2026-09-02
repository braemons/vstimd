//! Scene-config files: the per-experiment configuration, saved as
//! `<storage-dir>/projects/<project>/scene-configs/<name>.config.json`. A
//! scene-config is a [`SceneConfig`] (stimuli, animations, background,
//! photodiode) plus the named VTL trigger lines. This module owns the file
//! format, the directory layout, the bundled demos and the quit-time archives.
//!
//! Not to be confused with [`crate::rig_config`], the server's other config:
//! that one describes the physical rig (TOML, `/etc`, changes when the
//! hardware does), this one describes an experiment (JSON, under
//! `--storage-dir`, changes per session).
//!
//! # Layout
//!
//! The storage dir holds **projects**, and a project holds **typed files**; a
//! scene-config is one of the types. Everything a study needs lives in one
//! directory, so copying, archiving or deleting a study is one operation:
//!
//! ```text
//! <storage-dir>/
//!   projects/
//!     <project>/
//!       scene-configs/    <name>.config.json
//!     default/            ← where an unqualified name lands
//!     demos/              ← seeded at startup
//!     _session/           ← the last-session slot and the quit-time archives
//! ```
//!
//! See `dev/design/ASSET_STORE_PLAN.md` for the full model, including the
//! asset types that join `scene-configs/` under a project.

use crate::scene::SceneConfig;
use crate::vtl_state::VtlConfig;

pub const CONFIG_VERSION: u32 = 5;

/// Reserved scene-config name for the auto-saved last-session slot. Written on
/// quit when `[startup] save_on_quit` is set, and loaded at boot when
/// `[startup] load_config = "last"`. Lives in the [`SESSION_PROJECT`], which is
/// per-rig rather than per-study, so it never clutters a real project.
pub const LAST_SESSION_CONFIG: &str = "_last_session";

// ── Projects and the directory layout ─────────────────────────────────────────

/// The one child of the storage dir. Every project is a directory below this, so
/// a single `--storage-dir` names the whole tree and the pieces can never be
/// pointed at unrelated places.
pub const PROJECTS_DIR: &str = "projects";

/// The scene-config type directory inside a project. Assets will grow siblings
/// (`images/`, `meshes/`, …); the type level is what scopes name collisions.
pub const SCENE_CONFIGS_DIR: &str = "scene-configs";

/// Where an unqualified scene-config name lands, and the active project at boot.
pub const DEFAULT_PROJECT: &str = "default";

/// The project holding the shipped demos, seeded at startup. Replaces the old
/// `demo_` name prefix: a demo that ships with images later needs no special
/// case, which a prefix scheme could never offer.
pub const DEMOS_PROJECT: &str = "demos";

/// The project holding the last-session slot and the timestamped quit archives.
/// Server-owned (leading underscore) and per-rig, not per-study.
pub const SESSION_PROJECT: &str = "_session";

/// True if `name` is a legal project or scene-config name: `[A-Za-z0-9._-]`,
/// 1-64 bytes. Rejects `..`, path separators, NUL and leading `-` by
/// construction rather than by sanitising, so a name can never escape the tree.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// True if `project` is server-owned and may not be created or deleted by a
/// client. Leading underscore is the marker; `default` and `demos` are ordinary
/// projects the server merely seeds.
pub fn is_reserved_project(project: &str) -> bool {
    project.starts_with('_')
}

/// A scene-config's two coordinates: the project holding it and its bare name.
/// This is what every load/save path takes, so no caller has to know the
/// on-disk layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneConfigRef {
    pub project: String,
    pub name:    String,
}

impl SceneConfigRef {
    /// Parse `[<project>/]<name>`, filling an unqualified name in with
    /// `default_project`. Both segments are validated, so a parsed ref is
    /// always safe to join onto the storage dir.
    pub fn parse(s: &str, default_project: &str) -> anyhow::Result<Self> {
        let (project, name) = match s.split_once('/') {
            Some((p, n)) => (p, n),
            None => (default_project, s),
        };
        anyhow::ensure!(is_valid_name(project), "invalid project name {project:?}");
        anyhow::ensure!(is_valid_name(name), "invalid scene-config name {name:?}");
        Ok(Self { project: project.to_string(), name: name.to_string() })
    }

    /// The wire/CLI spelling: bare inside `default`, `<project>/<name>`
    /// elsewhere, so the common case stays one word.
    pub fn qualified(&self) -> String {
        if self.project == DEFAULT_PROJECT {
            self.name.clone()
        } else {
            format!("{}/{}", self.project, self.name)
        }
    }
}

impl std::fmt::Display for SceneConfigRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.qualified())
    }
}

/// `<storage-dir>/projects`.
pub fn projects_dir(storage_dir: &std::path::Path) -> std::path::PathBuf {
    storage_dir.join(PROJECTS_DIR)
}

/// `<storage-dir>/projects/<project>`. The caller is responsible for having
/// validated `project` — [`SceneConfigRef::parse`] does.
pub fn project_dir(storage_dir: &std::path::Path, project: &str) -> std::path::PathBuf {
    projects_dir(storage_dir).join(project)
}

/// `<storage-dir>/projects/<project>/scene-configs`.
pub fn scene_config_dir(storage_dir: &std::path::Path, project: &str) -> std::path::PathBuf {
    project_dir(storage_dir, project).join(SCENE_CONFIGS_DIR)
}

/// Path to the file backing `r` under `storage_dir`. This is the one place the
/// scene-config layout is defined.
pub fn scene_config_path(storage_dir: &std::path::Path, r: &SceneConfigRef) -> std::path::PathBuf {
    scene_config_dir(storage_dir, &r.project).join(format!("{}.config.json", r.name))
}

/// The sections of a scene-config file other than the scene, borrowed at save
/// time — never stored. Serialized under the `io` key, which the on-disk
/// format has used since v1; the key is format, the type name is not.
#[derive(serde::Serialize)]
pub struct SectionsRef<'a> {
    pub vtl: &'a VtlConfig,
}

/// The same sections, owned, populated at load time — each field moved to its
/// subsystem owner.
#[derive(serde::Deserialize, Default)]
pub struct Sections {
    #[serde(default)]
    pub vtl: VtlConfig,
}

/// Borrowed top-level view — used only during save. No allocation or copies.
#[derive(serde::Serialize)]
struct ConfigFileRef<'a> {
    version: u32,
    scene:   &'a SceneConfig,
    io:      SectionsRef<'a>,
}

/// Owned top-level struct — used only during load. Fields are moved to their
/// owners. `version` is validated separately via `VersionProbe`, so it is not
/// repeated here (unknown JSON keys are ignored).
#[derive(serde::Deserialize)]
struct ConfigFile {
    scene: SceneConfig,
    io:    Sections,
}

/// Serialize current state to pretty JSON without touching the filesystem.
pub fn retrieve_config_json(scene: &SceneConfig, vtl: &VtlConfig) -> anyhow::Result<String> {
    let view = ConfigFileRef {
        version: CONFIG_VERSION,
        scene,
        io: SectionsRef { vtl },
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
/// Used by both `load_config` and `UploadSceneConfig` validation.
pub fn parse_config_json(s: &str) -> anyhow::Result<(SceneConfig, Sections)> {
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
    reject_stimuli_without_a_wire_type(&f.scene)?;
    Ok((f.scene, f.io))
}

/// Refuse a scene holding a stimulus this server cannot describe on the wire.
///
/// `StimulusBody` deserializes every arm it has, including `Mesh3d`, but the 3-D
/// types own no `StimulusType` wire value yet (`dev/3D_ROADMAP.md` §10.2 reserves
/// 20-29) and no query-params arm. A loaded 3-D stimulus would therefore sit in the
/// scene until the next `ListStimuli`, `QueryStimulus` or web snapshot walked it and
/// hit the `unimplemented!()` that refuses to report a 2-D type for it — killing the
/// thread that walked it.
///
/// Refusing the file is the honest end of that: the alternative is a scene the
/// server can hold but not answer questions about. It lifts on its own once Phase B
/// gives the 3-D types wire values, since this reads the same taxonomy `ipc/convert`
/// maps and asks it exactly what convert asks.
fn reject_stimuli_without_a_wire_type(scene: &SceneConfig) -> anyhow::Result<()> {
    for entry in scene.stimuli.values() {
        let stimulus_type = entry.stimulus.stimulus_type();
        anyhow::ensure!(
            !stimulus_type.is_3d(),
            "stimulus {:?} is a {}: 3-D stimuli have no wire representation yet \
             (dev/3D_ROADMAP.md §10.2), so this server cannot load them",
            entry.name(),
            stimulus_type.type_name(),
        );
    }
    Ok(())
}

/// Read a config file from disk and parse it.
pub fn load_config(path: &std::path::Path) -> anyhow::Result<(SceneConfig, Sections)> {
    let s = std::fs::read_to_string(path)?;
    parse_config_json(&s)
}

// ── Demo configs ──────────────────────────────────────────────────────────────

/// Demos shipped with the server: `(name, config JSON)`.
///
/// Demos are deliberately *ordinary* scene-configs — there is no demo-specific
/// command, load path or scene builder. They are compiled in (rather than
/// installed as package data) so a dev checkout, a `.deb` install and the
/// Raspberry Pi image all offer the same set, and are written into the
/// [`DEMOS_PROJECT`] at startup by [`seed_demo_configs`]. From that point on a
/// demo is just a file: `scene-config load demos/<name>` it, edit the scene,
/// `scene-config save` it under your own name.
///
/// The project directory is what keeps them visible as a group and distinct
/// from user-saved scene-configs; they carry no name prefix.
///
/// The trigger lines they use are the ones the Raspberry Pi 5 gpiochip-daqd
/// example wires to physical header pins (`in_pin11`, `out_pin36`, …), so a
/// demo does something measurable on a stock Pi 5 rig with no extra config.
pub const DEMO_CONFIGS: &[(&str, &str)] = &[
    ("first_light", include_str!("../config/demos/first_light.config.json")),
    ("drifting_grating", include_str!("../config/demos/drifting_grating.config.json")),
    ("gratings_triggered", include_str!("../config/demos/gratings_triggered.config.json")),
    ("moving_target", include_str!("../config/demos/moving_target.config.json")),
    ("photodiode_flicker", include_str!("../config/demos/photodiode_flicker.config.json")),
    ("trigger_gate", include_str!("../config/demos/trigger_gate.config.json")),
];

/// Sidecar recording the fingerprint of each demo file this server wrote, so a
/// later version can tell "the file I installed" from "the file the operator
/// changed". One `name hash` pair per line; unparsable lines are ignored.
const DEMO_STAMP_FILE: &str = ".vstimd_demo_seed";

/// FNV-1a over the file bytes. Change detection only — never a security
/// boundary — so a short non-cryptographic hash is the right size of tool.
fn demo_fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn read_demo_stamps(dir: &std::path::Path) -> std::collections::HashMap<String, u64> {
    let mut stamps = std::collections::HashMap::new();
    let Ok(raw) = std::fs::read_to_string(dir.join(DEMO_STAMP_FILE)) else {
        return stamps;
    };
    for line in raw.lines() {
        if let Some((name, hash)) = line.split_once(' ')
            && let Ok(hash) = hash.trim().parse::<u64>()
        {
            stamps.insert(name.to_string(), hash);
        }
    }
    stamps
}

fn write_demo_stamps(dir: &std::path::Path, stamps: &std::collections::HashMap<String, u64>) {
    let mut lines: Vec<String> = stamps.iter().map(|(n, h)| format!("{n} {h}")).collect();
    lines.sort();
    // Best-effort: losing the stamp file only costs the next refresh, which
    // then leaves the on-disk demo alone (the safe direction).
    let _ = std::fs::write(dir.join(DEMO_STAMP_FILE), lines.join("\n") + "\n");
}

/// What [`seed_demo_configs`] did, per outcome. Installing and refreshing are
/// reported apart because they mean different things to an operator: one added
/// a file, the other *replaced* one that was already there.
#[derive(Default, Debug)]
pub struct DemoSeedReport {
    /// Demos that were not present and have now been written.
    pub installed: Vec<&'static str>,
    /// Demos that were present, unmodified since this server installed them,
    /// and have been replaced with a newer shipped version.
    pub refreshed: Vec<&'static str>,
    /// Demos left exactly as they were, and why — see [`DemoSkip`].
    pub kept: Vec<(&'static str, DemoSkip)>,
    /// Per-file errors. Collected, never propagated: failing to install a demo
    /// must not stop the server from starting.
    pub failed: Vec<(&'static str, std::io::Error)>,
}

/// Why a demo already on disk was left alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemoSkip {
    /// Byte-identical to the shipped version — nothing to do. The file is
    /// (re)stamped, so a copy restored by hand stays in the refresh path.
    UpToDate,
    /// Changed since this server wrote it: the operator's file, and theirs to
    /// keep. It stops tracking the shipped version until they delete it.
    Modified,
    /// Present with no stamp — this server never wrote it, so it cannot be told
    /// apart from an operator's file and is treated as one. Applies to config
    /// dirs that predate the stamp sidecar.
    Unstamped,
}

/// Install the [`DEMO_CONFIGS`] into `dir`, refreshing the ones this server
/// previously wrote and never clobbering the ones an operator touched.
///
/// | On disk | Action |
/// |---|---|
/// | absent | written, fingerprint recorded |
/// | identical to the shipped copy | left; (re)stamped so it stays in the refresh path |
/// | matches the fingerprint we recorded, but the shipped copy has changed | **replaced** with the new version |
/// | anything else — edited, or present with no stamp | left alone, permanently |
///
/// The refresh case is the one that matters for shipping fixes: without it an
/// improved demo could never reach a rig that already had the old file — the fix
/// ships, the rig keeps the stale copy, and the demo silently behaves like the
/// old version.
///
/// A demo the operator deletes comes back on the next start; deleting it for
/// good means saving something else under that name.
pub fn seed_demo_configs(storage_dir: &std::path::Path) -> DemoSeedReport {
    let mut report = DemoSeedReport::default();
    let dir = scene_config_dir(storage_dir, DEMOS_PROJECT);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        report.failed.push(("(demos project)", e));
        return report;
    }
    let mut stamps = read_demo_stamps(&dir);

    for (name, json) in DEMO_CONFIGS {
        let path = dir.join(format!("{name}.config.json"));
        let shipped = demo_fingerprint(json.as_bytes());
        let mut refreshing = false;
        match std::fs::read(&path) {
            Ok(on_disk) => {
                let current = demo_fingerprint(&on_disk);
                if current == shipped {
                    stamps.insert((*name).to_string(), shipped);
                    report.kept.push((*name, DemoSkip::UpToDate));
                    continue;
                }
                match stamps.get(*name) {
                    Some(&stamped) if stamped == current => refreshing = true,
                    Some(_) => {
                        report.kept.push((*name, DemoSkip::Modified));
                        continue;
                    }
                    None => {
                        report.kept.push((*name, DemoSkip::Unstamped));
                        continue;
                    }
                }
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                report.failed.push((*name, e));
                continue;
            }
            Err(_) => {} // absent — fall through and write it
        }
        match std::fs::write(&path, json) {
            Ok(()) => {
                stamps.insert((*name).to_string(), shipped);
                if refreshing {
                    report.refreshed.push(*name);
                } else {
                    report.installed.push(*name);
                }
            }
            Err(e) => report.failed.push((*name, e)),
        }
    }

    write_demo_stamps(&dir, &stamps);
    report
}

/// Bare scene-config names (no path, no extension) in one project, sorted. A
/// project with no `scene-configs/` directory yet lists empty rather than
/// erroring — an empty project and a missing one are the same thing here.
pub fn list_scene_config_names(
    storage_dir: &std::path::Path,
    project: &str,
) -> anyhow::Result<Vec<String>> {
    let dir = scene_config_dir(storage_dir, project);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(bare) = name.strip_suffix(".config.json") {
            names.push(bare.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Project names present under `<storage-dir>/projects`, sorted. Non-directories
/// and names the layout would reject are skipped: the filesystem is the source
/// of truth, and files arrive over Samba and ssh without the server's knowledge.
pub fn list_projects(storage_dir: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let dir = projects_dir(storage_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_valid_name(&name) {
            names.push(name.into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Every scene-config in the storage dir, as [`SceneConfigRef::qualified`]
/// strings: bare inside `default`, `<project>/<name>` elsewhere. This is what
/// `ListSceneConfigs` returns when no project is named.
pub fn list_all_scene_configs(storage_dir: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let mut out = vec![];
    for project in list_projects(storage_dir)? {
        for name in list_scene_config_names(storage_dir, &project)? {
            out.push(SceneConfigRef { project: project.clone(), name }.qualified());
        }
    }
    out.sort();
    Ok(out)
}

// ── Startup storage directory ─────────────────────────────────────────────────

/// Default storage directory for a deployed rig. Matches the packaged systemd
/// unit (`StateDirectory=braemons/vstimd`). Used when `--storage-dir` is not
/// given; falls back to a home directory if this is not writable.
pub const DEFAULT_STORAGE_DIR: &str = "/var/lib/braemons/vstimd";

/// True if `dir` exists (creating it if needed) and a file can be created in
/// it. Used to pick a writable storage dir at startup — a rig running as a
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

/// Warn past this many timestamped archives in the session project — they are
/// never pruned automatically, so this nudges an operator to clean up.
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

/// Count timestamped archive scene-configs in the session project (ignores the
/// last-session slot). Returns 0 on any read error.
pub fn count_archive_configs(storage_dir: &std::path::Path) -> usize {
    list_scene_config_names(storage_dir, SESSION_PROJECT)
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
    fn names_are_validated_not_sanitised() {
        assert!(is_valid_name("faces2026"));
        assert!(is_valid_name("_last_session"));
        assert!(is_valid_name("a.b-c_d"));
        assert!(is_valid_name(&"x".repeat(64)));

        assert!(!is_valid_name(""));
        assert!(!is_valid_name(&"x".repeat(65)));
        assert!(!is_valid_name("."));
        assert!(!is_valid_name(".."));
        assert!(!is_valid_name("a/b"));
        assert!(!is_valid_name("a\\b"));
        assert!(!is_valid_name("-flag"));
        assert!(!is_valid_name("caf\u{e9}"));
    }

    #[test]
    fn scene_config_refs_round_trip() {
        let bare = SceneConfigRef::parse("center_target", DEFAULT_PROJECT).unwrap();
        assert_eq!(bare.project, DEFAULT_PROJECT);
        assert_eq!(bare.name, "center_target");
        // Bare in `default`, qualified elsewhere — so the common case stays one word.
        assert_eq!(bare.qualified(), "center_target");

        let qualified = SceneConfigRef::parse("demos/gratings", DEFAULT_PROJECT).unwrap();
        assert_eq!(qualified.project, "demos");
        assert_eq!(qualified.qualified(), "demos/gratings");

        // An unqualified name lands in whatever project is active.
        let active = SceneConfigRef::parse("session1", "faces2026").unwrap();
        assert_eq!(active.qualified(), "faces2026/session1");

        // Traversal is rejected at parse time, so no path built from a ref can escape.
        assert!(SceneConfigRef::parse("../etc/passwd", DEFAULT_PROJECT).is_err());
        assert!(SceneConfigRef::parse("..", DEFAULT_PROJECT).is_err());
        assert!(SceneConfigRef::parse("a/b/c", DEFAULT_PROJECT).is_err());
    }

    #[test]
    fn layout_puts_a_scene_config_in_its_project() {
        let state = std::path::Path::new("/var/lib/braemons/vstimd");
        let r = SceneConfigRef::parse("demos/gratings", DEFAULT_PROJECT).unwrap();
        assert_eq!(
            scene_config_path(state, &r),
            state.join("projects/demos/scene-configs/gratings.config.json"),
        );
    }

    #[test]
    fn shipped_demo_names_are_legal_and_unprefixed() {
        for (name, _) in DEMO_CONFIGS {
            assert!(is_valid_name(name), "demo name {name:?} is not a legal name");
            assert!(!name.starts_with("demo_"), "demo {name:?} still carries the retired prefix");
        }
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
