//! User input: the app-level action vocabulary and the device backends that
//! produce it.
//!
//! Input is not a rendering concern — the render loop only consumes the
//! [`AppKey`]s and egui events this module emits. Gamepad and mouse handling
//! will join `console_input` here as sibling device backends.

pub mod app_keys;
pub use app_keys::AppKey;

/// libinput keyboard handling shared by the bare-console backends
/// (`render::drm` and `render::evdi`).
#[cfg(target_os = "linux")]
pub(crate) mod console_input;
