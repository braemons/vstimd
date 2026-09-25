use indexmap::IndexMap;

use super::animation::AnimationEntry;
use super::camera3d::{Camera3D, Lighting3D};
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
    /// The 3-D camera. Defaulted and omitted on save like `conditions`, so a
    /// pure 2-D scene-config is written exactly as it was before 3-D existed.
    #[serde(default, skip_serializing_if = "camera_is_default")]
    pub camera: Deferred<Camera3D>,
    /// Lighting for `Phong` 3-D surfaces. Omitted on save when default, like the
    /// camera.
    #[serde(default, skip_serializing_if = "lighting_is_default")]
    pub lighting: Deferred<Lighting3D>,
    /// Camera zones — see [`super::zones`]. Omitted on save when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub camera_zones: Vec<super::zones::CameraZone>,
}

fn lighting_is_default(lighting: &Deferred<Lighting3D>) -> bool {
    lighting.live == Lighting3D::default()
}

fn camera_is_default(camera: &Deferred<Camera3D>) -> bool {
    camera.live == Camera3D::default()
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
            camera: Deferred::new(Camera3D::default()),
            lighting: Deferred::new(Lighting3D::default()),
            camera_zones: Vec::new(),
        }
    }
}
