use std::sync::{Arc, Mutex, RwLock};

use crate::render::system_info::HostInfo;
use crate::scene::SceneState;
use crate::vtl_state::VtlState;

/// Everything a render backend needs from `main`: shared state plus host info.
pub struct BackendData {
    pub scene: Arc<RwLock<SceneState>>,
    pub vtl: Option<Arc<Mutex<VtlState>>>,
    pub host_info: HostInfo,
    /// egui overlay UI scale factor (independent of OS/window DPI). See
    /// `rig_config::DisplayRigConfig::overlay_scale`.
    pub overlay_scale: f32,
}


