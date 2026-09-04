//! The user-facing stimulus taxonomy.

/// What a client calls a stimulus.
///
/// The single source of truth for the user-facing taxonomy. Before this existed it
/// lived in two unlinked encodings — the `&'static str` each geometry returned, and
/// the `match` in `ipc` that produced the wire's `StimulusType` — so adding a type
/// meant updating both with nothing forcing the second. They had already drifted:
/// a text stimulus reported itself as `"TextStimulus"` from a constant three modules
/// away, while the error that asked for one said `"Text"`.
///
/// Finer than [`StimulusBody`](super::StimulusBody), deliberately: `Rect`, `Ellipse`
/// and `Circle` are three types out of the one `Shape` render path, and the three
/// 3-D types out of the one `Mesh3d`. This is the taxonomy the *user* asked in; the
/// body is the one the renderer works in.
///
/// Not every arm of the wire enum appears here. `Bitmap`, `Shader`, `Particle` and
/// `Polygon` have proto values but no scene representation — `CreatePolygon` is
/// refused in `ipc/dispatch` — so they are not constructible and have no business in
/// a type the scene hands out. The traffic runs the other way too: the 3-D types are
/// here and own no wire value yet, which `ipc/convert` refuses rather than guesses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StimulusType {
    // ── 2-D ──
    Rect,
    Ellipse,
    Circle,
    Grating,
    Text,
    Dots,

    // ── 3-D — no wire value yet; dev/3D_ROADMAP.md §10.2 reserves 20–29 ──
    Cube3D,
    Sphere3D,
    Plane3D,
}

impl StimulusType {
    /// The name a client sees: in `WRONG_STIMULUS_TYPE` error messages and in the
    /// overlay's stimulus list. Never an internal body name — a client has never
    /// heard of "Shape".
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Rect => "Rect",
            Self::Ellipse => "Ellipse",
            Self::Circle => "Circle",
            Self::Grating => "Grating",
            Self::Text => "Text",
            Self::Dots => "Dots",
            Self::Cube3D => "Cube3D",
            Self::Sphere3D => "Sphere3D",
            Self::Plane3D => "Plane3D",
        }
    }

    /// True for the types placed in world space, whose wire representation is still
    /// owed (`transform_3d` on the placement oneof, and the reserved enum values).
    pub fn is_3d(self) -> bool {
        matches!(self, Self::Cube3D | Self::Sphere3D | Self::Plane3D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every type names itself distinctly. Two types sharing a name would make a
    /// `WRONG_STIMULUS_TYPE` message ambiguous about what the client actually sent.
    #[test]
    fn type_names_are_unique() {
        const ALL: [StimulusType; 9] = [
            StimulusType::Rect,
            StimulusType::Ellipse,
            StimulusType::Circle,
            StimulusType::Grating,
            StimulusType::Text,
            StimulusType::Dots,
            StimulusType::Cube3D,
            StimulusType::Sphere3D,
            StimulusType::Plane3D,
        ];
        let mut names: Vec<&str> = ALL.iter().map(|t| t.type_name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two stimulus types share a name");
    }

    /// Every name matches the wire enum's own spelling, so a client is never told
    /// about a type under one name and shown it under another. `Text` used to be the
    /// exception, reported as `"TextStimulus"` (the C++ StimServer's name) while the
    /// error asking for one said `"Text"`.
    #[test]
    fn names_match_the_wire_spelling() {
        assert_eq!(StimulusType::Text.type_name(), "Text");
        assert_eq!(StimulusType::Rect.type_name(), "Rect");
    }
}
