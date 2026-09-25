//! Direct-KMS presentation on a DisplayLink (`evdi`) output — no compositor.
//!
//! See `docs/developer/evdi-direct-presentation-plan.md` for the full design.
//! Not for stimulus *timing*: DisplayLink relays frames over USB with no GPU
//! vsync regardless of who drives the KMS device, so stimulus onset cannot be
//! trusted to the frame. Suitable for auxiliary/status output and for
//! behavioral-training stimulus, where no onset timestamp is being measured;
//! use the DRM backend for recording sessions.

mod evdi_detect;
mod evdi_init;
mod evdi_kms;
mod evdi_render_loop;

pub use evdi_detect::{find_connected_evdi, has_connected_native_display};
pub use evdi_init::init as init_headless_vk;
pub use evdi_kms::EvdiOutput;
pub use evdi_render_loop::EvdiBackend;
