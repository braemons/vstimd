//! Small proto <-> scene conversions shared across the command groups.

use super::response::err;
use crate::Color;
use crate::proto;
use crate::scene::stimulus::{DrawMode as SceneDrawMode, ShapeAppearance};
use uuid::Uuid;

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

pub(super) fn color_or_default(c: Option<proto::Color>, default: Color) -> Color {
    c.map(|c| c.into()).unwrap_or(default)
}

pub(super) fn parse_or_new_uuid(s: &str) -> Result<Uuid, Box<proto::Response>> {
    if s.is_empty() {
        return Ok(Uuid::new_v4());
    }
    Uuid::parse_str(s).map_err(|_| {
        Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "id must be a valid UUID string",
        ))
    })
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
