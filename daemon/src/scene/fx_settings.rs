//! Graphics-debugging knobs, driven by the overlay's FX panel.
//!
//! Render tuning, not scene content: never serialized into a scene-config, and
//! nothing whose result an experiment may depend on. The defaults are the
//! values the renderer used before the panel existed, so a rig nobody has
//! touched draws exactly what it drew before.
//!
//! These live here, in the scene's runtime state, because the overlay writes
//! them and the render thread reads them, and that is the pair of threads the
//! scene lock already exists to separate.

/// Splat-rendering knobs. See `shaders/splat.wgsl` for what each one does to a
/// drawn splat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxSettings {
    /// Alpha, in 1/255 units, below which a splat's fragment is dropped — and
    /// below which its quad is not drawn at all.
    ///
    /// 1 is exact: it draws every fragment that could still change the frame.
    /// Raising it is the one measured lever on splat cost, because a splat
    /// scene is limited by the fragments that *blend*, not by the ones that are
    /// discarded. At 2560×1440 on the MipNeRF-360 room capture, 8 cost about
    /// 29% less frame time for a 1% mean pixel difference.
    ///
    /// It changes what reaches the screen, so it is a knob and not a default.
    pub splat_alpha_floor: f32,
    /// Longest axis, in pixels, that one splat's quad may be drawn at. Bounds
    /// what a single splat close to the camera can cost.
    ///
    /// Measured weak on the MipNeRF-360 room capture: clamping all the way down
    /// to 32 px bought 5% (7.71 ms -> 7.34 ms) and 128 px bought nothing at all,
    /// because that scene's fill comes from many moderate splats rather than a
    /// few huge near ones. Kept because a scene *can* be the other shape -- a
    /// corridor trained from too few views grows large splats close in -- and
    /// this is the knob that finds out.
    pub splat_max_axis_px: f32,
}

impl Default for FxSettings {
    fn default() -> Self {
        Self {
            splat_alpha_floor: 1.0,
            splat_max_axis_px: 1024.0,
        }
    }
}

impl FxSettings {
    /// Inclusive slider range for [`splat_alpha_floor`](Self::splat_alpha_floor).
    pub const ALPHA_FLOOR_RANGE: std::ops::RangeInclusive<f32> = 1.0..=32.0;
    /// Inclusive slider range for [`splat_max_axis_px`](Self::splat_max_axis_px).
    pub const MAX_AXIS_RANGE: std::ops::RangeInclusive<f32> = 32.0..=1024.0;
}
