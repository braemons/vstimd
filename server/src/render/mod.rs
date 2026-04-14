pub mod vertex;
pub use vertex::Vertex;

pub(crate) mod tess;

// ── wgpu backend (default) ────────────────────────────────────────────────────
#[cfg(not(feature = "drm"))]
mod gpu_buffers;
#[cfg(all(not(feature = "drm"), feature = "overlay"))]
mod overlay;
#[cfg(not(feature = "drm"))]
mod pipeline;
#[cfg(not(feature = "drm"))]
mod state;
#[cfg(not(feature = "drm"))]
pub use state::RenderState;

// ── bare-metal Linux backend ──────────────────────────────────────────────────
#[cfg(feature = "drm")]
pub mod drm;
#[cfg(feature = "drm")]
pub use drm::RenderState;
