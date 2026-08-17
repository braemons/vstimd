//! Named-config load/save on the scene itself. The matching IPC commands
//! live in `ipc::config_commands`; nothing here speaks protobuf.

use super::SceneState;
use crate::io_config::{
    ARCHIVE_WARN_THRESHOLD, LAST_SESSION_CONFIG, archive_timestamp_name, config_path,
    count_archive_configs, load_config, save_config,
};
use crate::vtl_state::{VtlConfig, VtlState};

impl SceneState {
    /// Load a named config from the config directory into the scene, replacing
    /// (or, with `additive`, merging) the current scene and — if a VTL segment
    /// is present — its line names. Shared by the `LoadConfig` command and the
    /// `[startup] load_config` boot path.
    pub fn load_named_config(
        &mut self,
        name: &str,
        additive: bool,
        vtl: Option<&mut VtlState>,
    ) -> anyhow::Result<()> {
        let path = config_path(&self.runtime.config_dir, name);
        let (scene_cfg, io) = load_config(&path)?;
        if let Some(v) = vtl {
            v.config.names = io.vtl.names;
            v.sync_names_to_shm();
        }
        let mode = if additive {
            super::scene_config::LoadMode::Additive
        } else {
            super::scene_config::LoadMode::Replace
        };
        self.load_snapshot(scene_cfg, mode);
        Ok(())
    }

    /// Save the current scene and VTL line names to a named config file in the
    /// config directory, creating the directory if needed.
    pub fn save_named_config(&self, name: &str, vtl: Option<&VtlState>) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.runtime.config_dir)?;
        let path = config_path(&self.runtime.config_dir, name);
        let default_vtl = VtlConfig::default();
        let vtl_cfg = vtl.map_or(&default_vtl, |v| &v.config);
        save_config(&self.config, vtl_cfg, &path)
    }

    /// Quit-time save (`[startup] save_on_quit`): overwrite the last-session
    /// slot and write a timestamped archive so history is preserved. Returns
    /// the archive's config name. Logs a warning once archives pile up past
    /// [`ARCHIVE_WARN_THRESHOLD`] — they are never pruned automatically.
    pub fn save_session_snapshot(&self, vtl: Option<&VtlState>) -> anyhow::Result<String> {
        self.save_named_config(LAST_SESSION_CONFIG, vtl)?;
        let archive = archive_timestamp_name();
        self.save_named_config(&archive, vtl)?;

        let n = count_archive_configs(&self.runtime.config_dir);
        if n > ARCHIVE_WARN_THRESHOLD {
            log::warn!(
                "vstimd: {n} timestamped session archives in {:?} — consider pruning old ones",
                self.runtime.config_dir
            );
        }
        Ok(archive)
    }
}
