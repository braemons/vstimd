//! Dot-field <-> proto conversions.

use super::color_or_default;
use crate::Color;
use crate::proto;

use crate::scene::stimulus::dots::{
    Aperture, ApertureClip, CoherenceCount, DotShape, Dots, DotsParams, NoiseRule, Region,
    RegionShape, Reinsertion, SignalRule,
};

// ── Enumerations ──────────────────────────────────────────────────────────────

pub(crate) fn dot_shape_from_proto(v: i32) -> DotShape {
    match proto::DotShape::try_from(v).unwrap_or(proto::DotShape::Unspecified) {
        proto::DotShape::Unspecified | proto::DotShape::Round => DotShape::Round,
        proto::DotShape::Square => DotShape::Square,
        proto::DotShape::RoundSmooth => DotShape::RoundSmooth,
    }
}

pub(crate) fn dot_shape_to_proto(s: DotShape) -> proto::DotShape {
    match s {
        DotShape::Round => proto::DotShape::Round,
        DotShape::Square => proto::DotShape::Square,
        DotShape::RoundSmooth => proto::DotShape::RoundSmooth,
    }
}

pub(crate) fn region_shape_from_proto(v: i32) -> RegionShape {
    match proto::RegionShape::try_from(v).unwrap_or(proto::RegionShape::Unspecified) {
        proto::RegionShape::Unspecified | proto::RegionShape::Rect => RegionShape::Rect,
        proto::RegionShape::Ellipse => RegionShape::Ellipse,
    }
}

pub(crate) fn region_shape_to_proto(s: RegionShape) -> proto::RegionShape {
    match s {
        RegionShape::Rect => proto::RegionShape::Rect,
        RegionShape::Ellipse => proto::RegionShape::Ellipse,
    }
}

pub(crate) fn coherence_count_from_proto(v: i32) -> CoherenceCount {
    match proto::CoherenceCount::try_from(v).unwrap_or(proto::CoherenceCount::Unspecified) {
        proto::CoherenceCount::Unspecified | proto::CoherenceCount::Exact => CoherenceCount::Exact,
        proto::CoherenceCount::Binomial => CoherenceCount::Binomial,
    }
}

pub(crate) fn coherence_count_to_proto(c: CoherenceCount) -> proto::CoherenceCount {
    match c {
        CoherenceCount::Exact => proto::CoherenceCount::Exact,
        CoherenceCount::Binomial => proto::CoherenceCount::Binomial,
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

// ── Region and aperture ───────────────────────────────────────────────────────

/// A region whose zero width or height falls back to `fallback`'s — for a field the
/// default field, for an aperture the field. Shape and offset never fall back: a
/// zero offset and `Rect` are ordinary values.
pub(crate) fn region_from_proto(r: Option<proto::Region>, fallback: Region) -> Region {
    let Some(r) = r else {
        return fallback;
    };
    Region {
        shape: region_shape_from_proto(r.shape),
        size_px: [
            if r.width_px == 0.0 { fallback.size_px[0] } else { r.width_px },
            if r.height_px == 0.0 { fallback.size_px[1] } else { r.height_px },
        ],
        offset_px: [r.offset_x_px, r.offset_y_px],
    }
}

pub(crate) fn region_to_proto(r: &Region) -> proto::Region {
    proto::Region {
        shape: region_shape_to_proto(r.shape) as i32,
        width_px: r.size_px[0],
        height_px: r.size_px[1],
        offset_x_px: r.offset_px[0],
        offset_y_px: r.offset_px[1],
    }
}

/// An aperture that defaults to `field`.
///
/// An absent aperture, or one with no region, is the field itself — the classic-RDK
/// case where the aperture *is* the field, and a mask that hides nothing. It has to
/// be resolved here rather than in the scene, because only the request knows the
/// field.
pub(crate) fn aperture_from_proto(a: Option<proto::Aperture>, field: Region) -> Aperture {
    let Some(a) = a else {
        return Aperture::of_field(field);
    };
    Aperture {
        region: region_from_proto(a.region, field),
        invert: a.invert,
        clip: aperture_clip_from_proto(a.clip),
    }
}

pub(crate) fn aperture_to_proto(a: &Aperture) -> proto::Aperture {
    proto::Aperture {
        region: Some(region_to_proto(&a.region)),
        invert: a.invert,
        clip: aperture_clip_to_proto(a.clip) as i32,
    }
}

// ── DotsParams ↔ proto ────────────────────────────────────────────────────────

pub(crate) fn dots_params_from_proto(cmd: &proto::DotsParams) -> DotsParams {
    let d = DotsParams::default();
    let field = region_from_proto(cmd.field, d.field);
    DotsParams {
        field,
        dot_count: if cmd.dot_count == 0 { d.dot_count } else { cmd.dot_count },
        aperture: aperture_from_proto(cmd.aperture, field),
        dot_size_px: if cmd.dot_size_px == 0.0 { d.dot_size_px } else { cmd.dot_size_px },
        dot_color: color_or_default(cmd.dot_color, Color::WHITE),
        dot_color_alt: cmd.dot_color_alt.map(Into::into),
        dot_shape: dot_shape_from_proto(cmd.dot_shape),
        pixel_snap: cmd.pixel_snap,
        direction_deg: cmd.direction_deg,
        // Not zero-means-default: zero is meaningful for both of these — a static
        // field, and a field of pure noise — so they carry field presence and the
        // fallback is on absence, not on zero.
        speed_px_per_s: cmd.speed_px_per_s.unwrap_or(d.speed_px_per_s),
        coherence: cmd.coherence.map_or(d.coherence, |c| c.clamp(0.0, 1.0)),
        coherence_count: coherence_count_from_proto(cmd.coherence_count),
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
            field: Some(region_to_proto(&p.field)),
            dot_count: p.dot_count,
            aperture: Some(aperture_to_proto(&p.aperture)),
            dot_size_px: p.dot_size_px,
            dot_color: Some(p.dot_color.into()),
            dot_color_alt: p.dot_color_alt.map(Into::into),
            dot_shape: dot_shape_to_proto(p.dot_shape) as i32,
            pixel_snap: p.pixel_snap,
            direction_deg: p.direction_deg,
            speed_px_per_s: Some(p.speed_px_per_s),
            coherence: Some(p.coherence),
            coherence_count: coherence_count_to_proto(p.coherence_count) as i32,
            signal_rule: signal_rule_to_proto(p.signal_rule) as i32,
            noise_rule: noise_rule_to_proto(p.noise_rule) as i32,
            reinsertion: reinsertion_to_proto(p.reinsertion) as i32,
            dot_lifetime_frames: p.dot_lifetime_frames,
            seed: p.seed,
        })),
    }
}
