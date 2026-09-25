//! Scene-config persistence commands. The scene-side loading and saving they
//! call lives on `SceneState` in `scene::scene_state`; only the proto plumbing
//! is here.

use super::response::{err, ok_ack, ok_body};
use crate::proto;
use crate::scene::{LoadMode, SceneState};
use crate::scene_config_file::{
    is_format_error, is_not_found, list_all_scene_configs, list_scene_config_names,
    parse_config_json, retrieve_config_json, scene_config_path,
};
use crate::vtl_state::{VtlConfig, VtlState};

impl SceneState {
    pub(super) fn cmd_list_scene_configs(
        &self,
        cmd: proto::ListSceneConfigsRequest,
    ) -> proto::Response {
        // An empty project means "the whole store"; naming one scopes the
        // listing to it and drops back to bare names.
        let listed = if cmd.project.is_empty() {
            list_all_scene_configs(&self.runtime.storage_dir)
        } else {
            list_scene_config_names(&self.runtime.storage_dir, &cmd.project)
        };
        match listed {
            Ok(names) => ok_body(proto::response::Body::SceneConfigList(
                proto::ListSceneConfigsResponse { names },
            )),
            Err(e) => err(proto::ErrorCode::FileIo, e.to_string()),
        }
    }

    pub(super) fn cmd_load_scene_config(
        &mut self,
        cmd: proto::LoadSceneConfigRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        match self.load_named_config(&cmd.name, cmd.additive, vtl) {
            Ok(()) => ok_ack(),
            Err(e) if is_not_found(&e) => err(proto::ErrorCode::FileNotFound, e.to_string()),
            Err(e) if is_format_error(&e) => err(proto::ErrorCode::FileFormat, e.to_string()),
            Err(e) => err(proto::ErrorCode::FileIo, e.to_string()),
        }
    }

    pub(super) fn cmd_upload_scene_config(
        &mut self,
        cmd: proto::UploadSceneConfigRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        let (scene_cfg, io) = match parse_config_json(&cmd.json) {
            Ok(v) => v,
            Err(e) => return err(proto::ErrorCode::FileFormat, e.to_string()),
        };
        // Resolve before writing: a name that names an illegal project or
        // escapes the tree is refused, never sanitised.
        let scene_config_ref = match self.scene_config_ref(&cmd.name) {
            Ok(r) => r,
            Err(e) => return err(proto::ErrorCode::InvalidArgument, e.to_string()),
        };
        let path = scene_config_path(&self.runtime.storage_dir, &scene_config_ref);
        if path.exists() && !cmd.overwrite {
            return err(
                proto::ErrorCode::FileAlreadyExists,
                "scene-config already exists",
            );
        }
        let created = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, &cmd.json));
        if let Err(e) = created {
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

    pub(super) fn cmd_retrieve_scene_config(&self, vtl: Option<&VtlState>) -> proto::Response {
        let default_vtl = VtlConfig::default();
        let vtl_cfg = vtl.map_or(&default_vtl, |v| &v.config);
        match retrieve_config_json(&self.config, vtl_cfg) {
            Ok(json) => ok_body(proto::response::Body::RetrievedSceneConfig(
                proto::RetrieveSceneConfigResponse { json },
            )),
            Err(e) => err(proto::ErrorCode::Unknown, e.to_string()),
        }
    }
}
