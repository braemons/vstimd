//! Procedural 3-D primitives: cube, sphere, plane.
//!
//! The scene side of Phase B (`dev/3D_ROADMAP.md`). Rendering (#70) and the
//! wire commands (#72) build on it.
//!
//! ## Why one struct rather than `Cube3D` / `Sphere3D` / `Plane3D`
//!
//! §9.1 of the roadmap lists these as separate `Stimulus` variants. They should
//! not be. Per §1.6 the mesh cache is keyed by *geometry*, not by stimulus
//! handle, so a cube and a sphere differ only in a `MeshKey` plus a nominal size_cm
//! folded into the model matrix — exactly the relationship rect/ellipse/circle
//! have, which is why they share [`Shape`](super::Shape). They also share a
//! pipeline (`mesh3d_pipeline`), a push-constant layout (§B.5), a texture cache
//! and a dirty/upload lifecycle: all four tests for a collapse.
//!
//! The payoff is that adding `Cylinder3D` later is one [`Mesh3dGeometry`] arm
//! plus one tessellator, touching no render pass and no exhaustive match
//! outside this file — rather than the eight new match arms §9.1's flat list
//! would require.

use super::stimulus_type::StimulusType;
use super::transform3d::{Material3D, Transform3D};
use crate::scene::deferred::Deferred;

/// A procedurally generated 3-D primitive, drawn in the 3-D pass with depth
/// test and back-face culling.
///
/// Carries no `mesh_id` / `texture_id`. GPU resources are render-thread-private
/// and belong in `SceneCache`, keyed by [`Mesh3dGeometry::mesh_key`] for meshes
/// and by path for textures — a stored id would be a second source of truth that
/// can disagree with the geometry it describes. (§D.2 and §E.3 of the roadmap
/// put `mesh_id` / `skin_id` / `scene_id` on the stimulus struct; that is the one
/// thing the config/runtime split rejects outright.)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Mesh3d {
    pub transform: Deferred<Transform3D>,
    pub material: Deferred<Material3D>,
    pub geometry: Deferred<Mesh3dGeometry>,
    /// Server-side filesystem path; `None` = untextured. Decoded on the ZMQ
    /// thread at create time (§B.6) — the render thread must never block or
    /// heap-allocate. Untextured stimuli bind a 1×1 white texture so textured
    /// and untextured share one pipeline and one descriptor set layout.
    pub texture_path: Option<String>,
}

/// Which primitive, and its nominal size_cm.
///
/// Sizes are **full extents** in centimetres, matching the 2-D convention that
/// `CreateRect{width, height}` and the saved `"size_cm"` are the same numbers. The
/// unit cube is 2 units across, so the halving happens when building the model
/// matrix — never in the API or the config, and never as a `half_size` field
/// (that split is what the v3 config format removed).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Mesh3dGeometry {
    Cube {
        /// Full extents, cm.
        size_cm: [f32; 3],
    },
    Sphere {
        /// Full extent across, cm — the same convention `Circle` uses, so every
        /// geometry in the scene is sized by its full extent.
        diameter_cm: f32,
        /// Tessellation quality — the only fields that select geometry, hence
        /// the only ones in the [`MeshKey`].
        rings: u32,
        sectors: u32,
    },
    /// Bounded quad. Arrives with the corridor (Phase C), which builds floors
    /// and walls from many instances of this one mesh.
    Plane {
        /// Full extents, cm.
        size_cm: [f32; 2],
    },
}

impl Default for Mesh3dGeometry {
    fn default() -> Self {
        Self::Cube { size_cm: [10.0; 3] }
    }
}

/// Identifies a *shared* unit mesh in the render thread's `Mesh3dCache`.
///
/// Keyed by geometry rather than by stimulus handle (§1.6) — the one decision in
/// Phase A/B that is expensive to reverse. Every other cache in the codebase
/// (`SolidMeshCache`, `TextMeshCache`) keys by handle; copying that here would
/// make a corridor of N tiles allocate N identical vertex buffers.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum MeshKey {
    Cube,
    Sphere { rings: u32, sectors: u32 },
    Plane,
}

impl Mesh3dGeometry {
    /// Which user-facing type this geometry is — the same many-to-one hop
    /// [`ShapeGeometry::stimulus_type`](super::ShapeGeometry::stimulus_type) makes,
    /// out of the one [`StimulusBody::Mesh3d`](super::StimulusBody::Mesh3d) arm.
    pub fn stimulus_type(&self) -> StimulusType {
        match self {
            Self::Cube { .. } => StimulusType::Cube3D,
            Self::Sphere { .. } => StimulusType::Sphere3D,
            Self::Plane { .. } => StimulusType::Plane3D,
        }
    }

    /// The **user-facing** type name, as it appears in the config's `geometry.type`
    /// tag. See [`ShapeGeometry::type_name`](super::ShapeGeometry::type_name).
    pub fn type_name(&self) -> &'static str {
        self.stimulus_type().type_name()
    }

    /// The shared-mesh cache key. Only tessellation-affecting fields
    /// participate: `size_cm` and `diameter_cm` fold into the model matrix, so a resize
    /// does **not** re-tessellate, and a screen resize does not invalidate 3-D
    /// meshes at all (unlike 2-D, whose vertices are baked to NDC).
    pub fn mesh_key(&self) -> MeshKey {
        match *self {
            Self::Cube { .. } => MeshKey::Cube,
            Self::Sphere { rings, sectors, .. } => MeshKey::Sphere { rings, sectors },
            Self::Plane { .. } => MeshKey::Plane,
        }
    }

    /// The size to fold into the model matrix as scale, so the shared mesh stays
    /// a unit primitive.
    ///
    /// The unit meshes in `render::tess3d` span `[-1, 1]` on every axis they
    /// have — 2 units across — so a full extent of `size_cm` is a scale of
    /// `size_cm / 2`. That halving happens here and nowhere else. The plane is
    /// flat in XZ, so its Y scale is 1 (it has no thickness to scale).
    pub fn model_scale(&self) -> glam::Vec3 {
        match *self {
            Self::Cube { size_cm } => glam::Vec3::from(size_cm) * 0.5,
            Self::Sphere { diameter_cm, .. } => glam::Vec3::splat(diameter_cm * 0.5),
            Self::Plane { size_cm: [w, d] } => glam::Vec3::new(w * 0.5, 1.0, d * 0.5),
        }
    }
}

impl Mesh3d {
    pub fn new(
        transform: Transform3D,
        material: Material3D,
        geometry: Mesh3dGeometry,
        texture_path: Option<String>,
    ) -> Self {
        Self {
            transform: Deferred::new(transform),
            material: Deferred::new(material),
            geometry: Deferred::new(geometry),
            texture_path,
        }
    }

    pub fn make_copy(&mut self) {
        self.transform.make_copy();
        self.material.make_copy();
        self.geometry.make_copy();
    }

    pub fn flip(&mut self) {
        self.transform.flip();
        self.material.flip();
        self.geometry.flip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn model_scale_halves_full_extents() {
        let cube = Mesh3dGeometry::Cube {
            size_cm: [20.0, 10.0, 4.0],
        };
        assert_eq!(cube.model_scale(), Vec3::new(10.0, 5.0, 2.0));
        let sphere = Mesh3dGeometry::Sphere {
            diameter_cm: 30.0,
            rings: 16,
            sectors: 32,
        };
        assert_eq!(sphere.model_scale(), Vec3::splat(15.0));
        let plane = Mesh3dGeometry::Plane {
            size_cm: [100.0, 40.0],
        };
        assert_eq!(plane.model_scale(), Vec3::new(50.0, 1.0, 20.0));
    }

    #[test]
    fn size_does_not_change_the_mesh_key() {
        let a = Mesh3dGeometry::Sphere {
            diameter_cm: 1.0,
            rings: 16,
            sectors: 32,
        };
        let b = Mesh3dGeometry::Sphere {
            diameter_cm: 50.0,
            rings: 16,
            sectors: 32,
        };
        assert_eq!(a.mesh_key(), b.mesh_key());
        let c = Mesh3dGeometry::Sphere {
            diameter_cm: 1.0,
            rings: 8,
            sectors: 32,
        };
        assert_ne!(a.mesh_key(), c.mesh_key());
    }
}
