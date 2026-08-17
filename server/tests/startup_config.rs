//! Tests for the startup save/load boot path: `SceneState::save_named_config`
//! and `SceneState::load_named_config`, plus the `[startup]` config-dir layout.
//!
//! These exercise the same code the `main.rs` boot flow uses (load a named
//! config at startup, save the scene to the last-session slot on quit) without
//! a GPU, ZMQ, or a live VTL segment.

use std::sync::atomic::{AtomicU32, Ordering};

use uuid::Uuid;
use vstimd::scene_config_file::{
    config_path, count_archive_configs, dir_is_writable, first_writable_dir, is_archive_name,
    is_not_found, LAST_SESSION_CONFIG,
};
use vstimd::scene::{
    Deferred, RectStimulus, SceneState, ShapeAppearance, StimulusCommon, Stimulus,
    StimulusSceneEntry,
};

/// A unique scratch directory that is removed when dropped, so each test gets
/// an isolated config dir without a `tempfile` dependency.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vstimd_startup_test_{}_{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scene_with_rect(dir: &std::path::Path) -> SceneState {
    let mut scene = SceneState::new_with_config_dir(dir.to_path_buf());
    scene.add_stimulus(StimulusSceneEntry::new(
        Uuid::new_v4(),
        Some("target".into()),
        Stimulus::Rect(RectStimulus {
            common: StimulusCommon::new([10.0, 20.0], 0.0),
            appearance: Deferred::new(ShapeAppearance::default()),
            size: Deferred::new([100.0, 50.0]),
        }),
    ));
    scene
}

#[test]
fn save_then_load_named_config_roundtrips() {
    let dir = TempDir::new();

    // Save a one-stimulus scene under a name.
    let saved = scene_with_rect(dir.path());
    saved
        .save_named_config("center_target", None)
        .expect("save should succeed");

    // A fresh, empty scene loads it back and gets the stimulus.
    let mut loaded = SceneState::new_with_config_dir(dir.path().to_path_buf());
    assert_eq!(loaded.stimuli.len(), 0);
    loaded
        .load_named_config("center_target", false, None)
        .expect("load should succeed");
    assert_eq!(loaded.stimuli.len(), 1);
}

#[test]
fn save_named_config_writes_expected_path() {
    let dir = TempDir::new();
    let scene = scene_with_rect(dir.path());
    scene.save_named_config("my_scene", None).unwrap();

    let expected = config_path(dir.path(), "my_scene");
    assert!(expected.exists(), "config file should be at {expected:?}");
    assert_eq!(expected.file_name().unwrap(), "vstimd_my_scene.config.json");
}

#[test]
fn save_named_config_creates_missing_config_dir() {
    let dir = TempDir::new();
    let nested = dir.path().join("does/not/exist/yet");
    let scene = scene_with_rect(&nested);
    scene
        .save_named_config("boot", None)
        .expect("save should create the config dir");
    assert!(config_path(&nested, "boot").exists());
}

#[test]
fn load_missing_named_config_reports_not_found() {
    let dir = TempDir::new();
    let mut scene = SceneState::new_with_config_dir(dir.path().to_path_buf());
    let err = scene
        .load_named_config("nope", false, None)
        .expect_err("loading a missing config should fail");
    assert!(is_not_found(&err), "expected a not-found error, got: {err}");
}

#[test]
fn last_session_slot_roundtrips() {
    // Mirrors save_on_quit → load_config = "last": save to the reserved slot,
    // then load it back on the next boot.
    let dir = TempDir::new();
    let saved = scene_with_rect(dir.path());
    saved.save_named_config(LAST_SESSION_CONFIG, None).unwrap();

    let mut restored = SceneState::new_with_config_dir(dir.path().to_path_buf());
    restored
        .load_named_config(LAST_SESSION_CONFIG, false, None)
        .expect("last-session slot should load");
    assert_eq!(restored.stimuli.len(), 1);
}

#[test]
fn session_snapshot_writes_last_and_timestamped_archive() {
    let dir = TempDir::new();
    let scene = scene_with_rect(dir.path());

    let archive = scene
        .save_session_snapshot(None)
        .expect("session snapshot should succeed");

    // Both artifacts exist: the overwritable last-session slot and the archive.
    assert!(config_path(dir.path(), LAST_SESSION_CONFIG).exists());
    assert!(config_path(dir.path(), &archive).exists());
    // The archive name is a recognisable timestamp, distinct from the slot.
    assert!(is_archive_name(&archive), "archive name '{archive}' should be a timestamp");
    assert_ne!(archive, LAST_SESSION_CONFIG);
    // Exactly one archive so far; the last-session slot is not counted.
    assert_eq!(count_archive_configs(dir.path()), 1);
}

#[test]
fn dir_is_writable_detects_reachable_dir() {
    let dir = TempDir::new();
    // A fresh temp dir is writable; a nested path is created on demand.
    assert!(dir_is_writable(dir.path()));
    assert!(dir_is_writable(&dir.path().join("nested/child")));
}

#[test]
fn first_writable_dir_skips_unwritable_candidates() {
    let dir = TempDir::new();
    let good = dir.path().join("good");
    // A path under /proc cannot be created — stands in for a non-writable
    // system dir like /var/lib on a non-root run.
    let bad = std::path::PathBuf::from("/proc/vstimd_cannot_create_here");
    let chosen = first_writable_dir(&[bad, good.clone()]);
    assert_eq!(chosen, good);
}

#[test]
fn load_named_config_replace_clears_previous_scene() {
    let dir = TempDir::new();
    // An empty saved scene…
    let empty = SceneState::new_with_config_dir(dir.path().to_path_buf());
    empty.save_named_config("empty", None).unwrap();

    // …loaded over a populated scene replaces it (default, non-additive).
    let mut scene = scene_with_rect(dir.path());
    assert_eq!(scene.stimuli.len(), 1);
    scene.load_named_config("empty", false, None).unwrap();
    assert_eq!(scene.stimuli.len(), 0);
}
