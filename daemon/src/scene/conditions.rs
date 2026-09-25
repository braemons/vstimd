//! Conditions — the experiment's protocol steps, and which stimuli and
//! animations belong to each.
//!
//! A condition is addressed by an **index**. Declaring one additionally gives
//! that index a name, so a script can say `"probe"` instead of `2`; declaration
//! is optional, and an undeclared index is a perfectly good nameless condition.
//! Only a *name* has to be declared before it can be used, because a typo that
//! silently selected some other condition would blank the screen.
//!
//! Membership lives on the stimulus and animation entries as a list of indices,
//! and **empty means every condition** — a scene that never mentions conditions
//! behaves exactly as it did before, and only what opts in is ever gated.
//!
//! The gating itself is a derived runtime flag on each entry
//! (`StimulusFlags::cond_enabled`, `AnimationEntry::cond_enabled`), recomputed
//! by [`SceneState::apply_conditions`] whenever the active condition or a
//! membership changes. Deriving it once, rather than consulting the membership
//! list at every read, is what lets the existing render paths — which ask a
//! stimulus `is_visible()` and nothing else — pick conditions up unchanged.
//!
//! [`SceneState::apply_conditions`]: super::SceneState::apply_conditions

/// One declared condition: the index it is addressed by, plus the name that
/// index may also be addressed by.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Condition {
    pub index: u32,
    /// Absent when the condition was declared without a name — an index alone
    /// is enough to switch to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Condition {
    pub fn new(index: u32, name: Option<String>) -> Self {
        Self { index, name }
    }
}

/// The scene's condition state: what is declared, and what is active.
///
/// `active` starts at 0, so a scene with no conditions at all is "in condition
/// 0" — which, since every unqualified membership list is empty, gates nothing.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Conditions {
    /// Declared conditions, in declaration order.
    pub declared: Vec<Condition>,
    /// The active condition index. Need not be declared.
    pub active: u32,
}

impl Conditions {
    /// True when this scene says nothing about conditions: nothing declared,
    /// and sitting in condition 0. Such a scene is not written to the config
    /// file at all — a `"conditions"` block that only ever reads
    /// `{"declared": [], "active": 0}` is noise in every file that does not use
    /// the feature, and its absence is exactly what the serde default restores.
    pub fn is_default(&self) -> bool {
        self.declared.is_empty() && self.active == 0
    }

    /// The index a declared name refers to, or `None` if no condition carries it.
    pub fn index_of_name(&self, name: &str) -> Option<u32> {
        self.declared
            .iter()
            .find(|c| c.name.as_deref() == Some(name))
            .map(|c| c.index)
    }

    /// The name declared for `index`, if any.
    pub fn name_of(&self, index: u32) -> Option<&str> {
        self.declared
            .iter()
            .find(|c| c.index == index)
            .and_then(|c| c.name.as_deref())
    }

    /// The name of the active condition, or `""`.
    pub fn active_name(&self) -> &str {
        self.name_of(self.active).unwrap_or("")
    }

    /// How the active condition reads in a log line or the overlay:
    /// `2 (probe)` when named, `2` when not.
    pub fn active_label(&self) -> String {
        match self.name_of(self.active) {
            Some(n) => format!("{} ({n})", self.active),
            None => self.active.to_string(),
        }
    }

    /// Replace the declared set. Indices must be unique, and so must the names
    /// that are present — a duplicate of either makes an address ambiguous, and
    /// there is no reading of that which is better than refusing it.
    pub fn declare(&mut self, declared: Vec<Condition>) -> Result<(), String> {
        for (i, c) in declared.iter().enumerate() {
            let earlier = &declared[..i];
            if earlier.iter().any(|e| e.index == c.index) {
                return Err(format!("duplicate condition index {}", c.index));
            }
            if let Some(name) = c.name.as_deref()
                && earlier.iter().any(|e| e.name.as_deref() == Some(name))
            {
                return Err(format!("duplicate condition name {name:?}"));
            }
        }
        self.declared = declared;
        Ok(())
    }

    /// Merge `other`'s declarations into this set, keeping ours where an index
    /// or a name already exists. The additive-load path: a merged-in scene may
    /// name conditions this one has not, but it may not rename ours.
    pub fn merge_declared(&mut self, other: &Conditions) {
        for c in &other.declared {
            let clash = self.declared.iter().any(|e| {
                e.index == c.index || (c.name.is_some() && e.name == c.name)
            });
            if !clash {
                self.declared.push(c.clone());
            }
        }
    }
}

/// What a condition switch does to an animation's lifecycle state.
///
/// Membership decides *whether* an animation runs; this decides what happens to
/// one at the moment that answer changes. It sits per-animation, beside
/// `start_action`/`final_action`/`cancel_action`, because the two useful
/// readings genuinely differ per animation: a sequence belonging to one protocol
/// step wants to start afresh every time that step comes up, while a background
/// animation shared across steps wants to be left exactly where it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConditionAction {
    /// Leaving the active condition returns the animation to `Idle` (releasing
    /// any visibility hold it placed); entering re-arms it from the start.
    #[default]
    Reset,
    /// Leave the lifecycle state alone: the animation stops advancing while its
    /// condition is inactive and resumes from the same frame when it returns.
    Hold,
    /// Stop on the way out, but do not re-arm on the way in.
    Stop,
}

impl ConditionAction {
    /// True for the default policy, which is not written to the config file —
    /// an animation that never mentions conditions should serialize exactly as
    /// it did before conditions existed.
    pub fn is_default(&self) -> bool {
        *self == ConditionAction::Reset
    }
}

/// Whether a membership list is satisfied by the active condition.
///
/// The one place the "empty means every condition" rule is spelled, so a
/// stimulus and an animation can never disagree about what an empty list means.
pub fn active_in(member_of: &[u32], active: u32) -> bool {
    member_of.is_empty() || member_of.contains(&active)
}
