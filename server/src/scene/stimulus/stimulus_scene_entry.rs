use super::Stimulus;
use uuid::Uuid;

// ── StimulusIdentity ──────────────────────────────────────────────────────────

/// Who a stimulus is, as opposed to what it draws or where it sits.
///
/// The scene-side mirror of `proto::StimulusIdentity`, and it exists for the same
/// reason: the identity of a stimulus is one thing, so it travels as one thing
/// rather than as a pair of parallel arguments threaded through every create
/// path. Growing it — tags, a group — then reaches every create site at once.
///
/// Flattened into the entry when serialized, so the scene-config JSON keeps the
/// flat `{"id": …, "name": …}` shape it has always had.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StimulusIdentity {
    /// Stable across sessions: survives serialization round-trips and lets a
    /// reconnecting client match server-side stimuli to its in-memory objects.
    pub id: Uuid,
    /// Optional human-readable label, for debugging and tooling.
    pub name: Option<String>,
}

impl StimulusIdentity {
    /// A fresh identity under a server-assigned id. This is the only way a
    /// stimulus gets created over IPC — clients do not supply ids.
    pub fn new(name: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
        }
    }

    /// An identity with a known id, for loading a stimulus back out of a
    /// scene-config, where the id is part of the saved state.
    pub fn with_id(id: Uuid, name: Option<String>) -> Self {
        Self { id, name }
    }
}

// ── StimulusEntry ─────────────────────────────────────────────────────────────

/// Identity + stimulus stored as one unit in `SceneState::stimuli`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StimulusSceneEntry {
    #[serde(flatten)]
    pub identity: StimulusIdentity,
    pub stimulus: Stimulus,
}

impl StimulusSceneEntry {
    pub fn new(identity: StimulusIdentity, stimulus: Stimulus) -> Self {
        Self { identity, stimulus }
    }

    /// The stable id. A shorthand for the `identity.id` hop, which most callers
    /// only ever want one field out of.
    pub fn id(&self) -> Uuid {
        self.identity.id
    }

    /// The label, or `""` when unset.
    pub fn name(&self) -> &str {
        self.identity.name.as_deref().unwrap_or("")
    }
}
