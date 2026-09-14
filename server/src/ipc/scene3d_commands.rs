//! The 3-D scene's camera and lighting — scene-wide, like the background.

use super::convert::{
    camera_zone_from_proto, camera_zone_to_proto, camera3d_from_proto, camera3d_to_proto, lighting3d_from_proto, lighting3d_to_proto,
};
use super::response::{ok_ack, ok_body};
use crate::proto;
use crate::scene::SceneState;
use crate::vtl_state::VtlState;

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

    /// Replace every camera zone. Zones keep no "inside" memory across a
    /// replace: a zone the camera is already in fires its entry next frame.
    pub(super) fn cmd_set_camera_zones(
        &mut self,
        cmd: proto::SetCameraZonesRequest,
        vtl: Option<&VtlState>,
    ) -> proto::Response {
        let names = vtl.map_or(&[][..], |v| v.names.as_slice());
        let zones = match cmd
            .zones
            .iter()
            .map(|z| camera_zone_from_proto(z, names))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(z) => z,
            Err(refusal) => return *refusal,
        };
        if let Err(msg) = crate::scene::zones::validate_zones(&zones) {
            return super::response::err(proto::ErrorCode::InvalidArgument, msg);
        }
        self.config.camera_zones = zones;
        ok_ack()
    }

    pub(super) fn cmd_list_camera_zones(&self) -> proto::Response {
        ok_body(proto::response::Body::CameraZoneList(proto::ListCameraZonesResponse {
            zones: self.config.camera_zones.iter().map(camera_zone_to_proto).collect(),
        }))
    }
}
