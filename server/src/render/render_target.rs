#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowMode {
    #[default]
    Fullscreen,
    Windowed {
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderTarget {
    Drm,
    Desktop(WindowMode),
    Null,
    /// Direct KMS presentation on a DisplayLink (evdi) output, no
    /// compositor. Not auto-detected — opt in with `--evdi`. See
    /// `docs/developer/evdi-direct-presentation-plan.md`.
    Evdi,
}
