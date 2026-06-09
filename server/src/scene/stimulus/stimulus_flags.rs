#[derive(Clone, Copy)]
pub struct StimulusFlags {
    /// User-controlled visibility. Written by SetEnabled / SetAllEnabled (ZMQ thread).
    pub enabled: bool,
    pub enabled_copy: bool,
    /// Animation-controlled visibility. Written by the render thread each frame.
    /// Defaults to true (no animation hold). Animations set this; user commands do not.
    /// Not part of deferred mode — the render thread owns it exclusively.
    pub anim_enabled: bool,
    pub protected: bool, // survives RemoveAll
    /// Set on creation, mutation, or flip. Cleared by the render thread after
    /// tessellation+upload. Prevents redundant vkAllocateMemory every frame.
    pub dirty: bool,
}

impl Default for StimulusFlags {
    fn default() -> Self {
        Self {
            enabled: false,
            enabled_copy: false,
            anim_enabled: true,
            protected: false,
            dirty: true,
        }
    }
}

impl StimulusFlags {
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn make_copy(&mut self) {
        self.enabled_copy = self.enabled;
    }

    pub fn get_copy(&mut self) {
        self.enabled = self.enabled_copy;
    }

    pub fn is_visible(&self) -> bool {
        self.enabled && self.anim_enabled
    }
}
