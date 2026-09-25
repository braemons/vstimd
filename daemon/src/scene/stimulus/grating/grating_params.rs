/// Waveform shape of the grating carrier.
#[derive(Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Waveform {
    #[default]
    Sin = 0,
    Sqr = 1,
    Saw = 2,
    Tri = 3,
}

/// Aperture mask applied over the grating patch.
#[derive(Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum GratingMask {
    #[default]
    None = 0,
    Circle = 1,
    Gauss = 2,
    /// Cosine bell: 0.5*(1+cos(π·r/R)). Tapers from centre all the way to the edge.
    Hann = 3,
    /// Tukey window: flat at 1 in the inner 80%, raised-cosine taper in the outer 20%.
    /// Matches PsychoPy's `mask='raisedCos'` (fringeWidth=0.2).
    RaisedCos = 4,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct GratingParams {
    pub sf_cycles_per_px: f32,       // cycles/pixel
    pub phase_cycles: f32,    // static phase offset [0, 1]
    pub contrast: f32, // [0, 1]
    pub waveform: Waveform,
    pub mask: GratingMask,
    /// Mask-specific parameter (0 = use default):
    /// - `Gauss`:     SD in normalized units where patch radius = 1 (default 1/3)
    /// - `RaisedCos`: fringe proportion [0, 1] (default 0.2)
    pub mask_param: f32,
    pub drift_speed_hz: f32, // cycles/second; negative reverses direction
    /// When true the drift direction equals the grating stripe orientation
    /// (perpendicular to the stripes).  When false `drift_angle_deg` is used instead.
    pub drift_coupled: bool,
    pub drift_angle_deg: f32, // degrees CCW; used only when !drift_coupled
    pub fore_color: crate::Color, // rgba peak colour (carrier = +1)
    pub back_color: crate::Color, // rgba trough colour (carrier = −1)
}

impl Default for GratingParams {
    fn default() -> Self {
        Self {
            sf_cycles_per_px: 0.05,
            phase_cycles: 0.0,
            contrast: 1.0,
            waveform: Waveform::Sin,
            mask: GratingMask::None,
            mask_param: 0.0,
            drift_speed_hz: 0.0,
            drift_coupled: true,
            drift_angle_deg: 0.0,
            fore_color: crate::Color::WHITE,
            back_color: crate::Color::BLACK,
        }
    }
}


