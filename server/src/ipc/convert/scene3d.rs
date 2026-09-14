//! Camera and lighting <-> proto.

use super::mesh3d::{Refusal, vec3_from_proto, vec3_to_proto};
use crate::ipc::response::err;
use crate::proto;
use crate::scene::units::Pos3Cm;
use crate::scene::zones::CameraZone;
use crate::scene::{Camera3D, Lighting3D};
use crate::vtl_state::VtlNameEntry;

fn invalid(msg: impl Into<String>) -> Refusal {
    Box::new(err(proto::ErrorCode::InvalidArgument, msg))
}

/// Zero-means-default for the projection fields; the pose has no "unset".
pub(crate) fn camera3d_from_proto(c: Option<proto::Camera3D>) -> Result<Camera3D, Refusal> {
    let Some(c) = c else {
        return Err(invalid("camera must be set"));
    };
    let default = Camera3D::default();
    let or_default = |v: f32, d: f32| if v == 0.0 { d } else { v };
    let camera = Camera3D {
        position_cm: Pos3Cm(vec3_from_proto(c.position_cm)),
        yaw_deg: c.yaw_deg,
        pitch_deg: c.pitch_deg,
        roll_deg: c.roll_deg,
        fov_y_deg: or_default(c.fov_y_deg, default.fov_y_deg),
        near_cm: or_default(c.near_cm, default.near_cm),
        far_cm: or_default(c.far_cm, default.far_cm),
    };
    let all_finite = camera.position_cm.0.is_finite()
        && [
            camera.yaw_deg,
            camera.pitch_deg,
            camera.roll_deg,
            camera.fov_y_deg,
            camera.near_cm,
            camera.far_cm,
        ]
        .iter()
        .all(|v| v.is_finite());
    if !all_finite {
        return Err(invalid("camera fields must be finite"));
    }
    if !(camera.fov_y_deg > 0.0 && camera.fov_y_deg < 180.0) {
        return Err(invalid(format!(
            "fov_y_deg must be in (0, 180), got {}",
            camera.fov_y_deg
        )));
    }
    if camera.near_cm <= 0.0 || camera.far_cm <= camera.near_cm {
        return Err(invalid(format!(
            "need 0 < near_cm < far_cm, got near_cm={} far_cm={}",
            camera.near_cm, camera.far_cm
        )));
    }
    Ok(camera)
}

pub(crate) fn camera3d_to_proto(c: &Camera3D) -> proto::Camera3D {
    proto::Camera3D {
        position_cm: Some(vec3_to_proto(c.position_cm.0)),
        yaw_deg: c.yaw_deg,
        pitch_deg: c.pitch_deg,
        roll_deg: c.roll_deg,
        fov_y_deg: c.fov_y_deg,
        near_cm: c.near_cm,
        far_cm: c.far_cm,
    }
}

pub(crate) fn lighting3d_from_proto(l: Option<proto::Lighting3D>) -> Result<Lighting3D, Refusal> {
    let Some(l) = l else {
        return Err(invalid("lighting must be set"));
    };
    let lighting = Lighting3D {
        ambient_color: vec3_from_proto(l.ambient_color).to_array(),
        sun_direction: vec3_from_proto(l.sun_direction),
        sun_color: vec3_from_proto(l.sun_color).to_array(),
    };
    let colors_finite = lighting
        .ambient_color
        .iter()
        .chain(&lighting.sun_color)
        .all(|v| v.is_finite());
    if !colors_finite || !lighting.sun_direction.is_finite() {
        return Err(invalid("lighting fields must be finite"));
    }
    if lighting.sun_direction.length_squared() == 0.0 {
        return Err(invalid("sun_direction must not be zero"));
    }
    Ok(lighting)
}

pub(crate) fn lighting3d_to_proto(l: &Lighting3D) -> proto::Lighting3D {
    proto::Lighting3D {
        ambient_color: Some(vec3_to_proto(glam::Vec3::from(l.ambient_color))),
        sun_direction: Some(vec3_to_proto(l.sun_direction)),
        sun_color: Some(vec3_to_proto(glam::Vec3::from(l.sun_color))),
    }
}

pub(crate) fn camera_zone_from_proto(
    z: &proto::CameraZone,
    names: &[VtlNameEntry],
) -> Result<CameraZone, Refusal> {
    let range = |axis: &str, lo: Option<f32>, hi: Option<f32>| match (lo, hi) {
        (None, None) => Ok(None),
        (Some(lo), Some(hi)) => Ok(Some([lo, hi])),
        _ => Err(invalid(format!("zone '{}': give both {axis} bounds or neither", z.name))),
    };
    let zone = CameraZone {
        name: z.name.clone(),
        line: super::vtl_bit_from_proto(z.line.as_ref(), names)?,
        x_cm: range("x", z.x_min_cm, z.x_max_cm)?,
        y_cm: range("y", z.y_min_cm, z.y_max_cm)?,
        z_cm: range("z", z.z_min_cm, z.z_max_cm)?,
        inside: false,
    };
    zone.validate().map_err(invalid)?;
    Ok(zone)
}

pub(crate) fn camera_zone_to_proto(z: &CameraZone) -> proto::CameraZoneInfo {
    let lo = |r: Option<[f32; 2]>| r.map(|r| r[0]);
    let hi = |r: Option<[f32; 2]>| r.map(|r| r[1]);
    proto::CameraZoneInfo {
        zone: Some(proto::CameraZone {
            name: z.name.clone(),
            line: Some(super::vtl_bit_to_proto(z.line)),
            x_min_cm: lo(z.x_cm),
            x_max_cm: hi(z.x_cm),
            y_min_cm: lo(z.y_cm),
            y_max_cm: hi(z.y_cm),
            z_min_cm: lo(z.z_cm),
            z_max_cm: hi(z.z_cm),
        }),
        inside: z.inside,
    }
}
