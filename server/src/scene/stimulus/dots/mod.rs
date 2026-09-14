pub mod dots_params;
pub mod dots_pipeline;
pub mod dots_rng;
pub mod dots_stimulus;
pub mod dots_tess;

pub use dots_params::{
    Aperture, ApertureClip, CoherenceCount, DotShape, DotsParams, NoiseRule, Region, RegionShape,
    Reinsertion, SignalRule,
};
pub use dots_pipeline::{DotInstance, DotsPushConstants, VkDotsPipeline};
pub use dots_rng::DotsRng;
pub use dots_stimulus::{Dots, DotsConfig, build_dots_push_constants};
