//! Shared memory between an input-producing process — a wheel or treadmill
//! reader, an eye tracker — and vstimd, which reads it every frame.
//!
//! Mirrors the `vtl` crate (`shm_open` + `mmap`, a `#[repr(C)]` header checked
//! by magic and version, atomics in a state section) with three additions this
//! contract needs and `vtl` does not:
//!
//! - a **seqlock**, so a multi-axis read is a coherent snapshot rather than
//!   values from two different writes;
//! - **`f64` values**, so a cumulative count stays exact for a whole session;
//! - a **heartbeat**, so a reader can tell a live producer from a dead one.
//!
//! The *producer* owns the segment ([`VinputOwner`]) and vstimd is a read-only
//! [`VinputClient`] — the reverse of `vtl`, because the device exists
//! independently of any one vstimd run. Layout: `dev/INPUT_LATENCY.md` §4.

pub mod layout;
mod segment;
mod owner;
mod client;

pub use client::VinputClient;
pub use layout::{MAX_AXES, MAX_READ_SPINS, Semantic};
pub use owner::VinputOwner;
pub use segment::{AxisDesc, Torn};
