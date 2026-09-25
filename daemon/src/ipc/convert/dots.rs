//! Dot-field <-> proto conversions.

use super::color_or_default;
use crate::Color;
use crate::proto;

use crate::scene::stimulus::dots::{
    Aperture, ApertureClip, ApertureShape, DotShape, Dots, DotsParams, NoiseRule, Reinsertion,
    SignalRule,
};

// ── Enumerations ──────────────────────────────────────────────────────────────

pub(crate) fn dot_shape_from_proto(v: i32) -> DotShape {
    match proto::DotShape::try_from(v).unwrap_or(proto::DotShape::Unspecified) {
        proto::DotShape::Unspecified | proto::DotShape::Round => DotShape::Round,
        proto::DotShape::Square => DotShape::Square,
    }
}

pub(crate) fn dot_shape_to_proto(s: DotShape) -> proto::DotShape {
    match s {
        DotShape::Round => proto::DotShape::Round,
        DotShape::Square => proto::DotShape::Square,
    }
}

pub(crate) fn aperture_shape_from_proto(v: i32) -> ApertureShape {
    match proto::ApertureShape::try_from(v).unwrap_or(proto::ApertureShape::Unspecified) {
        proto::ApertureShape::Unspecified | proto::ApertureShape::Rect => ApertureShape::Rect,
        proto::ApertureShape::Circle => ApertureShape::Circle,
    }
}

pub(crate) fn aperture_shape_to_proto(s: ApertureShape) -> proto::ApertureShape {
    match s {
        ApertureShape::Rect => proto::ApertureShape::Rect,
        ApertureShape::Circle => proto::ApertureShape::Circle,
    }
}

pub(crate) fn aperture_clip_from_proto(v: i32) -> ApertureClip {
    match proto::ApertureClip::try_from(v).unwrap_or(proto::ApertureClip::Unspecified) {
        proto::ApertureClip::Unspecified | proto::ApertureClip::DotCenter => ApertureClip::DotCenter,
        proto::ApertureClip::Pixel => ApertureClip::Pixel,
    }
}

pub(crate) fn aperture_clip_to_proto(c: ApertureClip) -> proto::ApertureClip {
    match c {
        ApertureClip::DotCenter => proto::ApertureClip::DotCenter,
        ApertureClip::Pixel => proto::ApertureClip::Pixel,
    }
}

pub(crate) fn signal_rule_from_proto(v: i32) -> SignalRule {
    match proto::SignalRule::try_from(v).unwrap_or(proto::SignalRule::Unspecified) {
        proto::SignalRule::Unspecified | proto::SignalRule::Same => SignalRule::Same,
        proto::SignalRule::Different => SignalRule::Different,
    }
}

pub(crate) fn signal_rule_to_proto(r: SignalRule) -> proto::SignalRule {
    match r {
        SignalRule::Same => proto::SignalRule::Same,
        SignalRule::Different => proto::SignalRule::Different,
    }
}

pub(crate) fn noise_rule_from_proto(v: i32) -> NoiseRule {
    match proto::NoiseRule::try_from(v).unwrap_or(proto::NoiseRule::Unspecified) {
        proto::NoiseRule::Unspecified | proto::NoiseRule::Direction => NoiseRule::Direction,
        proto::NoiseRule::Position => NoiseRule::Position,
        proto::NoiseRule::Walk => NoiseRule::Walk,
    }
}

pub(crate) fn noise_rule_to_proto(r: NoiseRule) -> proto::NoiseRule {
    match r {
        NoiseRule::Position => proto::NoiseRule::Position,
        NoiseRule::Direction => proto::NoiseRule::Direction,
        NoiseRule::Walk => proto::NoiseRule::Walk,
    }
}

pub(crate) fn reinsertion_from_proto(v: i32) -> Reinsertion {
    match proto::Reinsertion::try_from(v).unwrap_or(proto::Reinsertion::Unspecified) {
        proto::Reinsertion::Unspecified | proto::Reinsertion::Wrap => Reinsertion::Wrap,
        proto::Reinsertion::Respawn => Reinsertion::Respawn,
    }
}

pub(crate) fn reinsertion_to_proto(r: Reinsertion) -> proto::Reinsertion {
    match r {
        Reinsertion::Wrap => proto::Reinsertion::Wrap,
        Reinsertion::Respawn => proto::Reinsertion::Respawn,
    }
}

// ── Aperture ──────────────────────────────────────────────────────────────────

/// An aperture whose zero fields fall back to `field_size_px`.
///
/// A zero size means "the whole field", which is the classic-RDK case where the
/// aperture *is* the field. The default has to be resolved here rather than in the
/// scene, because only the create request knows the field size.
pub(crate) fn aperture_from_proto(a: Option<proto::Aperture>, field_size_px: [f32; 2]) -> Aperture {
    let Some(a) = a else {
        return Aperture {
            shape: ApertureShape::Rect,
            size_px: field_size_px,
            ..Default::default()
        };
    };
    let width_px = if a.width_px == 0.0 { field_size_px[0] } else { a.width_px };
    let height_px = if a.height_px == 0.0 { field_size_px[1] } else { a.height_px };
    Aperture {
        shape: aperture_shape_from_proto(a.shape),
        size_px: [width_px, height_px],
        offset_px: [a.offset_x_px, a.offset_y_px],
        invert: a.invert,
        clip: aperture_clip_from_proto(a.clip),
    }
}

pub(crate) fn aperture_to_proto(a: &Aperture) -> proto::Aperture {
    proto::Aperture {
        shape: aperture_shape_to_proto(a.shape) as i32,
        width_px: a.size_px[0],
        height_px: a.size_px[1],
        offset_x_px: a.offset_px[0],
        offset_y_px: a.offset_px[1],
        invert: a.invert,
        clip: aperture_clip_to_proto(a.clip) as i32,
    }
}

// ── DotsParams ↔ proto ────────────────────────────────────────────────────────

pub(crate) fn dots_params_from_proto(cmd: &proto::DotsParams) -> DotsParams {
    let d = DotsParams::default();
    let field_size_px = [
        if cmd.field_width_px == 0.0 { d.field_size_px[0] } else { cmd.field_width_px },
        if cmd.field_height_px == 0.0 { d.field_size_px[1] } else { cmd.field_height_px },
    ];
    DotsParams {
        field_size_px,
        dot_count: if cmd.dot_count == 0 { d.dot_count } else { cmd.dot_count },
        aperture: aperture_from_proto(cmd.aperture, field_size_px),
        dot_size_px: if cmd.dot_size_px == 0.0 { d.dot_size_px } else { cmd.dot_size_px },
        dot_color: color_or_default(cmd.dot_color, Color::WHITE),
        dot_color_alt: cmd.dot_color_alt.map(Into::into),
        dot_shape: dot_shape_from_proto(cmd.dot_shape),
        direction_deg: cmd.direction_deg,
        // Not zero-means-default: zero is meaningful for both of these — a static
        // field, and a field of pure noise — so they carry field presence and the
        // fallback is on absence, not on zero.
        speed_px_per_s: cmd.speed_px_per_s.unwrap_or(d.speed_px_per_s),
        coherence: cmd.coherence.map_or(d.coherence, |c| c.clamp(0.0, 1.0)),
        signal_rule: signal_rule_from_proto(cmd.signal_rule),
        noise_rule: noise_rule_from_proto(cmd.noise_rule),
        reinsertion: reinsertion_from_proto(cmd.reinsertion),
        dot_lifetime_frames: cmd.dot_lifetime_frames,
        seed: cmd.seed,
    }
}

// ── Query ─────────────────────────────────────────────────────────────────────

pub(crate) fn dots_params_to_proto(s: &Dots) -> proto::StimulusParams {
    let p = s.params.live;
    proto::StimulusParams {
        shape: Some(proto::stimulus_params::Shape::Dots(proto::DotsParams {
            field_width_px: p.field_size_px[0],
            field_height_px: p.field_size_px[1],
            dot_count: p.dot_count,
            aperture: Some(aperture_to_proto(&p.aperture)),
            dot_size_px: p.dot_size_px,
            dot_color: Some(p.dot_color.into()),
            dot_color_alt: p.dot_color_alt.map(Into::into),
            dot_shape: dot_shape_to_proto(p.dot_shape) as i32,
            direction_deg: p.direction_deg,
            speed_px_per_s: Some(p.speed_px_per_s),
            coherence: Some(p.coherence),
            signal_rule: signal_rule_to_proto(p.signal_rule) as i32,
            noise_rule: noise_rule_to_proto(p.noise_rule) as i32,
            reinsertion: reinsertion_to_proto(p.reinsertion) as i32,
            dot_lifetime_frames: p.dot_lifetime_frames,
            seed: p.seed,
        })),
    }
}
