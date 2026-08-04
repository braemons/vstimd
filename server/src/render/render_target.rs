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

/// A rig-config `[display] backend` preference — like [`RenderTarget`] but
/// without the CLI-only `Desktop` window size, and with `None` standing in
/// for `"auto"` rather than its own variant (mirrors `ClockSource`'s
/// `parse_pref` convention in `system_info.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderTargetPref {
    Drm,
    Desktop,
    Null,
    Evdi,
}

impl RenderTargetPref {
    pub fn as_str(self) -> &'static str {
        match self {
            RenderTargetPref::Drm => "drm",
            RenderTargetPref::Desktop => "desktop",
            RenderTargetPref::Null => "null",
            RenderTargetPref::Evdi => "evdi",
        }
    }

    /// Parses a backend preference, as used in rig-config's `[display]
    /// backend` key. `"auto"` (case-insensitive) means "auto-detect"
    /// (`None`); any other value must name a variant in snake_case (e.g.
    /// `"drm"`, `"desktop"`, `"null"`, `"evdi"`).
    pub fn parse_pref(s: &str) -> Result<Option<Self>, String> {
        if s.eq_ignore_ascii_case("auto") {
            return Ok(None);
        }
        use serde::Deserialize;
        use serde::de::IntoDeserializer;
        RenderTargetPref::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(s))
            .map(Some)
            .map_err(|e| {
                format!(
                    "invalid render backend {s:?} ({e}) — expected \"auto\", \"drm\", \"desktop\", \
                     \"null\", or \"evdi\""
                )
            })
    }
}
