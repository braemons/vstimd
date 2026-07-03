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
    /// Preferred DRM display mode from rig-config. Ignored by the desktop and
    /// null backends — only `DrmBackend` selects a display mode.
    pub display_pref: DisplayModePref,
}

/// A rig-config display-mode preference, as loose match criteria against the
/// modes a DRM display actually reports. Any field left `None` is not
/// filtered on. See `rig_config::DisplayRigConfig`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DisplayModePref {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh_hz: Option<f64>,
}


