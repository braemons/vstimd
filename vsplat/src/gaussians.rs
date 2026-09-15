//! A splat scene with every trained parameter kept, for tools that edit scenes
//! rather than draw them.
//!
//! [`crate::load`] reduces each splat to what the shader needs ([`crate::GpuSplat`]:
//! a covariance and an RGBA8 colour) and cannot be written back. [`Gaussians`]
//! keeps the parameters as trained — log-scales, the quaternion, the opacity
//! logit and the spherical-harmonic coefficients — so a scene can be moved,
//! cropped and saved without losing anything.
//!
//! PLY only on input: it is what trainers export. Output is PLY (the reference
//! layout, readable by every viewer) or `.splat` (degree-0 colour only).

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use glam::{Mat3, Quat, Vec3};

use crate::splat_file::{SH_C0, SplatFileError, invalid, open, read_ply_header};

/// One Gaussian as trained. The higher SH bands live in [`Gaussians::sh_rest`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaussian {
    pub position: Vec3,
    /// Natural log of the standard deviation along each local axis.
    pub log_scale: Vec3,
    pub rotation: Quat,
    /// Opacity before the sigmoid.
    pub opacity_logit: f32,
    /// Degree-0 SH coefficient per channel: colour is `0.5 + C0 · f_dc`.
    pub f_dc: [f32; 3],
}

impl Gaussian {
    pub fn opacity(&self) -> f32 {
        1.0 / (1.0 + (-self.opacity_logit).exp())
    }

    /// Product of the three standard deviations — proportional to the volume.
    pub fn volume(&self) -> f32 {
        (self.log_scale.x + self.log_scale.y + self.log_scale.z).exp()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gaussians {
    pub splats: Vec<Gaussian>,
    /// 0–3. The number of `f_rest` coefficients per splat follows from it.
    pub sh_degree: u32,
    /// `rest_len()` numbers per splat, in PLY order: all of red's bands, then
    /// green's, then blue's (`f_rest_0 … f_rest_{3k−1}`).
    pub sh_rest: Vec<f32>,
}

/// `f_rest` coefficients per splat at `degree`: three channels of `(d+1)² − 1`.
pub fn rest_len_for(degree: u32) -> usize {
    3 * (((degree + 1) * (degree + 1)) as usize - 1)
}

impl Gaussians {
    pub fn len(&self) -> usize {
        self.splats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.splats.is_empty()
    }

    pub fn rest_len(&self) -> usize {
        rest_len_for(self.sh_degree)
    }

    /// Read a Gaussian splat PLY with all of its parameters.
    pub fn read_ply(path: &Path) -> Result<Self, SplatFileError> {
        let file = open(path)?;
        let io = |e: std::io::Error| SplatFileError::Io(format!("{}: {e}", path.display()));
        let mut reader = BufReader::with_capacity(1 << 20, file);
        let header = read_ply_header(path, &mut reader)?;

        let mut rest: Vec<(usize, usize)> = Vec::new();
        for (name, offset, is_f32) in &header.props {
            if let Some(i) = name
                .strip_prefix("f_rest_")
                .and_then(|i| i.parse::<usize>().ok())
            {
                if !is_f32 {
                    return Err(invalid(path, format!("property {name} must be float")));
                }
                rest.push((i, *offset));
            }
        }
        rest.sort_unstable();
        let sh_degree = (0..=3)
            .find(|&d| rest_len_for(d) == rest.len())
            .ok_or_else(|| {
                invalid(
                    path,
                    format!("{} f_rest properties fit no SH degree", rest.len()),
                )
            })?;
        if rest.iter().enumerate().any(|(k, (i, _))| k != *i) {
            return Err(invalid(path, "f_rest properties are not numbered 0…n−1"));
        }

        let count = header.count as usize;
        let mut out = Gaussians {
            splats: Vec::with_capacity(count),
            sh_degree,
            sh_rest: Vec::with_capacity(count * rest.len()),
        };
        let mut record = vec![0u8; header.record_bytes];
        for _ in 0..count {
            reader.read_exact(&mut record).map_err(io)?;
            let r = &record;
            let f = |i: usize| f32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
            out.splats.push(Gaussian {
                position: Vec3::from_array(header.position.map(f)),
                log_scale: Vec3::from_array(header.scale.map(f)),
                rotation: Quat::from_xyzw(
                    f(header.rot[1]),
                    f(header.rot[2]),
                    f(header.rot[3]),
                    f(header.rot[0]),
                ),
                opacity_logit: f(header.opacity),
                f_dc: header.f_dc.map(f),
            });
            out.sh_rest.extend(rest.iter().map(|&(_, off)| f(off)));
        }
        Ok(out)
    }

    /// Write the 3DGS reference PLY layout (binary little-endian, all `float`).
    pub fn write_ply(&self, path: &Path) -> std::io::Result<()> {
        let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
        let mut header = format!(
            "ply\nformat binary_little_endian 1.0\nelement vertex {}\n",
            self.len()
        );
        let mut prop = |name: &str| header += &format!("property float {name}\n");
        for name in ["x", "y", "z", "f_dc_0", "f_dc_1", "f_dc_2"] {
            prop(name);
        }
        for i in 0..self.rest_len() {
            prop(&format!("f_rest_{i}"));
        }
        for name in [
            "opacity", "scale_0", "scale_1", "scale_2", "rot_0", "rot_1", "rot_2", "rot_3",
        ] {
            prop(name);
        }
        header += "end_header\n";
        w.write_all(header.as_bytes())?;

        let rest_len = self.rest_len();
        for (i, g) in self.splats.iter().enumerate() {
            let q = g.rotation;
            let mut put = |v: f32| w.write_all(&v.to_le_bytes());
            for v in g.position.to_array().into_iter().chain(g.f_dc) {
                put(v)?;
            }
            for &v in &self.sh_rest[i * rest_len..(i + 1) * rest_len] {
                put(v)?;
            }
            put(g.opacity_logit)?;
            for v in g
                .log_scale
                .to_array()
                .into_iter()
                .chain([q.w, q.x, q.y, q.z])
            {
                put(v)?;
            }
        }
        w.flush()
    }

    /// Write antimatter15's `.splat` layout. Only degree-0 colour survives.
    pub fn write_splat(&self, path: &Path) -> std::io::Result<()> {
        let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let unit = |v: f32| (v * 128.0 + 128.0).round().clamp(0.0, 255.0) as u8;
        for g in &self.splats {
            for v in g
                .position
                .to_array()
                .into_iter()
                .chain(g.log_scale.exp().to_array())
            {
                w.write_all(&v.to_le_bytes())?;
            }
            let dc = g.f_dc.map(|c| byte(0.5 + SH_C0 * c));
            let q = g.rotation.normalize();
            w.write_all(&[dc[0], dc[1], dc[2], byte(g.opacity())])?;
            w.write_all(&[unit(q.w), unit(q.x), unit(q.y), unit(q.z)])?;
        }
        w.flush()
    }

    /// Drop the SH bands above `degree`. A no-op when the scene has fewer.
    pub fn truncate_sh(&mut self, degree: u32) {
        if degree >= self.sh_degree {
            return;
        }
        let (old, new) = (self.rest_len() / 3, rest_len_for(degree) / 3);
        let mut rest = Vec::with_capacity(self.len() * new * 3);
        for coeffs in self.sh_rest.chunks_exact(old * 3) {
            for channel in coeffs.chunks_exact(old) {
                rest.extend_from_slice(&channel[..new]);
            }
        }
        self.sh_rest = rest;
        self.sh_degree = degree;
    }

    /// Keep the splats `keep` accepts, in order, with their SH coefficients.
    pub fn retain(&mut self, mut keep: impl FnMut(usize, &Gaussian) -> bool) {
        let rest_len = self.rest_len();
        let mut j = 0;
        for i in 0..self.splats.len() {
            if keep(i, &self.splats[i]) {
                self.splats[j] = self.splats[i];
                self.sh_rest
                    .copy_within(i * rest_len..(i + 1) * rest_len, j * rest_len);
                j += 1;
            }
        }
        self.splats.truncate(j);
        self.sh_rest.truncate(j * rest_len);
    }

    /// Apply `x' = scale · rotation · x + translation` to the whole scene:
    /// positions, orientations, sizes and view-dependent colour.
    ///
    /// Degree-0 colour does not depend on direction and degree 1 rotates like
    /// a vector; degrees 2 and 3 would need Wigner rotation matrices, which are
    /// not implemented, so a scene with them is refused — truncate it first.
    pub fn apply_similarity(
        &mut self,
        scale: f32,
        rotation: Quat,
        translation: Vec3,
    ) -> Result<(), String> {
        if self.sh_degree > 1 {
            return Err(format!(
                "rotating SH degree {} is not supported; truncate to degree 1 or 0 first",
                self.sh_degree
            ));
        }
        let rotation = rotation.normalize();
        let ln_scale = scale.ln();
        for g in &mut self.splats {
            g.position = scale * (rotation * g.position) + translation;
            g.rotation = (rotation * g.rotation).normalize();
            g.log_scale += Vec3::splat(ln_scale);
        }
        if self.sh_degree == 1 {
            let r = Mat3::from_quat(rotation);
            for c in self.sh_rest.as_chunks_mut::<9>().0 {
                for ch in 0..3 {
                    let [c0, c1, c2] = [c[ch * 3], c[ch * 3 + 1], c[ch * 3 + 2]];
                    let v = r * sh1_vector(c0, c1, c2);
                    [c[ch * 3], c[ch * 3 + 1], c[ch * 3 + 2]] = sh1_coeffs(v);
                }
            }
        }
        Ok(())
    }
}

/// Band 1 evaluates as `C1 · (−y·c0 + z·c1 − x·c2)` for a view direction
/// `(x, y, z)`, i.e. `C1 · dot(v, d)` with this `v`. Rotating the scene rotates `v`.
fn sh1_vector(c0: f32, c1: f32, c2: f32) -> Vec3 {
    Vec3::new(-c2, -c0, c1)
}

fn sh1_coeffs(v: Vec3) -> [f32; 3] {
    [-v.y, v.z, -v.x]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vsplat-gaussians-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn scene(degree: u32, n: usize) -> Gaussians {
        let splats = (0..n)
            .map(|i| {
                let f = i as f32;
                Gaussian {
                    position: Vec3::new(f, 2.0 * f, -f),
                    log_scale: Vec3::new(-1.0, -2.0, 0.5 * f),
                    rotation: Quat::from_rotation_y(0.3 * f),
                    opacity_logit: f - 1.0,
                    f_dc: [0.1 * f, -0.2, 0.3],
                }
            })
            .collect();
        let rest = rest_len_for(degree);
        Gaussians {
            splats,
            sh_degree: degree,
            sh_rest: (0..n * rest).map(|i| i as f32 * 0.01).collect(),
        }
    }

    #[test]
    fn ply_round_trips_every_parameter() {
        for degree in 0..=3 {
            let g = scene(degree, 5);
            let path = temp(&format!("rt{degree}.ply"));
            g.write_ply(&path).unwrap();
            assert_eq!(Gaussians::read_ply(&path).unwrap(), g, "degree {degree}");
            // And the renderer's reader agrees on count and position.
            let gpu = crate::load(&path).unwrap();
            assert_eq!(gpu.len(), 5);
            assert_eq!(gpu[3].position, [3.0, 6.0, -3.0]);
        }
    }

    #[test]
    fn splat_output_is_readable_by_the_renderer() {
        let g = scene(1, 3);
        let path = temp("out.splat");
        g.write_splat(&path).unwrap();
        let gpu = crate::load(&path).unwrap();
        assert_eq!(gpu.len(), 3);
        assert_eq!(gpu[2].position, [2.0, 4.0, -2.0]);
    }

    #[test]
    fn truncate_keeps_the_low_bands_of_each_channel() {
        let mut g = scene(3, 2);
        let before = g.sh_rest.clone();
        g.truncate_sh(1);
        assert_eq!(g.sh_rest.len(), 2 * 9);
        // Splat 1, green channel: offset 45 + 15 in the old layout, 9 + 3 in the new.
        assert_eq!(g.sh_rest[9 + 3..9 + 6], before[45 + 15..45 + 18]);
    }

    #[test]
    fn retain_moves_coefficients_with_their_splat() {
        let mut g = scene(1, 4);
        let kept = g.sh_rest[27..36].to_vec();
        g.retain(|i, _| i == 3);
        assert_eq!(g.len(), 1);
        assert_eq!(g.sh_rest, kept);
    }

    #[test]
    fn similarity_moves_scales_and_rotates_covariance() {
        let mut g = scene(0, 3);
        let before = g.clone();
        let r = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        g.apply_similarity(10.0, r, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        for (a, b) in before.splats.iter().zip(&g.splats) {
            assert!((b.position - (10.0 * (r * a.position) + Vec3::X)).length() < 1e-4);
            // Σ' = s² R Σ Rᵀ
            let cov = |g: &Gaussian| {
                let m = Mat3::from_quat(g.rotation) * Mat3::from_diagonal(g.log_scale.exp());
                m * m.transpose()
            };
            let rm = Mat3::from_quat(r);
            let want = 100.0 * rm * cov(a) * rm.transpose();
            let got = cov(b);
            for (w, g) in want.to_cols_array().iter().zip(got.to_cols_array()) {
                assert!((w - g).abs() < 1e-3 * w.abs().max(1.0), "{want} vs {got}");
            }
        }
    }

    #[test]
    fn degree_one_colour_follows_the_rotation() {
        // Colour seen along d before rotating must be seen along R·d after.
        let eval = |c: &[f32], d: Vec3| -> f32 { -d.y * c[0] + d.z * c[1] - d.x * c[2] };
        let mut g = scene(1, 1);
        g.sh_rest = vec![0.3, -0.7, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let before = g.sh_rest.clone();
        let r = Quat::from_euler(glam::EulerRot::YXZ, 0.4, -1.1, 0.25);
        g.apply_similarity(1.0, r, Vec3::ZERO).unwrap();
        for d in [Vec3::X, Vec3::Y, Vec3::new(0.3, -0.5, 0.8).normalize()] {
            let a = eval(&before[0..3], d);
            let b = eval(&g.sh_rest[0..3], r * d);
            assert!((a - b).abs() < 1e-5, "{a} vs {b}");
        }
    }

    #[test]
    fn higher_degrees_are_refused_rather_than_rotated_wrong() {
        let mut g = scene(2, 1);
        assert!(g.apply_similarity(1.0, Quat::IDENTITY, Vec3::ZERO).is_err());
    }
}
