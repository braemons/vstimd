//! Every proto <-> scene conversion, in one place.
//!
//! Nothing under `scene/` speaks protobuf: the scene owns runtime state and the
//! wire format is this module's problem alone. So the per-stimulus conversions
//! live here next to the shared ones, one submodule per stimulus kind, mirroring
//! the `*_commands.rs` split of the dispatcher that consumes them.
//!
//! This module holds what every kind needs — draw modes, colours, the identity
//! and placement every `Create*` request carries — and the shape appearance,
//! which three of the geometry kinds share.

mod grating;
mod text;

pub(super) use grating::{
    grating_params_from_proto, grating_query_params, proto_to_mask, proto_to_waveform,
};
pub(super) use text::{anchor_from_str, proto_to_language_style, text_query_params,
    text_render_params_from_proto};

use crate::Color;
use crate::proto;
use crate::scene::stimulus::{DrawMode as SceneDrawMode, ShapeAppearance, StimulusIdentity};

pub(super) fn proto_draw_mode_to_scene(mode: i32) -> Result<SceneDrawMode, Box<proto::Response>> {
    match proto::ShapeDrawMode::try_from(mode).unwrap_or(proto::ShapeDrawMode::Unspecified) {
        proto::ShapeDrawMode::Unspecified => Ok(SceneDrawMode::Fill),
        proto::ShapeDrawMode::Filled => Ok(SceneDrawMode::Fill),
        proto::ShapeDrawMode::Outlined => Ok(SceneDrawMode::Stroke),
        proto::ShapeDrawMode::FilledAndOutlined => Ok(SceneDrawMode::FillAndStroke),
    }
}

pub(super) fn scene_draw_mode_to_proto(mode: SceneDrawMode) -> i32 {
    match mode {
        SceneDrawMode::Fill => proto::ShapeDrawMode::Filled as i32,
        SceneDrawMode::Stroke => proto::ShapeDrawMode::Outlined as i32,
        SceneDrawMode::FillAndStroke => proto::ShapeDrawMode::FilledAndOutlined as i32,
    }
}

/// Shape fill/outline state → proto, for the per-shape query params.
pub(super) fn shape_appearance_to_proto(a: &ShapeAppearance) -> proto::ShapeAppearance {
    proto::ShapeAppearance {
        fill_color: Some(a.fill_color.into()),
        outline_color: Some(a.outline_color.into()),
        outline_width: a.stroke_width,
        draw_mode: scene_draw_mode_to_proto(a.draw_mode),
    }
}

fn color_or_default(c: Option<proto::Color>, default: Color) -> Color {
    c.map(|c| c.into()).unwrap_or(default)
}

/// Proto → shape fill/outline state, for the `Create*` commands.
///
/// `appearance` absent means the scene defaults throughout: fill from
/// `default_fill`, outline from `default_outline`, stroke width and draw mode
/// from [`ShapeAppearance::default`].
///
/// `appearance` present overrides field by field, each with the same fallback,
/// so a client may set only `draw_mode` and inherit the rest. Zero means unset
/// for `outline_width`, matching the convention the create commands already use
/// for `width`/`height`/`radius` — and a 0-width outline draws nothing anyway,
/// so `draw_mode` is how you turn an outline off, not width.
pub(super) fn shape_appearance_from_proto(
    appearance: Option<proto::ShapeAppearance>,
    default_fill: Color,
    default_outline: Color,
) -> Result<ShapeAppearance, Box<proto::Response>> {
    let base = ShapeAppearance {
        fill_color: default_fill,
        outline_color: default_outline,
        ..Default::default()
    };
    let Some(a) = appearance else {
        return Ok(base);
    };
    Ok(ShapeAppearance {
        fill_color: color_or_default(a.fill_color, base.fill_color),
        outline_color: color_or_default(a.outline_color, base.outline_color),
        stroke_width: if a.outline_width == 0.0 {
            base.stroke_width
        } else {
            a.outline_width
        },
        draw_mode: proto_draw_mode_to_scene(a.draw_mode)?,
    })
}

/// A create request's `placement` → the scene's `(pos, angle)` pair.
///
/// Absent, or absent `pos`, means the screen centre at 0° — the same default the
/// bare `center`/`angle` fields gave before placement was a message.
pub(super) fn placement_to_scene(placement: Option<proto::Transform2D>) -> ([f32; 2], f32) {
    let Some(t) = placement else {
        return ([0.0, 0.0], 0.0);
    };
    let pos = t.pos.unwrap_or_default();
    ([pos.x, pos.y], t.rotation_deg)
}

/// A create request's `identity` → the scene's, minting the id.
///
/// The server assigns every stimulus id: `proto::StimulusIdentity` carries only a
/// name, so this is where a stimulus acquires the UUID the response echoes back.
pub(super) fn scene_identity(identity: Option<proto::StimulusIdentity>) -> StimulusIdentity {
    StimulusIdentity::new(identity.and_then(|i| nonempty(i.name)))
}

pub(super) fn nonempty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// Split the compile-time version into the numeric major/minor/patch the proto
/// carries.
///
/// The string comes from the git tag via build.rs, so it can carry a
/// pre-release or build suffix that the numeric triple has nowhere to put:
/// `0.1.0~alpha4+2.ga3bb27e` is major 0, minor 1, patch 0. Each field is read
/// up to its first non-digit rather than parsed whole, which is what keeps the
/// patch of a tagged pre-release from silently reading as 0.
pub(super) fn parse_version() -> proto::Version {
    parse_version_str(env!("VSTIMD_VERSION"))
}

pub(super) fn parse_version_str(s: &str) -> proto::Version {
    let mut parts = s.splitn(3, '.').map(|p| {
        // Leading digits only. Skipping over non-digits first would make
        // "0.1.~alpha4" report patch 4, dressing a malformed version up as a
        // plausible one — the opposite of what the 0.0.0 sentinel is for.
        let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u32>().unwrap_or(0)
    });
    proto::Version {
        major: parts.next().unwrap_or(0),
        minor: parts.next().unwrap_or(0),
        patch: parts.next().unwrap_or(0),
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;

    fn triple(s: &str) -> (u32, u32, u32) {
        let v = parse_version_str(s);
        (v.major, v.minor, v.patch)
    }

    #[test]
    fn plain_release() {
        assert_eq!(triple("0.1.0"), (0, 1, 0));
        assert_eq!(triple("12.34.56"), (12, 34, 56));
    }

    /// The regression this function exists for: a naive `parse::<u32>()` on the
    /// last field reads the patch of every tagged pre-release as 0.
    #[test]
    fn pre_release_keeps_its_patch() {
        assert_eq!(triple("0.1.2~alpha4"), (0, 1, 2));
        assert_eq!(triple("1.2.3~rc1"), (1, 2, 3));
    }

    #[test]
    fn build_suffix_and_dirty_marker() {
        assert_eq!(triple("0.1.2~alpha4+2.ga3bb27e"), (0, 1, 2));
        assert_eq!(triple("0.1.2+5.gdeadbee+dirty"), (0, 1, 2));
    }

    /// The 0.0.0 sentinel must survive round-tripping, so a client can spot a
    /// binary whose version was never stamped.
    #[test]
    fn sentinel_is_preserved() {
        assert_eq!(triple("0.0.0"), (0, 0, 0));
    }

    #[test]
    fn malformed_input_does_not_panic() {
        assert_eq!(triple(""), (0, 0, 0));
        assert_eq!(triple("nonsense"), (0, 0, 0));
        assert_eq!(triple("1.2"), (1, 2, 0));
    }

    /// A field that does not *start* with a digit reads as 0 rather than
    /// scanning forward for one. Otherwise "0.1.~alpha4" would report patch 4
    /// and a malformed version would look well-formed to a client.
    #[test]
    fn digits_after_junk_are_not_harvested() {
        assert_eq!(triple("0.1.~alpha4"), (0, 1, 0));
        assert_eq!(triple("~alpha4.1.2"), (0, 1, 2));
        assert_eq!(triple("0.1.abc9"), (0, 1, 0));
    }
}
