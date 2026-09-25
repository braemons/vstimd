use std::io;

use crate::layout::SHM_SIZE;
use crate::segment::{AxisDesc, Segment, Torn};

/// Opens a device segment read-only. vstimd holds one per device.
///
/// Open it once, off the render thread; [`read_into`](Self::read_into) and
/// [`age_ns`](Self::age_ns) are then allocation-free atomic loads, safe to call
/// every frame.
pub struct VinputClient {
    seg: Segment,
}

impl VinputClient {
    /// Open the existing segment `shm_name`.
    ///
    /// Fails if it does not exist — it never creates one, so a mistyped device
    /// name is an error rather than a silently motionless stimulus — or if its
    /// magic, version or axis count do not match this crate.
    pub fn open(shm_name: &str) -> io::Result<Self> {
        let name = std::ffi::CString::new(shm_name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "shm name contains NUL"))?;
        // SAFETY: valid C string; O_RDONLY without O_CREAT never creates.
        let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDONLY, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is an open descriptor.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: `st` is a valid out-pointer.
        if unsafe { libc::fstat(fd, &mut st) } < 0 || (st.st_size as usize) < SHM_SIZE {
            // SAFETY: closing our descriptor.
            unsafe { libc::close(fd) };
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{shm_name} is not a vinput segment (too small)"),
            ));
        }
        // SAFETY: mapping SHM_SIZE bytes of an object at least that large, read-only.
        let ptr = unsafe {
            libc::mmap(std::ptr::null_mut(), SHM_SIZE, libc::PROT_READ, libc::MAP_SHARED, fd, 0)
        };
        let mmap_err = (ptr == libc::MAP_FAILED).then(io::Error::last_os_error);
        // SAFETY: the mapping keeps the object alive.
        unsafe { libc::close(fd) };
        if let Some(e) = mmap_err {
            return Err(e);
        }
        let seg = Segment { ptr: ptr as *mut u8 };
        if !seg.is_valid() {
            // SAFETY: unmapping what was just mapped.
            unsafe { libc::munmap(ptr, SHM_SIZE) };
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{shm_name} has the wrong magic, version or axis count for vinput"),
            ));
        }
        Ok(Self { seg })
    }

    /// Copy a coherent snapshot of up to `buf.len()` axis values into `buf`.
    /// Returns how many were copied, or [`Torn`] if no consistent copy could be
    /// taken within [`MAX_READ_SPINS`](crate::MAX_READ_SPINS) attempts — keep
    /// the previous snapshot then. Never allocates or blocks.
    pub fn read_into(&self, buf: &mut [f64]) -> Result<usize, Torn> {
        self.seg.read_into(buf)
    }

    /// Nanoseconds since the producer last wrote. How stale is too stale is the
    /// consumer's policy, not this crate's.
    pub fn age_ns(&self) -> u64 {
        self.seg.age_ns()
    }

    pub fn n_axes(&self) -> usize {
        self.seg.n_axes()
    }

    /// Allocates; call when opening, not per frame.
    pub fn axes(&self) -> Vec<AxisDesc> {
        self.seg.axes()
    }

    pub fn device_name(&self) -> String {
        self.seg.device_name()
    }

    /// Completed writes since the producer created the segment.
    pub fn write_count(&self) -> u64 {
        self.seg.write_count()
    }
}

impl Drop for VinputClient {
    fn drop(&mut self) {
        // SAFETY: unmapping this client's mapping; the name is the owner's to unlink.
        unsafe { libc::munmap(self.seg.ptr as *mut libc::c_void, SHM_SIZE) };
    }
}
