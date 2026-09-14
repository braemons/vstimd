//! 3-D stimulus <-> proto conversions: placement, material and the per-geometry
//! params.
//!
//! Requests are validated here, where the wire's zero-means-default convention
//! is resolved: zero picks the default, negative or non-finite is refused. A
//! refusal is a ready `INVALID_ARGUMENT` response, the same shape
//! `draw_mode_from_proto` uses.

use glam::Vec3;

use super::color_or_default;
use crate::Color;
use crate::ipc::response::err;
use crate::proto;
use crate::scene::stimulus::{Material3D, Mesh3d, Mesh3dGeometry, Shading3D, Transform3D};

pub(crate) type Refusal = Box<proto::Response>;

fn invalid(msg: impl Into<String>) -> Refusal {
    Box::new(err(proto::ErrorCode::InvalidArgument, msg))
}

/// Default full extent of a cube side and a sphere, cm.
pub(crate) const DEFAULT_SIZE_CM: f32 = 10.0;
/// Default full extent of a plane side, cm.
pub(crate) const DEFAULT_PLANE_SIZE_CM: f32 = 100.0;
pub(crate) const DEFAULT_SPHERE_RINGS: u32 = 16;
pub(crate) const DEFAULT_SPHERE_SECTORS: u32 = 32;
/// Beyond this a sphere is millions of vertices for no visible gain.
pub(crate) const MAX_SPHERE_TESSELLATION: u32 = 256;

pub(crate) fn vec3_from_proto(v: Option<proto::Vec3>) -> Vec3 {
    v.map_or(Vec3::ZERO, |v| Vec3::new(v.x, v.y, v.z))
}

pub(crate) fn vec3_to_proto(v: Vec3) -> proto::Vec3 {
    proto::Vec3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

fn finite(v: Vec3, what: &str) -> Result<Vec3, Refusal> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(invalid(format!("{what} must be finite")))
    }
}

/// One size component: zero → `default`, negative or non-finite → refused.
fn extent(v: f32, default: f32, what: &str) -> Result<f32, Refusal> {
    if !v.is_finite() || v < 0.0 {
        Err(invalid(format!(
            "{what} must be a non-negative number, got {v}"
        )))
    } else if v == 0.0 {
        Ok(default)
    } else {
        Ok(v)
    }
}

// ── Placement ─────────────────────────────────────────────────────────────────

pub(crate) fn transform3d_from_proto(
    t: Option<proto::Transform3D>,
) -> Result<Transform3D, Refusal> {
    let Some(t) = t else {
        return Ok(Transform3D::default());
    };
    let mut scale = finite(vec3_from_proto(t.scale), "scale")?;
    if t.scale.is_none() {
        scale = Vec3::ONE;
    }
    if scale.min_element() < 0.0 {
        // A negative scale mirrors the mesh, which reverses its winding and
        // culls the faces that should show.
        return Err(invalid(format!(
            "scale components must not be negative, got {scale}"
        )));
    }
    Ok(Transform3D {
        position_cm: finite(vec3_from_proto(t.position_cm), "position_cm")?,
        rotation_deg: finite(vec3_from_proto(t.rotation_deg), "rotation_deg")?,
        scale: Vec3::select(scale.cmpeq(Vec3::ZERO), Vec3::ONE, scale),
    })
}

pub(crate) fn transform3d_to_proto(t: &Transform3D) -> proto::Transform3D {
    proto::Transform3D {
        position_cm: Some(vec3_to_proto(t.position_cm)),
        rotation_deg: Some(vec3_to_proto(t.rotation_deg)),
        scale: Some(vec3_to_proto(t.scale)),
    }
}

// ── Material ──────────────────────────────────────────────────────────────────

pub(crate) fn shading3d_from_proto(v: i32) -> Shading3D {
    match proto::Shading::try_from(v).unwrap_or(proto::Shading::Unspecified) {
        proto::Shading::Unspecified | proto::Shading::Unlit => Shading3D::Unlit,
        proto::Shading::Phong => Shading3D::Phong,
    }
}

pub(crate) fn shading3d_to_proto(s: Shading3D) -> proto::Shading {
    match s {
        Shading3D::Unlit => proto::Shading::Unlit,
        Shading3D::Phong => proto::Shading::Phong,
    }
}

pub(crate) fn material3d_from_proto(m: Option<proto::Material3D>) -> Result<Material3D, Refusal> {
    let Some(m) = m else {
        return Ok(Material3D::default());
    };
    Ok(Material3D {
        albedo: color_or_default(m.albedo, Color::WHITE),
        emissive: finite(vec3_from_proto(m.emissive), "emissive")?.to_array(),
        shading: shading3d_from_proto(m.shading),
    })
}

pub(crate) fn material3d_to_proto(m: &Material3D) -> proto::Material3D {
    proto::Material3D {
        albedo: Some(m.albedo.into()),
        emissive: Some(vec3_to_proto(Vec3::from(m.emissive))),
        shading: shading3d_to_proto(m.shading) as i32,
    }
}

// ── Geometry ──────────────────────────────────────────────────────────────────

pub(crate) fn cube_size_from_proto(v: Option<proto::Vec3>) -> Result<[f32; 3], Refusal> {
    let v = vec3_from_proto(v);
    Ok([
        extent(v.x, DEFAULT_SIZE_CM, "size_cm.x")?,
        extent(v.y, DEFAULT_SIZE_CM, "size_cm.y")?,
        extent(v.z, DEFAULT_SIZE_CM, "size_cm.z")?,
    ])
}

pub(crate) fn sphere_diameter_from_proto(d: f32) -> Result<f32, Refusal> {
    extent(d, DEFAULT_SIZE_CM, "diameter_cm")
}

pub(crate) fn plane_size_from_proto(v: Option<proto::Vec2>) -> Result<[f32; 2], Refusal> {
    let v = v.unwrap_or_default();
    Ok([
        extent(v.x, DEFAULT_PLANE_SIZE_CM, "size_cm.x")?,
        extent(v.y, DEFAULT_PLANE_SIZE_CM, "size_cm.y")?,
    ])
}

fn tessellation(v: u32, default: u32, what: &str) -> Result<u32, Refusal> {
    match v {
        0 => Ok(default),
        v if v > MAX_SPHERE_TESSELLATION => Err(invalid(format!(
            "{what} must be at most {MAX_SPHERE_TESSELLATION}, got {v}"
        ))),
        v => Ok(v),
    }
}

/// Textures are not implemented yet; refusing a path beats silently ignoring it.
pub(crate) fn texture_path_from_proto(path: String) -> Result<Option<String>, Refusal> {
    if path.is_empty() {
        Ok(None)
    } else {
        Err(Box::new(err(
            proto::ErrorCode::NotSupported,
            "3-D textures are not supported yet (texture_path must be empty)",
        )))
    }
}

/// A validated create request, ready to become a stimulus.
pub(crate) fn cube3d_from_proto(
    p: proto::Cube3DParams,
) -> Result<(Mesh3dGeometry, Material3D, Option<String>), Refusal> {
    Ok((
        Mesh3dGeometry::Cube {
            size_cm: cube_size_from_proto(p.size_cm)?,
        },
        material3d_from_proto(p.material)?,
        texture_path_from_proto(p.texture_path)?,
    ))
}

pub(crate) fn sphere3d_from_proto(
    p: proto::Sphere3DParams,
) -> Result<(Mesh3dGeometry, Material3D, Option<String>), Refusal> {
    Ok((
        Mesh3dGeometry::Sphere {
            diameter_cm: sphere_diameter_from_proto(p.diameter_cm)?,
            rings: tessellation(p.rings, DEFAULT_SPHERE_RINGS, "rings")?,
            sectors: tessellation(p.sectors, DEFAULT_SPHERE_SECTORS, "sectors")?,
        },
        material3d_from_proto(p.material)?,
        texture_path_from_proto(p.texture_path)?,
    ))
}

pub(crate) fn plane3d_from_proto(
    p: proto::Plane3DParams,
) -> Result<(Mesh3dGeometry, Material3D, Option<String>), Refusal> {
    Ok((
        Mesh3dGeometry::Plane {
            size_cm: plane_size_from_proto(p.size_cm)?,
        },
        material3d_from_proto(p.material)?,
        texture_path_from_proto(p.texture_path)?,
    ))
}

/// The query `params` arm for a 3-D stimulus.
pub(crate) fn mesh3d_params_to_proto(m: &Mesh3d) -> proto::stimulus_params::Shape {
    let material = Some(material3d_to_proto(&m.material.live));
    let texture_path = m.texture_path.clone().unwrap_or_default();
    match m.geometry.live {
        Mesh3dGeometry::Cube { size_cm } => {
            proto::stimulus_params::Shape::Cube3d(proto::Cube3DParams {
                size_cm: Some(vec3_to_proto(Vec3::from(size_cm))),
                material,
                texture_path,
            })
        }
        Mesh3dGeometry::Sphere {
            diameter_cm,
            rings,
            sectors,
        } => proto::stimulus_params::Shape::Sphere3d(proto::Sphere3DParams {
            diameter_cm,
            rings,
            sectors,
            material,
            texture_path,
        }),
        Mesh3dGeometry::Plane { size_cm: [x, y] } => {
            proto::stimulus_params::Shape::Plane3d(proto::Plane3DParams {
                size_cm: Some(proto::Vec2 { x, y }),
                material,
                texture_path,
            })
        }
    }
}
