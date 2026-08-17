//! Config persistence commands. The scene-side loading and saving they call
//! lives on `SceneState` in `scene::scene_state`; only the proto plumbing is here.

use super::response::{err, ok_ack, ok_body};
use crate::scene_config_file::{
    config_path, is_format_error, is_not_found, list_config_names, parse_config_json,
    retrieve_config_json,
};
use crate::proto;
use crate::scene::{LoadMode, SceneState};
use crate::vtl_state::{VtlConfig, VtlState};

impl SceneState {
    pub(super) fn cmd_list_configs(&self) -> proto::Response {
        match list_config_names(&self.runtime.config_dir) {
            Ok(names) => ok_body(proto::response::Body::ConfigList(
                proto::ListConfigsResponse { names },
            )),
            Err(e) => err(proto::ErrorCode::FileIo, e.to_string()),
        }
    }

    pub(super) fn cmd_load_config(
        &mut self,
        cmd: proto::LoadConfigRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        match self.load_named_config(&cmd.name, cmd.additive, vtl) {
            Ok(()) => ok_ack(),
            Err(e) if is_not_found(&e) => err(proto::ErrorCode::FileNotFound, e.to_string()),
            Err(e) if is_format_error(&e) => err(proto::ErrorCode::FileFormat, e.to_string()),
            Err(e) => err(proto::ErrorCode::FileIo, e.to_string()),
        }
    }

    pub(super) fn cmd_upload_config(
        &mut self,
        cmd: proto::UploadConfigRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        let (scene_cfg, io) = match parse_config_json(&cmd.json) {
            Ok(v) => v,
            Err(e) => return err(proto::ErrorCode::FileFormat, e.to_string()),
        };
        let path = config_path(&self.runtime.config_dir, &cmd.name);
        if path.exists() && !cmd.overwrite {
            return err(proto::ErrorCode::FileAlreadyExists, "config already exists");
        }
        if let Err(e) = std::fs::create_dir_all(&self.runtime.config_dir)
            .and_then(|_| std::fs::write(&path, &cmd.json))
        {
            return err(proto::ErrorCode::FileIo, e.to_string());
        }
        if cmd.apply_now {
            if let Some(v) = vtl {
                v.config.names = io.vtl.names;
                v.sync_names_to_shm();
            }
            let mode = if cmd.additive {
                LoadMode::Additive
            } else {
                LoadMode::Replace
            };
            self.load_snapshot(scene_cfg, mode);
        }
        ok_ack()
    }

    pub(super) fn cmd_retrieve_config(&self, vtl: Option<&VtlState>) -> proto::Response {
        let default_vtl = VtlConfig::default();
        let vtl_cfg = vtl.map_or(&default_vtl, |v| &v.config);
        match retrieve_config_json(&self.config, vtl_cfg) {
            Ok(json) => ok_body(proto::response::Body::RetrievedConfig(
                proto::RetrieveConfigResponse { json },
            )),
            Err(e) => err(proto::ErrorCode::Unknown, e.to_string()),
        }
    }
}
