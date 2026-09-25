use std::sync::atomic::Ordering;

use crate::layout::{
    AXES_OFFSET, AXIS_NAME_LEN, AxisRecord, MAGIC, MAX_AXES, MAX_READ_SPINS, NAME_LEN, STATE_OFFSET,
    Semantic, StateSection, VERSION, VinputHeader,
};

/// A read gave up because the sequence stayed odd or kept changing for
/// [`MAX_READ_SPINS`] attempts — the producer is stopped mid-write, or writing
/// so fast that no copy completes. The caller keeps its previous snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Torn;

impl std::fmt::Display for Torn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("input device read torn: producer mid-write")
    }
}

impl std::error::Error for Torn {}

/// One axis as a producer declares it and a consumer reads it back.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisDesc {
    pub name: String,
    pub semantic: Semantic,
    pub scale: f32,
    pub deadzone: f32,
}

impl AxisDesc {
    pub fn new(name: impl Into<String>, semantic: Semantic, scale: f32) -> Self {
        Self { name: name.into(), semantic, scale, deadzone: 0.0 }
    }

    pub fn with_deadzone(mut self, deadzone: f32) -> Self {
        self.deadzone = deadzone;
        self
    }
}

/// A mapped segment. Shared by [`VinputOwner`](crate::VinputOwner) and
/// [`VinputClient`](crate::VinputClient); every accessor is `&self`.
pub(crate) struct Segment {
    pub(crate) ptr: *mut u8,
}

// SAFETY: the pointer is a MAP_SHARED mapping that lives as long as `Segment`
// and never moves. The header and axis table are written only before the
// segment is handed out (owner creation) and read-only afterwards. Every field
// written after that is an atomic, loaded and stored through `&` references, so
// concurrent access from any thread is sound. Only one thread writes values —
// `VinputOwner::write` takes `&mut self`.
unsafe impl Send for Segment {}
unsafe impl Sync for Segment {}

fn nul_terminated(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub(crate) fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out-pointer; CLOCK_MONOTONIC always exists on Linux.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

impl Segment {
    pub(crate) fn header(&self) -> &VinputHeader {
        // SAFETY: offset 0 of a SHM_SIZE mapping; the header is plain data.
        unsafe { &*(self.ptr as *const VinputHeader) }
    }

    fn axis(&self, i: usize) -> &AxisRecord {
        debug_assert!(i < MAX_AXES);
        // SAFETY: AXES_OFFSET + i·64 lies inside the mapping for i < MAX_AXES
        // (asserted in layout.rs).
        unsafe { &*(self.ptr.add(AXES_OFFSET + i * 64) as *const AxisRecord) }
    }

    pub(crate) fn state(&self) -> &StateSection {
        // SAFETY: STATE_OFFSET + size_of::<StateSection>() <= SHM_SIZE.
        unsafe { &*(self.ptr.add(STATE_OFFSET) as *const StateSection) }
    }

    /// Magic and version match this crate, and the axis count is in range.
    pub(crate) fn is_valid(&self) -> bool {
        let h = self.header();
        h.magic == MAGIC && h.version == VERSION && h.n_axes as usize <= MAX_AXES
    }

    pub(crate) fn n_axes(&self) -> usize {
        (self.header().n_axes as usize).min(MAX_AXES)
    }

    pub(crate) fn device_name(&self) -> String {
        nul_terminated(&self.header().name)
    }

    pub(crate) fn axes(&self) -> Vec<AxisDesc> {
        (0..self.n_axes())
            .map(|i| {
                let a = self.axis(i);
                AxisDesc {
                    name: nul_terminated(&a.name),
                    semantic: Semantic::from_u8(a.semantic).unwrap_or(Semantic::Absolute),
                    scale: a.scale,
                    deadzone: a.deadzone,
                }
            })
            .collect()
    }

    /// Write the header and axis table. Owner-only, before the segment is shared.
    ///
    /// # Safety
    /// No other reference to the header or axis table may exist yet.
    pub(crate) unsafe fn write_description(&self, name: &str, axes: &[AxisDesc]) {
        fn copy_name(dst: *mut u8, cap: usize, src: &str) {
            let n = src.len().min(cap - 1);
            // SAFETY: `dst` points at `cap` writable bytes; `n < cap`.
            unsafe {
                std::ptr::write_bytes(dst, 0, cap);
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst, n);
            }
        }
        let h = self.ptr as *mut VinputHeader;
        // SAFETY: the caller guarantees exclusive access; all offsets are in range.
        unsafe {
            std::ptr::addr_of_mut!((*h).magic).write(MAGIC);
            std::ptr::addr_of_mut!((*h).version).write(VERSION);
            std::ptr::addr_of_mut!((*h).n_axes).write(axes.len() as u32);
            copy_name(std::ptr::addr_of_mut!((*h).name) as *mut u8, NAME_LEN, name);
            for (i, a) in axes.iter().enumerate() {
                let r = self.ptr.add(AXES_OFFSET + i * 64) as *mut AxisRecord;
                copy_name(std::ptr::addr_of_mut!((*r).name) as *mut u8, AXIS_NAME_LEN, &a.name);
                std::ptr::addr_of_mut!((*r).semantic).write(a.semantic as u8);
                std::ptr::addr_of_mut!((*r).scale).write(a.scale);
                std::ptr::addr_of_mut!((*r).deadzone).write(a.deadzone);
            }
        }
    }

    /// The write half of the seqlock. Single writer.
    pub(crate) fn write(&self, values: &[f64]) -> usize {
        let s = self.state();
        let n = values.len().min(self.n_axes());
        // Read the clock before opening the write, to keep the odd window — the
        // time a reader has to wait — down to a few atomic stores.
        let now = monotonic_ns();
        s.seq.fetch_add(1, Ordering::AcqRel); // odd: writing
        for (slot, v) in s.values.iter().zip(&values[..n]) {
            slot.store(v.to_bits(), Ordering::Release);
        }
        s.heartbeat_ns.store(now, Ordering::Release);
        s.write_count.fetch_add(1, Ordering::AcqRel);
        s.seq.fetch_add(1, Ordering::AcqRel); // even: done
        n
    }

    /// The read half: a coherent copy of up to `buf.len()` values, or [`Torn`]
    /// after [`MAX_READ_SPINS`] attempts. Never allocates, never blocks.
    pub(crate) fn read_into(&self, buf: &mut [f64]) -> Result<usize, Torn> {
        let s = self.state();
        let n = buf.len().min(self.n_axes());
        for _ in 0..MAX_READ_SPINS {
            let before = s.seq.load(Ordering::Acquire);
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            for (dst, slot) in buf[..n].iter_mut().zip(&s.values) {
                *dst = f64::from_bits(slot.load(Ordering::Acquire));
            }
            if s.seq.load(Ordering::Acquire) == before {
                return Ok(n);
            }
            std::hint::spin_loop();
        }
        Err(Torn)
    }

    /// Nanoseconds since the producer last wrote (or created the segment).
    pub(crate) fn age_ns(&self) -> u64 {
        monotonic_ns().saturating_sub(self.state().heartbeat_ns.load(Ordering::Acquire))
    }

    pub(crate) fn write_count(&self) -> u64 {
        self.state().write_count.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VinputClient, VinputOwner};

    fn name(tag: &str) -> String {
        format!("/vinput_unit_{}_{tag}", std::process::id())
    }

    #[test]
    fn a_writer_stopped_mid_write_gives_torn_not_a_hang() {
        let n = name("torn");
        let owner = VinputOwner::create(&n, &[AxisDesc::new("x", Semantic::Absolute, 1.0)]).unwrap();
        let client = VinputClient::open(&n).unwrap();
        // What a producer SIGSTOPped between the two sequence bumps leaves behind.
        owner.seg.state().seq.fetch_add(1, Ordering::AcqRel);
        let start = std::time::Instant::now();
        assert_eq!(client.read_into(&mut [0.0]), Err(Torn));
        assert!(start.elapsed() < std::time::Duration::from_millis(10));
    }

    #[test]
    fn a_wrong_magic_is_refused() {
        let n = name("magic");
        let owner = VinputOwner::create(&n, &[AxisDesc::new("x", Semantic::Absolute, 1.0)]).unwrap();
        // SAFETY: test-only corruption of our own mapping.
        unsafe { std::ptr::addr_of_mut!((*(owner.seg.ptr as *mut VinputHeader)).magic).write(0xdead) };
        let err = VinputClient::open(&n).err().expect("must refuse");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        // SAFETY: as above.
        unsafe { std::ptr::addr_of_mut!((*(owner.seg.ptr as *mut VinputHeader)).magic).write(MAGIC) };
        unsafe { std::ptr::addr_of_mut!((*(owner.seg.ptr as *mut VinputHeader)).version).write(VERSION + 1) };
        assert!(VinputClient::open(&n).is_err(), "a version mismatch is refused too");
    }
}
