/// Serializable part of stimulus flags.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[derive(Default)]
pub struct StimulusFlagsConfig {
    /// User-controlled visibility.
    pub enabled: bool,
    pub protected: bool, // survives RemoveAll
}


/// Full stimulus flag state: serializable config + render-thread runtime fields.
#[derive(Clone)]
pub struct StimulusFlags {
    pub config: StimulusFlagsConfig,
    pub enabled_copy: bool,
    /// Condition-controlled visibility: false while the active condition
    /// excludes this stimulus. Derived from the entry's membership list by
    /// `SceneState::apply_conditions`, never written by a command, and never
    /// serialized — the membership is the saved state, this is the conclusion
    /// drawn from it. Defaults to true, so anything the conditions never speak
    /// about is shown.
    pub cond_enabled: bool,
    /// Animation-controlled visibility. Written by the render thread each frame.
    /// Defaults to true (no animation hold). Animations set this; user commands do not.
    /// Not part of deferred mode — the render thread owns it exclusively.
    pub anim_enabled: bool,
    /// Set on creation, mutation, or flip. Cleared by the render thread after
    /// tessellation+upload. Prevents redundant vkAllocateMemory every frame.
    pub dirty: bool,
}

impl Default for StimulusFlags {
    fn default() -> Self {
        Self {
            config: StimulusFlagsConfig::default(),
            enabled_copy: false,
            cond_enabled: true,
            anim_enabled: true,
            dirty: true,
        }
    }
}

impl std::ops::Deref for StimulusFlags {
    type Target = StimulusFlagsConfig;
    fn deref(&self) -> &StimulusFlagsConfig { &self.config }
}

impl std::ops::DerefMut for StimulusFlags {
    fn deref_mut(&mut self) -> &mut StimulusFlagsConfig { &mut self.config }
}

/// Serializes as the config half only (`enabled` + `protected`); the runtime
/// fields are restored to load-time defaults on the way back in. One of the
/// three leaves that own a config/runtime split — see [`Grating`] and [`Text`].
///
/// [`Grating`]: super::Grating
/// [`Text`]: super::Text
impl serde::Serialize for StimulusFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.config.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for StimulusFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from_config(StimulusFlagsConfig::deserialize(d)?))
    }
}

impl StimulusFlags {
    /// Rebuild from the config half, restoring every runtime field to the value
    /// a freshly-loaded stimulus needs: `dirty` so the render thread tessellates
    /// it on the first frame, `anim_enabled` and `cond_enabled` true (no
    /// animation hold, and no condition gate yet — the load path re-derives that
    /// one from the loaded conditions), and the deferred copy equal to live so a
    /// flip is a no-op.
    ///
    /// The one place a `StimulusFlags` comes back from a config file.
    pub fn from_config(config: StimulusFlagsConfig) -> Self {
        Self {
            enabled_copy: config.enabled,
            config,
            cond_enabled: true,
            anim_enabled: true,
            dirty: true,
        }
    }

    /// Construct with the given enabled state; all other fields take their defaults.
    pub fn enabled(enabled: bool) -> Self {
        Self {
            config: StimulusFlagsConfig { enabled, protected: false },
            enabled_copy: false,
            cond_enabled: true,
            anim_enabled: true,
            dirty: true,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn make_copy(&mut self) {
        self.enabled_copy = self.enabled;
    }

    pub fn get_copy(&mut self) {
        self.enabled = self.enabled_copy;
    }

    /// Three independent gates, all of which must be open: what the operator
    /// asked for, what the active condition allows, and what a running
    /// animation is holding.
    pub fn is_visible(&self) -> bool {
        self.enabled && self.cond_enabled && self.anim_enabled
    }
}
