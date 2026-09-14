//! Unit 3-D primitives, tessellated once and shared by every instance
//! (`dev/3D_ROADMAP.md` §1.6). Size, placement and rotation are the model
//! matrix's job, never baked into vertices.
//!
//! Winding is counter-clockwise seen from outside, matching the mesh3d
//! pipeline's `front_face = COUNTER_CLOCKWISE, cull_mode = BACK`.

use glam::Vec3;

use crate::Color;
use crate::geom::Vertex;

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
}
