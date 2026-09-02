use indexmap::IndexMap;

use super::animation::AnimationEntry;
use super::conditions::Conditions;
use super::deferred::Deferred;
use super::photodiode::PhotoDiodeState;
use super::stimulus::StimulusSceneEntry;
use crate::Color;

pub enum LoadMode {
    Replace,
    Additive,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneConfig {
    pub background: Deferred<Color>,
    pub default_fill: Color,
    pub default_outline: Color,
    pub photodiode: PhotoDiodeState,
    pub stimuli: IndexMap<u32, StimulusSceneEntry>,
    pub next_stim_handle: u32,
    pub animations: IndexMap<u32, AnimationEntry>,
    pub next_anim_handle: u32,
    /// Declared conditions and the active one. Defaulted on load, so a
    /// scene-config written before conditions existed still reads; omitted on
    /// save when it says nothing, so one that does not use conditions is
    /// written exactly as it always was.
    #[serde(default, skip_serializing_if = "Conditions::is_default")]
    pub conditions: Conditions,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            background: Deferred::new(Color::BLACK),
            default_fill: Color::WHITE,
            default_outline: Color::BLACK,
            photodiode: PhotoDiodeState::default(),
            stimuli: IndexMap::new(),
            next_stim_handle: 1,
            animations: IndexMap::new(),
            next_anim_handle: 1,
            conditions: Conditions::default(),
        }
    }
}
