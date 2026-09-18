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

use glam::{Mat4, Quat, Vec3};

use super::stimulus_type::StimulusType;
use super::transform3d::{Material3D, Transform3D};
use crate::Color;
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
    /// Draw copies of the stimulus every `period_cm` along world −Z, so an
    /// object recurs in every period of an endless corridor. Not deferred: it
    /// is fixed at creation, like a sphere's tessellation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat3D>,
}

/// Copies of a 3-D stimulus at `position + k · period_cm` along world −Z, for
/// `k` in `-behind..=ahead`. Copy 0 is the stimulus itself.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Repeat3D {
    pub period_cm: f32,
    pub ahead: u32,
    pub behind: u32,
}

/// An endless-corridor tile set: a floor, two walls in two-tone panels, an
/// optional ceiling, all instances of the shared unit plane.
///
/// In the stimulus' own frame the corridor starts at the origin and runs along
/// −Z, floor at `y = 0`, centred on `x = 0`. One period holds one floor tile and
/// two wall panels per side of alternating colour, so its appearance repeats
/// exactly every `period_cm` — which is what lets `LinearNav3D` wrap the camera
/// into `[0, period_cm)` without a seam. The stripes are also the motion cue:
/// flat walls give none.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CorridorParams {
    /// Full extents, cm.
    pub width_cm: f32,
    pub height_cm: f32,
    /// Length of one repeating period, cm.
    pub period_cm: f32,
    /// Periods drawn ahead of the origin (towards −Z) and behind it.
    pub periods_ahead: u32,
    pub periods_behind: u32,
    pub floor_color: Color,
    /// The two wall-panel colours, alternating every half period.
    pub wall_color: Color,
    pub stripe_color: Color,
    /// A ceiling in `floor_color`, facing down.
    pub ceiling: bool,
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
    /// Bounded quad in the XZ plane, facing +Y.
    Plane {
        /// Full extents, cm.
        size_cm: [f32; 2],
    },
    /// Many planes: see [`CorridorParams`].
    Corridor(CorridorParams),
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
            Self::Corridor(_) => StimulusType::Corridor3D,
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
            Self::Plane { .. } | Self::Corridor(_) => MeshKey::Plane,
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
            // A corridor scales each of its planes itself; see `for_each_instance`.
            Self::Corridor(_) => glam::Vec3::ONE,
        }
    }
}

/// Call `f(local_model, colour)` for every plane of one corridor.
fn corridor_planes(c: &CorridorParams, mut f: impl FnMut(Mat4, Color)) {
    let (w, h, l) = (c.width_cm, c.height_cm, c.period_cm);
    let behind = -(c.periods_behind as i64);
    let ahead = c.periods_ahead as i64;
    let wall = |x: f32, roll: f32, z: f32| {
        // The unit plane faces +Y; roll it about Z to face across the corridor,
        // its X extent becoming the wall's height.
        Mat4::from_scale_rotation_translation(
            Vec3::new(h * 0.5, 1.0, l * 0.25),
            Quat::from_rotation_z(roll),
            Vec3::new(x, h * 0.5, z),
        )
    };
    let half_pi = std::f32::consts::FRAC_PI_2;
    for i in behind..ahead {
        let i = i as f32;
        let z_mid = -(i + 0.5) * l;
        f(
            Mat4::from_scale_rotation_translation(
                Vec3::new(w * 0.5, 1.0, l * 0.5),
                Quat::IDENTITY,
                Vec3::new(0.0, 0.0, z_mid),
            ),
            c.floor_color,
        );
        if c.ceiling {
            f(
                Mat4::from_scale_rotation_translation(
                    Vec3::new(w * 0.5, 1.0, l * 0.5),
                    Quat::from_rotation_x(std::f32::consts::PI),
                    Vec3::new(0.0, h, z_mid),
                ),
                c.floor_color,
            );
        }
        for (panel, color) in [(0.25, c.wall_color), (0.75, c.stripe_color)] {
            let z = -(i + panel) * l;
            f(wall(-w * 0.5, -half_pi, z), color); // left wall faces +X
            f(wall(w * 0.5, half_pi, z), color); // right wall faces −X
        }
    }
}

impl Mesh3d {
    /// Call `f(model, colour)` for every instance this stimulus draws, from its
    /// live state: one for a primitive, a plane per tile face for a corridor,
    /// times the copies `repeat` asks for. `colour` multiplies the material's
    /// albedo — white for everything but corridor panels.
    ///
    /// A callback rather than a collection, so the render thread iterates it
    /// without allocating.
    pub fn for_each_instance(&self, mut f: impl FnMut(Mat4, Color)) {
        let t = self.transform.live;
        let geometry = self.geometry.live;
        let (period, behind, ahead) = match self.repeat {
            Some(r) => (r.period_cm, r.behind as i64, r.ahead as i64),
            None => (0.0, 0, 0),
        };
        for k in -behind..=ahead {
            let shift = Mat4::from_translation(Vec3::new(0.0, 0.0, -(k as f32) * period));
            match &geometry {
                Mesh3dGeometry::Corridor(c) => {
                    let base = shift * t.model_matrix(Vec3::ONE);
                    corridor_planes(c, |local, color| f(base * local, color));
                }
                g => f(shift * t.model_matrix(g.model_scale()), Color::WHITE),
            }
        }
    }

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
            repeat: None,
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

    fn corridor() -> CorridorParams {
        CorridorParams {
            width_cm: 60.0,
            height_cm: 40.0,
            period_cm: 100.0,
            periods_ahead: 3,
            periods_behind: 1,
            floor_color: Color::new(0.3, 0.3, 0.3, 1.0),
            wall_color: Color::new(0.6, 0.6, 0.6, 1.0),
            stripe_color: Color::new(0.2, 0.2, 0.2, 1.0),
            ceiling: true,
        }
    }

    fn collect(m: &Mesh3d) -> Vec<(Mat4, Color)> {
        let mut v = Vec::new();
        m.for_each_instance(|model, color| v.push((model, color)));
        v
    }

    fn mesh(geometry: Mesh3dGeometry) -> Mesh3d {
        Mesh3d::new(Transform3D::default(), Material3D::default(), geometry, None)
    }

    #[test]
    fn corridor_draws_floor_ceiling_and_four_panels_per_period() {
        let instances = collect(&mesh(Mesh3dGeometry::Corridor(corridor())));
        assert_eq!(instances.len(), 4 * (1 + 1 + 4));
    }

    #[test]
    fn corridor_faces_point_into_the_corridor() {
        for (model, _) in collect(&mesh(Mesh3dGeometry::Corridor(corridor()))) {
            let centre = model.transform_point3(Vec3::ZERO);
            let normal = model.transform_vector3(Vec3::Y).normalize();
            // Towards the corridor axis (x = 0, y = height / 2).
            let inward = Vec3::new(-centre.x, 20.0 - centre.y, 0.0).normalize();
            assert!(normal.dot(inward) > 0.99, "face at {centre} points {normal}");
        }
    }

    #[test]
    fn corridor_is_periodic() {
        // Shifting the whole set by one period maps it onto itself, apart from
        // the tiles at the two ends.
        let mut c = corridor();
        c.periods_behind = 0;
        c.periods_ahead = 5;
        let key = |m: Mat4, color: Color| {
            let p = m.transform_point3(Vec3::ZERO);
            ((p.x * 8.0) as i32, (p.y * 8.0) as i32, (p.z * 8.0) as i32, (color.r * 100.0) as i32)
        };
        let all: std::collections::HashSet<_> = collect(&mesh(Mesh3dGeometry::Corridor(c)))
            .into_iter()
            .map(|(m, col)| key(m, col))
            .collect();
        for (m, col) in collect(&mesh(Mesh3dGeometry::Corridor(c))) {
            let z = m.transform_point3(Vec3::ZERO).z;
            if z > -400.0 {
                let shifted = Mat4::from_translation(Vec3::new(0.0, 0.0, -100.0)) * m;
                assert!(all.contains(&key(shifted, col)), "no match one period ahead of z={z}");
            }
        }
    }

    #[test]
    fn repeat_draws_copies_one_period_apart() {
        let mut m = mesh(Mesh3dGeometry::Cube { size_cm: [10.0; 3] });
        m.repeat = Some(Repeat3D { period_cm: 100.0, ahead: 2, behind: 1 });
        let zs: Vec<f32> = collect(&m).iter().map(|(t, _)| t.transform_point3(Vec3::ZERO).z).collect();
        assert_eq!(zs, [100.0, 0.0, -100.0, -200.0]);
    }
}
