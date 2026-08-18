//! Procedural 3-D primitives: cube, sphere, plane.
//!
//! **Placeholder — Phase B of `dev/3D_ROADMAP.md`.** The type exists; nothing
//! tessellates, uploads or draws it, and no command constructs one, so every
//! arm reachable only from a live 3-D stimulus is `unimplemented!()`.
//!
//! ## Why one struct rather than `Cube3D` / `Sphere3D` / `Plane3D`
//!
//! §9.1 of the roadmap lists these as separate `Stimulus` variants. They should
//! not be. Per §1.6 the mesh cache is keyed by *geometry*, not by stimulus
//! handle, so a cube and a sphere differ only in a `MeshKey` plus a nominal size
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

/// Which primitive, and its nominal size.
///
/// Sizes are **full extents** in centimetres, matching the 2-D convention that
/// `CreateRect{width, height}` and the saved `"size"` are the same numbers. The
/// unit cube is 2 units across, so the halving happens when building the model
/// matrix — never in the API or the config, and never as a `half_size` field
/// (that split is what the v3 config format removed).
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Mesh3dGeometry {
    Cube {
        /// Full extents, cm.
        size: [f32; 3],
    },
    Sphere {
        /// Full extent across, cm — the same convention `Circle` uses, so every
        /// geometry in the scene is sized by its full extent.
        diameter: f32,
        /// Tessellation quality — the only fields that select geometry, hence
        /// the only ones in the [`MeshKey`].
        rings: u32,
        sectors: u32,
    },
    /// Bounded quad. Arrives with the corridor (Phase C), which builds floors
    /// and walls from many instances of this one mesh.
    Plane {
        /// Full extents, cm.
        size: [f32; 2],
    },
}

impl Default for Mesh3dGeometry {
    fn default() -> Self {
        Self::Cube { size: [10.0; 3] }
    }
}

/// Identifies a *shared* unit mesh in the render thread's `Mesh3dCache`.
///
/// Keyed by geometry rather than by stimulus handle (§1.6) — the one decision in
/// Phase A/B that is expensive to reverse. Every other cache in the codebase
/// (`SolidMeshCache`, `TextMeshCache`) keys by handle; copying that here would
/// make a corridor of N tiles allocate N identical vertex buffers.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
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
    /// participate: `size` and `diameter` fold into the model matrix, so a resize
    /// does **not** re-tessellate, and a screen resize does not invalidate 3-D
    /// meshes at all (unlike 2-D, whose vertices are baked to NDC).
    pub fn mesh_key(&self) -> MeshKey {
        match *self {
            Self::Cube { .. } => MeshKey::Cube,
            Self::Sphere { rings, sectors, .. } => MeshKey::Sphere { rings, sectors },
            Self::Plane { .. } => MeshKey::Plane,
        }
    }

    /// The nominal size to fold into the model matrix as scale, so the shared
    /// mesh can stay a *unit* primitive.
    ///
    /// Unimplemented pending the tessellator it has to agree with: the halving
    /// convention (`size * 0.5` for the 2-units-across unit cube) is only
    /// correct relative to how `tess3d` emits the unit geometry, and writing one
    /// without the other bakes in a factor-of-two nobody can later locate.
    pub fn model_scale(&self) -> [f32; 3] {
        unimplemented!("Phase B: unit-mesh scale — see dev/3D_ROADMAP.md §1.7, §B.3, §B.6")
    }
}

impl Mesh3d {
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
