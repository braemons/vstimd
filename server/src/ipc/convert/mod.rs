//! Every proto <-> scene conversion, in one place.
//!
//! Nothing under `scene/` speaks protobuf: the scene owns runtime state and the
//! wire format is this module's problem alone. So the per-stimulus conversions live
//! here next to the shared ones, one submodule per stimulus body, mirroring the
//! `*_commands.rs` split of the dispatcher that consumes them.
//!
//! The two conversions that are not per-body live here too: `animation` for the
//! `CreateAnimationRequest` body and its edge/polarity enums, and `vtl` for line
//! handles — both of which used to sit inside the command module that consumed
//! them, where nothing stopped a fourth copy of the taxonomy appearing beside them.
//!
//! Every function reads `X_from_proto` or `X_to_proto`, in that direction. The
//! four dialects this replaced (`proto_to_X`, `proto_X_to_scene`, `X_from_proto`,
//! `proto_X`) made the direction of a call something you looked up rather than read.
//!
//! This module holds what every body needs — colours, draw modes, the user-facing
//! type, and the identity and placement every `Create*` request carries — plus the
//! shape appearance, which three of the geometries share.
//!
//! "Every" is meant literally: `scene/` names no proto type anywhere, so a change to
//! the wire cannot reach the scene tree without passing through this module.

mod animation;
mod condition;
mod dots;
mod grating;
mod text;
mod vtl;

pub(super) use animation::{
    animation_body_to_proto, animation_from_proto, vtl_edge_from_proto, vtl_edge_to_proto,
};
pub(super) use condition::{
    condition_action_from_proto, condition_action_to_proto, condition_from_proto,
    condition_to_proto,
};
pub(super) use dots::{
    aperture_from_proto, dots_params_from_proto, dots_params_to_proto,
};
pub(super) use grating::{
    grating_params_from_proto, grating_params_to_proto, mask_from_proto, waveform_from_proto,
};
pub(super) use text::{anchor_from_str, language_style_from_proto, text_params_to_proto,
    text_render_params_from_proto};
pub(super) use vtl::{
    output_vtl_bit_from_proto, vtl_bit_from_proto, vtl_bit_to_proto, vtl_kind_from_proto,
};

use crate::Color;
use crate::proto;
use crate::scene::stimulus::{
    DrawMode as SceneDrawMode, ShapeAppearance, StimulusIdentity,
    StimulusType as SceneStimulusType,
};

// Colour is the one conversion the whole wire surface needs, in both directions.
// The impls live here rather than beside the type: coherence is per-crate, so
// nothing forces them into `color.rs`, and `scene/` has no business naming a proto
// type. Written as `From` rather than free functions because `Option::map(Into::into)`
// at the call sites is what keeps the absent-field handling readable.

impl From<proto::Color> for Color {
    fn from(c: proto::Color) -> Self {
        Self { r: c.r, g: c.g, b: c.b, a: c.a }
    }
}

impl From<Color> for proto::Color {
    fn from(c: Color) -> Self {
        Self { r: c.r, g: c.g, b: c.b, a: c.a }
    }
}

/// The scene's user-facing type → the wire enum.
///
/// The only place the two encodings of that taxonomy meet. `StimulusType` is
/// exhaustive here, so adding a stimulus type is a compile error until the wire
/// value is chosen — which is the whole reason the scene owns a native enum instead
/// of handing out strings and letting this match be written from memory.
pub(super) fn stimulus_type_to_proto(t: SceneStimulusType) -> proto::StimulusType {
    match t {
        SceneStimulusType::Rect => proto::StimulusType::Rect,
        SceneStimulusType::Ellipse => proto::StimulusType::Ellipse,
        SceneStimulusType::Circle => proto::StimulusType::Circle,
        SceneStimulusType::Grating => proto::StimulusType::Grating,
        SceneStimulusType::Text => proto::StimulusType::Text,
        SceneStimulusType::Dots => proto::StimulusType::Dots,
        // Phase B: dev/3D_ROADMAP.md §10.2 reserves wire values 20–29 for these.
        // Unreachable until a command constructs a `Mesh3d`, and reporting one of
        // the 2-D values instead would be a lie a client could not detect.
        SceneStimulusType::Cube3D | SceneStimulusType::Sphere3D | SceneStimulusType::Plane3D => {
            unimplemented!("Phase B: STIMULUS_TYPE_CUBE_3D / _SPHERE_3D / _PLANE_3D")
        }
    }
}

pub(super) fn draw_mode_from_proto(mode: i32) -> Result<SceneDrawMode, Box<proto::Response>> {
    match proto::ShapeDrawMode::try_from(mode).unwrap_or(proto::ShapeDrawMode::Unspecified) {
        proto::ShapeDrawMode::Unspecified => Ok(SceneDrawMode::Fill),
        proto::ShapeDrawMode::Filled => Ok(SceneDrawMode::Fill),
        proto::ShapeDrawMode::Outlined => Ok(SceneDrawMode::Stroke),
        proto::ShapeDrawMode::FilledAndOutlined => Ok(SceneDrawMode::FillAndStroke),
    }
}

pub(super) fn draw_mode_to_proto(mode: SceneDrawMode) -> i32 {
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
        outline_width_px: a.stroke_width_px,
        draw_mode: draw_mode_to_proto(a.draw_mode),
    }
}

pub(super) fn color_or_default(c: Option<proto::Color>, default: Color) -> Color {
    c.map(|c| c.into()).unwrap_or(default)
}

/// Proto → shape fill/outline state, for the `Create*` commands.
///
/// `appearance` absent means the scene defaults throughout: fill from
/// `default_fill`, outline from `default_outline`, stroke width_px and draw mode
/// from [`ShapeAppearance::default`].
///
/// `appearance` present overrides field by field, each with the same fallback,
/// so a client may set only `draw_mode` and inherit the rest. Zero means unset
/// for `outline_width_px`, matching the convention the create commands already use
/// for `width_px`/`height_px`/`diameter_px` — and a 0-width_px outline draws nothing anyway,
/// so `draw_mode` is how you turn an outline off, not width_px.
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
        stroke_width_px: if a.outline_width_px == 0.0 {
            base.stroke_width_px
        } else {
            a.outline_width_px
        },
        draw_mode: draw_mode_from_proto(a.draw_mode)?,
    })
}

/// A create request's `placement` → the scene's `(pos_px, angle_deg)` pair.
///
/// Absent, or absent `pos_px`, means the screen centre at 0° — the same default the
/// bare `center`/`angle_deg` fields gave before placement was a message.
pub(super) fn placement_from_proto(placement: Option<proto::Transform2D>) -> ([f32; 2], f32) {
    let Some(t) = placement else {
        return ([0.0, 0.0], 0.0);
    };
    let pos_px = t.pos_px.unwrap_or_default();
    ([pos_px.x, pos_px.y], t.rotation_deg)
}

/// A create request's `identity` → the scene's, minting the id.
///
/// The server assigns every stimulus id: `proto::StimulusIdentity` carries only a
/// name, so this is where a stimulus acquires the UUID the response echoes back.
pub(super) fn identity_from_proto(identity: Option<proto::StimulusIdentity>) -> StimulusIdentity {
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
mod stimulus_type_tests {
    use super::*;

    /// The scene's name and the wire's enum are two encodings of one taxonomy, and
    /// they are only linked by the match above. This pins the pairs so a change to
    /// either side has to be a deliberate edit here.
    #[test]
    fn scene_types_map_to_their_wire_values() {
        for (scene, wire, name) in [
            (SceneStimulusType::Rect, proto::StimulusType::Rect, "Rect"),
            (SceneStimulusType::Ellipse, proto::StimulusType::Ellipse, "Ellipse"),
            (SceneStimulusType::Circle, proto::StimulusType::Circle, "Circle"),
            (SceneStimulusType::Grating, proto::StimulusType::Grating, "Grating"),
            (SceneStimulusType::Text, proto::StimulusType::Text, "Text"),
        ] {
            assert_eq!(stimulus_type_to_proto(scene), wire, "wire value for {name}");
            assert_eq!(scene.type_name(), name);
        }
    }

    /// A 3-D type has no wire value yet, and must refuse rather than report a 2-D one
    /// — a client cannot tell a wrong type from a right one.
    #[test]
    #[should_panic(expected = "Phase B")]
    fn three_d_types_have_no_wire_value_yet() {
        let _ = stimulus_type_to_proto(SceneStimulusType::Cube3D);
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
