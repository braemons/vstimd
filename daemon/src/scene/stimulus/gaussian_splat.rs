//! A trained 3-D Gaussian splat scene, placed in world space.
//!
//! See `dev/design/GAUSSIAN_SPLAT_PLAN.md`. The stimulus is only the file and
//! its placement: the splats themselves are loaded, uploaded and sorted by the
//! render thread (`render::vk::cache::SplatCache`), keyed by stimulus handle,
//! because GPU resources never live in the scene tree.

use super::transform3d::Transform3D;
use crate::scene::deferred::Deferred;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct GaussianSplat {
    /// Scale is centimetres per scene unit: a trained scene has none of its own.
    pub transform: Deferred<Transform3D>,
    /// Server-side `.ply` or `.splat` file. Checked when the stimulus is
    /// created, loaded in the background when it is first drawn. Interim until
    /// the asset store gives it an asset reference. Fixed at creation.
    pub path: String,
}

impl GaussianSplat {
    pub fn new(transform: Transform3D, path: String) -> Self {
        Self {
            transform: Deferred::new(transform),
            path,
        }
    }

    pub fn make_copy(&mut self) {
        self.transform.make_copy();
    }

    pub fn flip(&mut self) {
        self.transform.flip();
    }
}
