//! Gaussian splat data off the GPU: reading and writing scene files, and
//! sorting splats.
//!
//! A crate because it has two users, as `vtl` and `vinput` do:
//! - **vstimd** (as `vstimd::splat`): the ZMQ thread probes a file when a
//!   `GaussianSplat3D` is created, and the render thread loads and sorts it.
//!   The GPU half is `render::vk::cache::SplatCache`.
//! - **vstimd-scene-from-capture**: reads a trained scene with every parameter kept
//!   ([`Gaussians`]), moves it into vstimd's world and writes it back.

mod gaussians;
mod splat_file;
mod splat_sort;

pub use gaussians::{Gaussian, Gaussians, rest_len_for};
pub use splat_file::{SplatFileError, SplatFileInfo, SplatFormat, load, probe};
pub use splat_sort::SplatSorter;

use glam::{Mat3, Quat, Vec3};

/// One splat as the shader reads it. Must match `struct Splat` in
/// `shaders/splat.wgsl` (storage buffer, 48 bytes).
///
/// The 3-D covariance is precomputed from scale and rotation at load, so the
/// vertex shader only projects it; it is symmetric, so six numbers carry it.
/// Colour is RGBA8 packed little-endian (`unpack4x8unorm` in the shader), alpha
/// being the splat's opacity.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuSplat {
    pub position: [f32; 3],
    pub color: u32,
    /// `[xx, xy, xz]`
    pub cov_a: [f32; 3],
    pub _pad0: f32,
    /// `[yy, yz, zz]`
    pub cov_b: [f32; 3],
    pub _pad1: f32,
}

impl GpuSplat {
    /// `scale` is the standard deviation along each local axis (already
    /// exponentiated), `rotation` need not be normalised, `rgba` is `[0, 1]`.
    pub fn new(position: Vec3, scale: Vec3, rotation: Quat, rgba: [f32; 4]) -> Self {
        let r = Mat3::from_quat(rotation.normalize());
        let m = r * Mat3::from_diagonal(scale);
        let cov = m * m.transpose();
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        Self {
            position: position.to_array(),
            color: byte(rgba[0]) | byte(rgba[1]) << 8 | byte(rgba[2]) << 16 | byte(rgba[3]) << 24,
            cov_a: [cov.x_axis.x, cov.y_axis.x, cov.z_axis.x],
            _pad0: 0.0,
            cov_b: [cov.y_axis.y, cov.z_axis.y, cov.z_axis.z],
            _pad1: 0.0,
        }
    }

    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_the_shader() {
        assert_eq!(std::mem::size_of::<GpuSplat>(), 48);
        assert_eq!(std::mem::offset_of!(GpuSplat, color), 12);
        assert_eq!(std::mem::offset_of!(GpuSplat, cov_a), 16);
        assert_eq!(std::mem::offset_of!(GpuSplat, cov_b), 32);
    }

    #[test]
    fn covariance_is_scale_squared_for_an_unrotated_splat() {
        let s = GpuSplat::new(
            Vec3::ZERO,
            Vec3::new(1.0, 2.0, 3.0),
            Quat::IDENTITY,
            [1.0; 4],
        );
        assert_eq!(s.cov_a, [1.0, 0.0, 0.0]);
        assert_eq!(s.cov_b, [4.0, 0.0, 9.0]);
    }

    #[test]
    fn rotation_moves_variance_between_axes() {
        // A long X axis turned 90° about Z lies along Y.
        let q = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let s = GpuSplat::new(Vec3::ZERO, Vec3::new(3.0, 1.0, 1.0), q, [1.0; 4]);
        assert!((s.cov_a[0] - 1.0).abs() < 1e-4, "xx = {}", s.cov_a[0]);
        assert!((s.cov_b[0] - 9.0).abs() < 1e-4, "yy = {}", s.cov_b[0]);
    }

    #[test]
    fn colour_packs_red_in_the_low_byte() {
        let s = GpuSplat::new(Vec3::ZERO, Vec3::ONE, Quat::IDENTITY, [1.0, 0.0, 0.5, 0.0]);
        assert_eq!(s.color, 0x0080_00ff);
    }
}
