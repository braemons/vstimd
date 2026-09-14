//! The on-memory layout. A producer in another language implements exactly this.
//!
//! ```text
//! offset 0      VinputHeader    magic "VIN1", version, n_axes, device name   (128 B)
//! offset 128    AxisRecord[16]  name, semantic, scale, deadzone               (64 B each)
//! offset 0x1000 StateSection    seq, heartbeat, write count, [f64 bits; 16]   (160 B)
//! ```
//!
//! All integers little-endian native; values are `f64` stored as their bits in
//! `AtomicU64`.
//!
//! ## Write protocol (one writer)
//!
//! 1. `seq += 1` — now odd: a write is in progress
//! 2. store every value, then the heartbeat, then `write_count += 1`
//! 3. `seq += 1` — even again
//!
//! ## Read protocol
//!
//! Load `seq`; if odd, retry. Copy the values. Load `seq` again; if it changed,
//! retry. Give up after [`MAX_READ_SPINS`] attempts: a producer stopped
//! mid-write must not hang the reader. A producer should not write in a loop
//! with no pause; a device's sample rate is pause enough.

use std::sync::atomic::{AtomicU32, AtomicU64};

pub const MAGIC: u32 = 0x5649_4E31; // "VIN1"
pub const VERSION: u32 = 1;

pub const MAX_AXES: usize = 16;
pub const NAME_LEN: usize = 64;
pub const AXIS_NAME_LEN: usize = 48;

pub const HEADER_OFFSET: usize = 0;
pub const AXES_OFFSET: usize = 128;
pub const STATE_OFFSET: usize = 0x1000;
pub const SHM_SIZE: usize = 0x2000;

/// How many times a read retries before reporting [`Torn`](crate::Torn): at
/// most a few microseconds of spinning. A write holds the sequence odd for a
/// handful of atomic stores, so only a writer stopped mid-write — or one in a
/// loop with no pause at all — outlasts it.
pub const MAX_READ_SPINS: u32 = 1024;

/// What an axis value means, which decides how a consumer uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Semantic {
    /// The current value — an eye tracker's gaze, a joystick's position.
    Absolute = 0,
    /// A running total that is never reset — wheel ticks, treadmill distance.
    /// The consumer differences successive reads, so no count is ever lost or
    /// counted twice whatever the write and read rates. Never write a delta.
    Cumulative = 1,
    /// A velocity — a gamepad stick. The consumer multiplies by frame time.
    Rate = 2,
}

impl Semantic {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Absolute),
            1 => Some(Self::Cumulative),
            2 => Some(Self::Rate),
            _ => None,
        }
    }
}

/// Offset 0, 128 bytes. Written once by the owner at creation.
#[repr(C)]
pub struct VinputHeader {
    pub magic: u32,
    pub version: u32,
    pub n_axes: u32,
    pub _pad0: u32,
    /// NUL-terminated UTF-8 device name.
    pub name: [u8; NAME_LEN],
    pub _pad: [u8; 48],
}

const _: () = assert!(std::mem::size_of::<VinputHeader>() == 128);

/// One axis description, 64 bytes. Written once by the owner at creation.
#[repr(C)]
pub struct AxisRecord {
    /// NUL-terminated UTF-8 axis name.
    pub name: [u8; AXIS_NAME_LEN],
    /// A [`Semantic`] discriminant.
    pub semantic: u8,
    pub _pad0: [u8; 3],
    /// Multiplier from raw value to the axis' unit (counts → cm, say).
    pub scale: f32,
    /// Absolute values within ±deadzone of zero read as zero.
    pub deadzone: f32,
    pub _pad1: [u8; 4],
}

const _: () = assert!(std::mem::size_of::<AxisRecord>() == 64);
const _: () = assert!(AXES_OFFSET + MAX_AXES * 64 <= STATE_OFFSET);

/// Offset 0x1000, 160 bytes. Written by the producer on every write.
#[repr(C)]
pub struct StateSection {
    /// Seqlock counter: odd while a write is in progress.
    pub seq: AtomicU32,
    pub _pad0: u32,
    /// `CLOCK_MONOTONIC` nanoseconds at the end of the last write.
    pub heartbeat_ns: AtomicU64,
    /// Completed writes since creation.
    pub write_count: AtomicU64,
    /// `f64::to_bits` of each axis value.
    pub values: [AtomicU64; MAX_AXES],
}

const _: () = assert!(std::mem::size_of::<StateSection>() == 152);
const _: () = assert!(STATE_OFFSET + std::mem::size_of::<StateSection>() <= SHM_SIZE);
