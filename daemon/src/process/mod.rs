//! Process-level concerns that are neither scene nor rendering: how the
//! process ends, and how its render thread is scheduled.
//!
//! `log_buffer` deliberately stays at the crate root — it is the in-process
//! log sink the overlay reads, and the seed of a future logging/events
//! module rather than process management.

pub mod sched;
pub mod shutdown;
