//! Grating <-> proto conversions.

use super::color_or_default;
use crate::Color;
use crate::proto;

use crate::scene::stimulus::grating::{Grating, GratingMask, GratingParams, Waveform};

// ── Waveform conversions ──────────────────────────────────────────────────────

pub(crate) fn waveform_from_proto(v: i32) -> Waveform {
    match proto::WaveformType::try_from(v).unwrap_or(proto::WaveformType::Unspecified) {
        proto::WaveformType::Unspecified | proto::WaveformType::Sin => Waveform::Sin,
        proto::WaveformType::Sqr => Waveform::Sqr,
        proto::WaveformType::Saw => Waveform::Saw,
        proto::WaveformType::Tri => Waveform::Tri,
    }
}

pub(crate) fn waveform_to_proto(w: Waveform) -> proto::WaveformType {
    match w {
        Waveform::Sin => proto::WaveformType::Sin,
        Waveform::Sqr => proto::WaveformType::Sqr,
        Waveform::Saw => proto::WaveformType::Saw,
        Waveform::Tri => proto::WaveformType::Tri,
    }
}

// ── Mask conversions ──────────────────────────────────────────────────────────

pub(crate) fn mask_from_proto(v: i32) -> GratingMask {
    match proto::MaskType::try_from(v).unwrap_or(proto::MaskType::Unspecified) {
        proto::MaskType::Unspecified | proto::MaskType::None => GratingMask::None,
        proto::MaskType::Circle => GratingMask::Circle,
        proto::MaskType::Gauss => GratingMask::Gauss,
        proto::MaskType::Hann => GratingMask::Hann,
        proto::MaskType::RaisedCos => GratingMask::RaisedCos,
    }
}

pub(crate) fn mask_to_proto(m: GratingMask) -> proto::MaskType {
    match m {
        GratingMask::None => proto::MaskType::None,
        GratingMask::Circle => proto::MaskType::Circle,
        GratingMask::Gauss => proto::MaskType::Gauss,
        GratingMask::Hann => proto::MaskType::Hann,
        GratingMask::RaisedCos => proto::MaskType::RaisedCos,
    }
}

// ── GratingParams ↔ proto ─────────────────────────────────────────────────────

pub(crate) fn grating_params_from_proto(cmd: &proto::GratingParams) -> GratingParams {
    let sf_cycles_per_px       = if cmd.sf_cycles_per_px       == 0.0 { 0.05 } else { cmd.sf_cycles_per_px };
    let contrast = if cmd.contrast == 0.0 { 1.0  } else { cmd.contrast };
    let fore = color_or_default(cmd.fore_color, Color::WHITE);
    let back = color_or_default(cmd.back_color, Color::BLACK);
    GratingParams {
        sf_cycles_per_px,
        phase_cycles:        cmd.phase_cycles,
        contrast,
        waveform:     waveform_from_proto(cmd.waveform),
        mask:         mask_from_proto(cmd.mask),
        mask_param:   cmd.mask_param,
        drift_speed_hz:  cmd.drift_speed_hz,
        drift_coupled: !cmd.drift_decoupled,
        drift_angle_deg:  cmd.drift_angle_deg,
        fore_color:   fore,
        back_color:   back,
    }
}

// ── Query ─────────────────────────────────────────────────────────────────────

pub(crate) fn grating_params_to_proto(s: &Grating) -> proto::StimulusParams {
    let p = s.params.live;
    proto::StimulusParams {
        shape: Some(proto::stimulus_params::Shape::Grating(proto::GratingParams {
            width_px: s.size_px.live[0],
            height_px: s.size_px.live[1],
            sf_cycles_per_px: p.sf_cycles_per_px,
            phase_cycles: p.phase_cycles,
            contrast: p.contrast,
            waveform: waveform_to_proto(p.waveform) as i32,
            mask: mask_to_proto(p.mask) as i32,
            mask_param: p.mask_param,
            drift_speed_hz: p.drift_speed_hz,
            drift_decoupled: !p.drift_coupled,
            drift_angle_deg: p.drift_angle_deg,
            fore_color: Some(p.fore_color.into()),
            back_color: Some(p.back_color.into()),
        })),
    }
}
