//! Unit 3-D primitives, tessellated once and shared by every instance
//! (`dev/3D_ROADMAP.md` §1.6). Size, placement and rotation are the model
//! matrix's job, never baked into vertices.
//!
//! Winding is counter-clockwise seen from outside, matching the mesh3d
//! pipeline's `front_face = COUNTER_CLOCKWISE, cull_mode = BACK`.

use glam::Vec3;

use crate::Color;
use crate::geom::Vertex;
use crate::scene::stimulus::MeshKey;

/// The shared unit mesh for `key`, white so the material's albedo tints it.
pub fn unit_mesh(key: MeshKey) -> (Vec<Vertex>, Vec<u32>) {
    match key {
        MeshKey::Cube => unit_cube(Color::WHITE),
        MeshKey::Sphere { rings, sectors } => unit_sphere(rings, sectors, Color::WHITE),
        MeshKey::Plane => unit_plane(Color::WHITE),
    }
}

/// The unit cube, `[-1, 1]³` — 2 units across, so a full extent `size_cm` scales
/// it by `size_cm / 2`. 24 vertices (4 per face, so each face has its own
/// normal) and 36 indices, faces in the order +X, −X, +Y, −Y, +Z, −Z.
pub fn unit_cube(color: Color) -> (Vec<Vertex>, Vec<u32>) {
    const FACES: [(Vec3, Vec3); 6] = [
        // (outward normal, face "up" in texture space)
        (Vec3::X, Vec3::Y),
        (Vec3::NEG_X, Vec3::Y),
        (Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::Z),
        (Vec3::Z, Vec3::Y),
        (Vec3::NEG_Z, Vec3::Y),
    ];
    let mut verts = Vec::with_capacity(24);
    let mut idxs = Vec::with_capacity(36);
    for (n, up) in FACES {
        // (right, up, n) is right-handed, so right × up = n: corners listed
        // (−,−) (+,−) (+,+) (−,+) run counter-clockwise seen from outside.
        let right = up.cross(n);
        let base = verts.len() as u32;
        for (s, t) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            verts.push(Vertex {
                position: (n + right * s + up * t).to_array(),
                normal: n.to_array(),
                uv: [(s + 1.0) * 0.5, (1.0 - t) * 0.5],
                color,
            });
        }
        idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (verts, idxs)
}

/// The unit UV sphere, radius 1 — 2 units across, like the cube.
///
/// `rings` latitude bands from the north pole (+Y) down, `sectors` longitude
/// slices. `u` runs east (towards +X from +Z), `v` runs south, so an
/// equirectangular texture sits upright.
///
/// The seam column is duplicated (`sector = 0` and `sector = sectors` share
/// positions but not `u`), or the texture would wrap backwards across one slice.
/// The pole bands are triangles, not zero-area quads. `rings` and `sectors` are
/// clamped to at least 2 and 3.
pub fn unit_sphere(rings: u32, sectors: u32, color: Color) -> (Vec<Vertex>, Vec<u32>) {
    let rings = rings.max(2);
    let sectors = sectors.max(3);
    let mut verts = Vec::with_capacity(((rings + 1) * (sectors + 1)) as usize);
    for r in 0..=rings {
        let v = r as f32 / rings as f32;
        let theta = v * std::f32::consts::PI;
        for s in 0..=sectors {
            let u = s as f32 / sectors as f32;
            let phi = u * std::f32::consts::TAU;
            let p = Vec3::new(
                theta.sin() * phi.sin(),
                theta.cos(),
                theta.sin() * phi.cos(),
            );
            verts.push(Vertex {
                position: p.to_array(),
                normal: p.to_array(),
                uv: [u, v],
                color,
            });
        }
    }
    let row = sectors + 1;
    let mut idxs = Vec::with_capacity((6 * sectors * (rings - 1)) as usize);
    for r in 0..rings {
        for s in 0..sectors {
            // a─d   a is (r, s); b is one ring south, d one sector east.
            // │ │
            // b─c
            let a = r * row + s;
            let (b, c, d) = (a + row, a + row + 1, a + 1);
            // b and c are both the south pole on the last band; a and d both
            // the north pole on the first. Skip the triangle that collapses.
            if r != rings - 1 {
                idxs.extend_from_slice(&[a, b, c]);
            }
            if r != 0 {
                idxs.extend_from_slice(&[a, c, d]);
            }
        }
    }
    (verts, idxs)
}

/// The unit plane: `[-1, 1]` in X and Z at `y = 0`, facing +Y. `u` along +X,
/// `v` along +Z, so a floor texture reads upright from a camera looking down −Z.
pub fn unit_plane(color: Color) -> (Vec<Vertex>, Vec<u32>) {
    let corner = |x: f32, z: f32, uv: [f32; 2]| Vertex {
        position: [x, 0.0, z],
        normal: [0.0, 1.0, 0.0],
        uv,
        color,
    };
    let verts = vec![
        corner(-1.0, 1.0, [0.0, 1.0]),
        corner(1.0, 1.0, [1.0, 1.0]),
        corner(1.0, -1.0, [1.0, 0.0]),
        corner(-1.0, -1.0, [0.0, 0.0]),
    ];
    (verts, vec![0, 1, 2, 0, 2, 3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_is_24_verts_36_indices_in_unit_bounds() {
        let (v, i) = unit_cube(Color::WHITE);
        assert_eq!((v.len(), i.len()), (24, 36));
        for p in v.iter().flat_map(|v| v.position) {
            assert_eq!(p.abs(), 1.0);
        }
    }

    #[test]
    fn cube_triangles_wind_ccw_from_outside() {
        let (v, i) = unit_cube(Color::WHITE);
        for tri in i.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(v[tri[k] as usize].position));
            let n = Vec3::from(v[tri[0] as usize].normal);
            let face_normal = (b - a).cross(c - a).normalize();
            assert!(
                face_normal.dot(n) > 0.999,
                "tri {tri:?}: {face_normal} vs {n}"
            );
            // The normal points away from the centre.
            assert!(((a + b + c) / 3.0).dot(n) > 0.0);
        }
    }

    /// Every triangle has area and faces away from the origin, `dir` giving
    /// "outward" at its centroid.
    fn assert_outward(v: &[Vertex], i: &[u32], dir: impl Fn(Vec3) -> Vec3) {
        for tri in i.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(v[tri[k] as usize].position));
            let cross = (b - a).cross(c - a);
            assert!(cross.length() > 1e-6, "degenerate triangle {tri:?}");
            let centroid = (a + b + c) / 3.0;
            assert!(cross.dot(dir(centroid)) > 0.0, "tri {tri:?} winds inward");
        }
    }

    #[test]
    fn sphere_counts_normals_and_winding() {
        let (rings, sectors) = (8, 12);
        let (v, i) = unit_sphere(rings, sectors, Color::WHITE);
        assert_eq!(v.len() as u32, (rings + 1) * (sectors + 1));
        assert_eq!(i.len() as u32, 6 * sectors * (rings - 1));
        for vert in &v {
            assert!((Vec3::from(vert.normal).length() - 1.0).abs() < 1e-5);
        }
        assert_outward(&v, &i, |c| c);
    }

    #[test]
    fn sphere_seam_is_duplicated() {
        let sectors = 12;
        let (v, _) = unit_sphere(8, sectors, Color::WHITE);
        let row = (sectors + 1) as usize;
        let equator = 4 * row;
        let (first, last) = (&v[equator], &v[equator + sectors as usize]);
        assert!((Vec3::from(first.position) - Vec3::from(last.position)).length() < 1e-5);
        assert_eq!((first.uv[0], last.uv[0]), (0.0, 1.0));
    }

    #[test]
    fn sphere_u_runs_east_and_v_runs_south() {
        let sectors = 12;
        let (v, _) = unit_sphere(8, sectors, Color::WHITE);
        let row = (sectors + 1) as usize;
        let at = |r: usize, s: usize| Vec3::from(v[r * row + s].position);
        assert!(at(4, 1).x > at(4, 0).x, "u increases towards +X from +Z");
        assert!(at(5, 0).y < at(4, 0).y, "v increases southwards");
    }

    #[test]
    fn plane_faces_up() {
        let (v, i) = unit_plane(Color::WHITE);
        assert_outward(&v, &i, |_| Vec3::Y);
    }

    #[test]
    fn cube_has_no_degenerate_triangles() {
        let (v, i) = unit_cube(Color::WHITE);
        assert_outward(&v, &i, |c| {
            // The dominant axis of the centroid is the face's outward direction.
            let a = c.abs();
            if a.x >= a.y && a.x >= a.z {
                Vec3::X * c.x.signum()
            } else if a.y >= a.z {
                Vec3::Y * c.y.signum()
            } else {
                Vec3::Z * c.z.signum()
            }
        });
    }
}
