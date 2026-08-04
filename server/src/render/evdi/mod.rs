//! Direct-KMS presentation on a DisplayLink (`evdi`) output — no compositor.
//!
//! See `docs/developer/evdi-direct-presentation-plan.md` for the full design.
//! Not for stimulus timing: DisplayLink relays frames over USB with no GPU
//! vsync regardless of who drives the KMS device. Auxiliary/status output
//! only.

mod evdi_detect;

pub use evdi_detect::find_connected_evdi;
