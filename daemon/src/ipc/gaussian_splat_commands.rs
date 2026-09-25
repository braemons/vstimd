//! Create command for the Gaussian splat stimulus.
//!
//! A prototype, kept in a module of its own so it can be added, disabled or
//! removed without touching the other 3-D commands. It shares only the 3-D
//! availability guard with `mesh3d_commands`, because a splat scene is refused
//! on the same displays a mesh is.

use super::convert::{gaussian_splat3d_from_proto, identity_from_proto, transform3d_from_proto};
use super::response::ok_handle_with_id;
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::{GaussianSplat, Stimulus, StimulusSceneEntry};

impl SceneState {
    pub(super) fn cmd_create_gaussian_splat_3d(
        &mut self,
        cmd: proto::CreateGaussianSplat3DRequest,
    ) -> proto::Response {
        if let Some(refusal) = self.refuse_if_3d_unavailable() {
            return refusal;
        }
        let transform = match transform3d_from_proto(cmd.placement) {
            Ok(t) => t,
            Err(refusal) => return *refusal,
        };
        let (path, info) = match gaussian_splat3d_from_proto(cmd.params.unwrap_or_default()) {
            Ok(p) => p,
            Err(refusal) => return *refusal,
        };
        log::info!("vstimd: GaussianSplat3D {path}: {} splats ({:?})", info.count, info.format);
        let identity = identity_from_proto(cmd.identity);
        let id = identity.id;
        let handle = self.alloc_stim_handle();
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(identity, Stimulus::from(GaussianSplat::new(transform, path))),
        );
        ok_handle_with_id(handle, &id)
    }
}
