//! `vstimd-scene-from-capture`: a folder of corridor photos in, a Gaussian splat scene
//! out, already in vstimd's world (centimetres, floor at `y = 0`, running down
//! −Z from the origin).
//!
//! A job wrapped around existing tools — COLMAP for camera poses, Brush for
//! training — plus the one step no tool does: alignment ([`align`]). The design
//! is `dev/design/SPLAT_RECONSTRUCTION_PLAN.md`; this crate is its phase 1, the
//! CLI. A job is a directory ([`job`]), so it can be resumed after a crash and
//! watched from another shell, and the server can take the format over later.

pub mod align;
pub mod colmap;
pub mod job;
pub mod params;
pub mod stages;
pub mod tools;
pub mod trainer;
