//! Text <-> proto conversions.

use super::color_or_default;
use crate::Color;
use crate::proto;

use crate::scene::stimulus::text::{Anchor, LanguageStyle, Text, TextRenderParams};

// ── Anchor ────────────────────────────────────────────────────────────────────

pub(crate) fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-left"     => Anchor::TopLeft,
        "top-right"    => Anchor::TopRight,
        "bottom-left"  => Anchor::BottomLeft,
        "bottom-right" => Anchor::BottomRight,
        _              => Anchor::Center,
    }
}

pub(crate) fn anchor_to_str(a: Anchor) -> &'static str {
    match a {
        Anchor::Center      => "center",
        Anchor::TopLeft     => "top-left",
        Anchor::TopRight    => "top-right",
        Anchor::BottomLeft  => "bottom-left",
        Anchor::BottomRight => "bottom-right",
    }
}

// ── LanguageStyle ─────────────────────────────────────────────────────────────

pub(crate) fn language_style_from_proto(v: i32) -> LanguageStyle {
    match proto::LanguageStyle::try_from(v).unwrap_or(proto::LanguageStyle::Unspecified) {
        proto::LanguageStyle::Rtl    => LanguageStyle::Rtl,
        proto::LanguageStyle::Arabic => LanguageStyle::Arabic,
        _                            => LanguageStyle::Ltr,
    }
}

pub(crate) fn language_style_to_proto(ls: LanguageStyle) -> i32 {
    match ls {
        LanguageStyle::Ltr    => proto::LanguageStyle::Ltr as i32,
        LanguageStyle::Rtl    => proto::LanguageStyle::Rtl as i32,
        LanguageStyle::Arabic => proto::LanguageStyle::Arabic as i32,
    }
}

// ── CreateTextRequest → scene types ───────────────────────────────────────────

pub(crate) fn text_render_params_from_proto(cmd: &proto::TextParams) -> TextRenderParams {
    let color = color_or_default(cmd.text_color, Color::WHITE);
    let fill_color = color_or_default(cmd.fill_color, Color::TRANSPARENT);
    let border_color = color_or_default(cmd.border_color, Color::TRANSPARENT);
    TextRenderParams {
        color,
        fill_color,
        border_color,
        flip_horiz: cmd.flip_horiz,
    }
}

// ── Scene → QueryStimulusResponse payload ────────────────────────────────────

pub(crate) fn text_params_to_proto(s: &Text) -> proto::StimulusParams {
    let p = &s.params.live;
    proto::StimulusParams {
        shape: Some(proto::stimulus_params::Shape::Text(proto::TextParams {
            text:          s.text_live.clone(),
            font:          s.font_family.clone(),
            letter_height: s.letter_height_px,
            box_size: Some(proto::Vec2 {
                x: s.box_size.live[0],
                y: s.box_size.live[1],
            }),
            anchor: anchor_to_str(s.anchor).to_string(),
            fill_color:   Some(p.fill_color.into()),
            border_color: Some(p.border_color.into()),
            text_color:   Some(p.color.into()),
            flip_horiz:     p.flip_horiz,
            language_style: language_style_to_proto(s.language_style),
        })),
    }
}
