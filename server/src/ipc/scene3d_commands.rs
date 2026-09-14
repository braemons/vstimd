//! The 3-D scene's camera and lighting — scene-wide, like the background.

use super::convert::{
    camera3d_from_proto, camera3d_to_proto, lighting3d_from_proto, lighting3d_to_proto,
};
use super::response::{ok_ack, ok_body};
use crate::proto;
use crate::scene::SceneState;

impl SceneState {
    pub(super) fn cmd_set_camera(&mut self, cmd: proto::SetCameraRequest) -> proto::Response {
        match camera3d_from_proto(cmd.camera) {
            Ok(camera) => {
                self.config.camera.set(self.runtime.deferred_mode, camera);
                ok_ack()
            }
            Err(refusal) => *refusal,
        }
    }

    /// The live camera — what is on screen, not what deferred mode has staged.
    pub(super) fn cmd_query_camera(&self) -> proto::Response {
        ok_body(proto::response::Body::Camera(camera3d_to_proto(
            &self.config.camera.live,
        )))
    }

    pub(super) fn cmd_set_lighting(&mut self, cmd: proto::SetLightingRequest) -> proto::Response {
        match lighting3d_from_proto(cmd.lighting) {
            Ok(lighting) => {
                self.config
                    .lighting
                    .set(self.runtime.deferred_mode, lighting);
                ok_ack()
            }
            Err(refusal) => *refusal,
        }
    }

    pub(super) fn cmd_query_lighting(&self) -> proto::Response {
        ok_body(proto::response::Body::Lighting(lighting3d_to_proto(
            &self.config.lighting.live,
        )))
    }
}
