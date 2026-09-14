use std::ffi::CString;
use std::io;
use std::sync::atomic::Ordering;

use crate::layout::{MAX_AXES, SHM_SIZE};
use crate::segment::{AxisDesc, Segment, monotonic_ns};

/// Creates, writes and owns a device segment. The producer process holds one.
///
/// Dropping it unmaps and unlinks the segment. A reader that still has it
/// mapped keeps reading the last values with a heartbeat that no longer moves —
/// which is how a consumer notices the producer is gone.
pub struct VinputOwner {
    pub(crate) seg: Segment,
    name: CString,
}

impl VinputOwner {
    /// Create the segment `shm_name` (e.g. `"/vstimd_wheel"`) describing `axes`.
    ///
    /// A segment left behind by a producer that crashed is removed first, as
    /// `vtl` does, so a restarted producer comes back under the same name.
    pub fn create(shm_name: &str, axes: &[AxisDesc]) -> io::Result<Self> {
        if axes.is_empty() || axes.len() > MAX_AXES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("an input device needs 1..={MAX_AXES} axes, got {}", axes.len()),
            ));
        }
        let name = CString::new(shm_name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "shm name contains NUL"))?;

        // SAFETY: `name` is a valid C string. A missing segment is not an error.
        unsafe { libc::shm_unlink(name.as_ptr()) };
        // SAFETY: valid C string; O_EXCL makes a concurrent creator fail rather
        // than share a half-initialised segment.
        let fd = unsafe {
            libc::shm_open(name.as_ptr(), libc::O_CREAT | libc::O_EXCL | libc::O_RDWR, 0o644)
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is an open shm descriptor.
        if unsafe { libc::ftruncate(fd, SHM_SIZE as libc::off_t) } < 0 {
            let err = io::Error::last_os_error();
            // SAFETY: closing and unlinking what was just created.
            unsafe {
                libc::close(fd);
                libc::shm_unlink(name.as_ptr());
            }
            return Err(err);
        }
        // SAFETY: mapping SHM_SIZE bytes of an fd truncated to that size.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                SHM_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        let mmap_err = (ptr == libc::MAP_FAILED).then(io::Error::last_os_error);
        // SAFETY: the mapping, if any, holds its own reference to the object.
        unsafe { libc::close(fd) };
        if let Some(e) = mmap_err {
            // SAFETY: removing the segment this call created.
            unsafe { libc::shm_unlink(name.as_ptr()) };
            return Err(e);
        }

        let seg = Segment { ptr: ptr as *mut u8 };
        // SAFETY: the mapping is fresh (zero-filled) and not yet shared by
        // anything in this process; a reader in another process that opens it
        // now sees a zero magic and refuses it until this completes.
        unsafe { seg.write_description(shm_name, axes) };
        seg.state().heartbeat_ns.store(monotonic_ns(), Ordering::Release);
        Ok(Self { seg, name })
    }

    /// Publish one sample: every axis value, a fresh heartbeat. Values beyond
    /// the declared axes are ignored; returns how many were written.
    ///
    /// `&mut self` because the seqlock admits one writer.
    pub fn write(&mut self, values: &[f64]) -> usize {
        self.seg.write(values)
    }

    pub fn axes(&self) -> Vec<AxisDesc> {
        self.seg.axes()
    }

    pub fn write_count(&self) -> u64 {
        self.seg.write_count()
    }
}

impl Drop for VinputOwner {
    fn drop(&mut self) {
        // SAFETY: unmapping this owner's mapping and unlinking its name.
        unsafe {
            libc::munmap(self.seg.ptr as *mut libc::c_void, SHM_SIZE);
            libc::shm_unlink(self.name.as_ptr());
        }
    }
}
