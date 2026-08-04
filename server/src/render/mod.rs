pub mod backend;
pub use backend::{BackendData, DisplayModePref};

pub mod null_backend;
pub use null_backend::NullBackend;

pub mod app_keys;
pub use app_keys::AppKey;

/// libinput keyboard handling shared by the bare-console backends
/// (`drm` and `evdi`).
#[cfg(target_os = "linux")]
pub(crate) mod console_input;

pub mod vertex;
pub use vertex::Vertex;

pub mod display_info;
pub use display_info::StimulusDisplayInfo;

pub mod render_target;
pub use render_target::{RenderTarget, RenderTargetPref, WindowMode};

pub mod system_info;
pub use system_info::{
    ClockSource, HostInfo, SystemInfo, query_hardware_model, query_hostname, query_local_ip,
};

pub(crate) mod benchmark;
pub(crate) mod system_metrics;
pub use system_metrics::{MetricsSampler, SystemMetrics};
pub(crate) mod overlay_ui;
pub mod tess;
pub(crate) mod vk;

pub(crate) mod scene_renderer;
pub use scene_renderer::SceneRenderer;

pub(crate) mod text_renderer;
pub use text_renderer::TextRenderer;

pub(crate) mod ui_renderer;
pub use ui_renderer::UiRenderer;

pub mod render_state;
pub use render_state::RenderState;

pub mod render_frame;
pub use render_frame::{ReadbackTarget, render_frame};

/// Render-loop steps shared by all backends (keys, overlay input, VTL).
pub(crate) mod frame_loop;

pub(crate) mod demo;
pub(crate) use demo::spawn_demo_stimuli;

#[cfg(target_os = "linux")]
pub mod drm;
#[cfg(target_os = "linux")]
pub mod evdi;
pub mod winit_vk;
