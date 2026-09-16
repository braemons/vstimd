//! Create/modify commands for 3-D stimuli: cube, sphere, plane and corridor.

use super::convert::{
    Mesh3dParts, Refusal, corridor3d_from_proto, cube_size_from_proto, cube3d_from_proto,
    identity_from_proto, material3d_from_proto,
    plane_size_from_proto, plane3d_from_proto, sphere_diameter_from_proto, sphere3d_from_proto,
    transform3d_from_proto,
};
use super::response::{err, err_not_found, err_wrong_type, ok_ack, ok_handle_with_id};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::stimulus::{
    Mesh3d, Mesh3dGeometry, Stimulus, StimulusBody, StimulusSceneEntry, StimulusType,
    Transform3D,
};

impl SceneState {
    pub(super) fn refuse_if_3d_unavailable(&self) -> Option<proto::Response> {
        self.runtime.render_3d_unavailable.then(|| {
            err(
                proto::ErrorCode::NotSupported,
                "this display has no depth buffer format, so it cannot draw 3-D stimuli",
            )
        })
    }

    fn create_mesh3d(
        &mut self,
        identity: Option<proto::StimulusIdentity>,
        placement: Option<proto::Transform3D>,
        params: Result<Mesh3dParts, Refusal>,
    ) -> proto::Response {
        if let Some(refusal) = self.refuse_if_3d_unavailable() {
            return refusal;
        }
        let transform = match transform3d_from_proto(placement) {
            Ok(t) => t,
            Err(refusal) => return *refusal,
        };
        let parts = match params {
            Ok(p) => p,
            Err(refusal) => return *refusal,
        };
        let identity = identity_from_proto(identity);
        let id = identity.id;
        let handle = self.alloc_stim_handle();
        let mut mesh = Mesh3d::new(transform, parts.material, parts.geometry, parts.texture_path);
        mesh.repeat = parts.repeat;
        self.config.stimuli.insert(
            handle,
            StimulusSceneEntry::new(identity, Stimulus::from(mesh)),
        );
        ok_handle_with_id(handle, &id)
    }

    pub(super) fn cmd_create_cube_3d(
        &mut self,
        cmd: proto::CreateCube3DRequest,
    ) -> proto::Response {
        let params = cube3d_from_proto(cmd.params.unwrap_or_default());
        self.create_mesh3d(cmd.identity, cmd.placement, params)
    }

    pub(super) fn cmd_create_sphere_3d(
        &mut self,
        cmd: proto::CreateSphere3DRequest,
    ) -> proto::Response {
        let params = sphere3d_from_proto(cmd.params.unwrap_or_default());
        self.create_mesh3d(cmd.identity, cmd.placement, params)
    }

    pub(super) fn cmd_create_plane_3d(
        &mut self,
        cmd: proto::CreatePlane3DRequest,
    ) -> proto::Response {
        let params = plane3d_from_proto(cmd.params.unwrap_or_default());
        self.create_mesh3d(cmd.identity, cmd.placement, params)
    }

    pub(super) fn cmd_create_corridor_3d(
        &mut self,
        cmd: proto::CreateCorridor3DRequest,
    ) -> proto::Response {
        let params = corridor3d_from_proto(cmd.params.unwrap_or_default());
        self.create_mesh3d(cmd.identity, cmd.placement, params)
    }

    /// Run `f` on the 3-D stimulus at `handle`. `expected` narrows further to one
    /// geometry, for the size setters; the error then names that type.
    ///
    /// Nothing is marked dirty: a 3-D stimulus has no per-stimulus mesh to
    /// invalidate. Its shared mesh follows the geometry's key and everything else
    /// is a per-draw constant.
    fn with_mesh3d(
        &mut self,
        handle: u32,
        cmd: &str,
        expected: Option<StimulusType>,
        f: impl FnOnce(&mut Mesh3d, bool) -> Result<(), Refusal>,
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let wrong_type = match expected {
            Some(t) => entry.stimulus.stimulus_type() != t,
            None => !entry.stimulus.stimulus_type().is_3d(),
        };
        if wrong_type {
            return match expected {
                Some(t) => err_wrong_type(&entry.stimulus, cmd, t),
                None => err(
                    proto::ErrorCode::WrongStimulusType,
                    format!(
                        "{cmd} requires a 3-D stimulus, got {}",
                        entry.stimulus.type_name()
                    ),
                ),
            };
        }
        let StimulusBody::Mesh3d(m) = &mut entry.stimulus.body else {
            // The only 3-D type that is not a mesh: it has a placement but no
            // material or size, and SetTransform3D does not come through here.
            return err(
                proto::ErrorCode::WrongStimulusType,
                format!("{cmd} does not apply to {}", entry.stimulus.type_name()),
            );
        };
        match f(m, deferred) {
            Ok(()) => ok_ack(),
            Err(refusal) => *refusal,
        }
    }

    pub(super) fn cmd_set_transform_3d(
        &mut self,
        handle: u32,
        cmd: proto::SetTransform3DRequest,
    ) -> proto::Response {
        let deferred = self.runtime.deferred_mode;
        let Some(entry) = self.config.stimuli.get_mut(&handle) else {
            return err_not_found(handle);
        };
        let type_name = entry.stimulus.type_name();
        let Some(transform) = entry.stimulus.transform3d_mut() else {
            return err(
                proto::ErrorCode::WrongStimulusType,
                format!("SetTransform3D requires a 3-D stimulus, got {type_name}"),
            );
        };
        match transform3d_from_proto(cmd.transform) {
            Ok(t) => {
                let t: Transform3D = t;
                transform.set(deferred, t);
                ok_ack()
            }
            Err(refusal) => *refusal,
        }
    }

    pub(super) fn cmd_set_material_3d(
        &mut self,
        handle: u32,
        cmd: proto::SetMaterial3DRequest,
    ) -> proto::Response {
        self.with_mesh3d(handle, "SetMaterial3D", None, |m, deferred| {
            m.material
                .set(deferred, material3d_from_proto(cmd.material)?);
            Ok(())
        })
    }

    pub(super) fn cmd_set_cube_3d_size(
        &mut self,
        handle: u32,
        cmd: proto::SetCube3DSizeRequest,
    ) -> proto::Response {
        self.with_mesh3d(
            handle,
            "SetCube3DSize",
            Some(StimulusType::Cube3D),
            |m, deferred| {
                let size_cm = cube_size_from_proto(cmd.size_cm)?;
                m.geometry.set(deferred, Mesh3dGeometry::Cube { size_cm });
                Ok(())
            },
        )
    }

    pub(super) fn cmd_set_sphere_3d_diameter(
        &mut self,
        handle: u32,
        cmd: proto::SetSphere3DDiameterRequest,
    ) -> proto::Response {
        self.with_mesh3d(
            handle,
            "SetSphere3DDiameter",
            Some(StimulusType::Sphere3D),
            |m, deferred| {
                let diameter_cm = sphere_diameter_from_proto(cmd.diameter_cm)?;
                // Tessellation is fixed at creation, so it comes from whichever slot
                // the write lands in — the same slot the new diameter goes to.
                let current = if deferred {
                    m.geometry.copy
                } else {
                    m.geometry.live
                };
                let Mesh3dGeometry::Sphere { rings, sectors, .. } = current else {
                    unreachable!("a Sphere3D stimulus has sphere geometry");
                };
                m.geometry.set(
                    deferred,
                    Mesh3dGeometry::Sphere {
                        diameter_cm,
                        rings,
                        sectors,
                    },
                );
                Ok(())
            },
        )
    }

    pub(super) fn cmd_set_plane_3d_size(
        &mut self,
        handle: u32,
        cmd: proto::SetPlane3DSizeRequest,
    ) -> proto::Response {
        self.with_mesh3d(
            handle,
            "SetPlane3DSize",
            Some(StimulusType::Plane3D),
            |m, deferred| {
                let size_cm = plane_size_from_proto(cmd.size_cm)?;
                m.geometry.set(deferred, Mesh3dGeometry::Plane { size_cm });
                Ok(())
            },
        )
    }
}
