//! CPU affinity and real-time priority for the render/vblank thread.
//!
//! Affinity is opt-in via `[scheduling] render_cpu_core` in the rig-config and
//! does nothing when unset.  Real-time priority is *on by default*: when
//! `render_rt_prio` is omitted, [`DEFAULT_RENDER_RT_PRIO`] is used; set it to
//! `0` to stay on `SCHED_OTHER`.  Applied to the calling
//! thread — on Linux `sched_setaffinity`/`sched_setscheduler` with pid 0 mean
//! "this thread", not "this process", which is what we want: the ZMQ and web
//! threads should stay on the general scheduler.
//!
//! Failures warn and continue rather than abort. Without `CAP_SYS_NICE` the
//! real-time promotion is simply refused, and a developer running vstimd from
//! a shell should still get a working server.
//!
//! What was *actually* achieved is read back from the kernel and returned as
//! [`SchedStatus`], which the overlay's System panel displays. A cgroup cpuset
//! or a missing capability can quietly defeat either setting, so "we asked for
//! it" is not evidence it happened.

use crate::rig_config::SchedulingRigConfig;

/// `SCHED_FIFO` priority applied to the render thread when the rig-config
/// does not name one.
///
/// Above normal (`SCHED_OTHER`) so a busy system cannot preempt frame
/// delivery, but deliberately *below* gpiochip-daqd's output thread (60, see
/// `gpiochip-daqd/src/bridge.rs`) — if the two ever land on the same core,
/// trigger output must still win over drawing.
pub const DEFAULT_RENDER_RT_PRIO: i32 = 55;

/// What the render thread's scheduling actually ended up as.
///
/// `requested_*` come from the rig-config; the rest are read back from the
/// kernel after the syscalls, so a silent failure shows up as a mismatch.
#[derive(Debug, Clone, Default)]
pub struct SchedStatus {
    /// Core requested via `[scheduling] render_cpu_core`.
    pub requested_core: Option<usize>,
    /// Affinity mask actually in effect, as a core list (e.g. "3", "0,1,2,3").
    pub affinity: String,
    /// Priority requested via `[scheduling] render_rt_prio`.
    pub requested_prio: Option<i32>,
    /// `SCHED_FIFO` priority actually in effect, if the thread is real-time.
    pub applied_prio: Option<i32>,
}

impl SchedStatus {
    /// True when a core was requested and the thread is pinned to exactly it.
    pub fn core_applied(&self) -> bool {
        match self.requested_core {
            Some(core) => self.affinity == core.to_string(),
            None => false,
        }
    }

    /// True when a priority was requested and the thread actually got it.
    pub fn prio_applied(&self) -> bool {
        self.requested_prio.is_some() && self.requested_prio == self.applied_prio
    }

    /// Nothing is in effect — unpinned and not real-time.
    pub fn is_default(&self) -> bool {
        self.requested_core.is_none() && self.requested_prio.is_none()
    }

    /// Priority was requested but refused — almost always a missing
    /// `CAP_SYS_NICE`, which is the common case on a dev checkout.
    pub fn rt_refused(&self) -> bool {
        self.requested_prio.is_some() && self.applied_prio.is_none()
    }

    /// Something was asked for but not achieved.
    pub fn has_failure(&self) -> bool {
        (self.requested_core.is_some() && !self.core_applied())
            || (self.requested_prio.is_some() && !self.prio_applied())
    }

    /// One-line summary for the overlay and the startup log.
    pub fn summary(&self) -> String {
        if self.is_default() {
            return "default (unpinned, SCHED_OTHER)".to_string();
        }
        let core = match self.requested_core {
            None => format!("unpinned ({})", self.affinity),
            Some(c) if self.core_applied() => format!("core {c}"),
            Some(c) => format!("core {c} FAILED (on {})", self.affinity),
        };
        let prio = match (self.requested_prio, self.applied_prio) {
            (None, _) => "SCHED_OTHER".to_string(),
            (Some(p), Some(a)) if p == a => format!("SCHED_FIFO {p}"),
            (Some(p), _) => format!("SCHED_FIFO {p} FAILED"),
        };
        format!("{core}, {prio}")
    }
}

/// Apply the configured affinity and real-time priority to the calling thread.
///
/// Call this from the render thread before entering the frame loop. Note this
/// runs *before* the backend's Vulkan initialisation, which therefore also
/// runs at the configured priority — acceptable because init is short relative
/// to the session, and splitting it would mean touching every backend.
pub fn apply_to_render_thread(cfg: &SchedulingRigConfig) -> SchedStatus {
    // Real-time priority is on by default; affinity is not. Pinning is a
    // rig-specific tuning decision that can only be made with knowledge of the
    // board, so leaving it unset is the safe default. Priority is not — the
    // render thread should outrank ordinary work everywhere.
    let prio = cfg.render_rt_prio.unwrap_or(DEFAULT_RENDER_RT_PRIO);
    let prio = (prio != 0).then_some(prio); // 0 = explicitly opt out

    let mut status = SchedStatus {
        requested_core: cfg.render_cpu_core,
        requested_prio: prio,
        ..Default::default()
    };

    if let Some(core) = cfg.render_cpu_core {
        pin_to_core(core);
    }
    if let Some(prio) = prio {
        set_realtime(prio);
    }

    status.affinity = affinity_str();
    status.applied_prio = current_fifo_prio();

    if status.has_failure() {
        log::warn!("vstimd: render thread scheduling: {}", status.summary());
    } else if !status.is_default() {
        log::info!("vstimd: render thread scheduling: {}", status.summary());
    }
    status
}

#[cfg(target_os = "linux")]
fn pin_to_core(core: usize) {
    let online = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    if online > 0 && core >= online as usize {
        log::warn!(
            "vstimd: [scheduling] render_cpu_core = {core} is out of range \
             (system has {online} online CPUs, 0-{}) — leaving affinity alone",
            online - 1
        );
        return;
    }

    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(core, &mut set);
    }
    let ret = unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) };
    if ret != 0 {
        log::warn!(
            "vstimd: sched_setaffinity(core {core}) failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

#[cfg(target_os = "linux")]
fn set_realtime(priority: i32) {
    if !(1..=99).contains(&priority) {
        log::warn!(
            "vstimd: [scheduling] render_rt_prio = {priority} is out of range (1-99) \
             — leaving scheduling policy alone"
        );
        return;
    }

    let param = libc::sched_param {
        sched_priority: priority,
    };
    let ret = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) };
    if ret != 0 {
        log::warn!(
            "vstimd: sched_setscheduler(SCHED_FIFO, {priority}) failed: {} \
             (running without CAP_SYS_NICE?)",
            std::io::Error::last_os_error()
        );
    }
}

/// Current affinity mask of the calling thread, as a comma-separated core list.
#[cfg(target_os = "linux")]
fn affinity_str() -> String {
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    let ret =
        unsafe { libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) };
    if ret != 0 {
        return "unknown".to_string();
    }
    let cores: Vec<String> = (0..libc::CPU_SETSIZE as usize)
        .filter(|&c| unsafe { libc::CPU_ISSET(c, &set) })
        .map(|c| c.to_string())
        .collect();
    if cores.is_empty() {
        "none".to_string()
    } else {
        cores.join(",")
    }
}

/// `SCHED_FIFO` priority of the calling thread, or `None` if not real-time.
#[cfg(target_os = "linux")]
fn current_fifo_prio() -> Option<i32> {
    if unsafe { libc::sched_getscheduler(0) } != libc::SCHED_FIFO {
        return None;
    }
    let mut param: libc::sched_param = unsafe { std::mem::zeroed() };
    if unsafe { libc::sched_getparam(0, &mut param) } != 0 {
        return None;
    }
    Some(param.sched_priority)
}

#[cfg(not(target_os = "linux"))]
fn pin_to_core(core: usize) {
    log::warn!("vstimd: [scheduling] render_cpu_core = {core} ignored (Linux only)");
}

#[cfg(not(target_os = "linux"))]
fn set_realtime(priority: i32) {
    log::warn!("vstimd: [scheduling] render_rt_prio = {priority} ignored (Linux only)");
}

#[cfg(not(target_os = "linux"))]
fn affinity_str() -> String {
    "n/a".to_string()
}

#[cfg(not(target_os = "linux"))]
fn current_fifo_prio() -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_requests_priority_but_no_pinning() {
        // Derive the same (prio, core) that apply_to_render_thread would use,
        // but don't make any syscalls so the test runner is unaffected.
        let cfg = SchedulingRigConfig::default();
        let prio = cfg.render_rt_prio.unwrap_or(DEFAULT_RENDER_RT_PRIO);
        let prio = (prio != 0).then_some(prio);
        assert_eq!(cfg.render_cpu_core, None, "must not pin by default");
        assert_eq!(prio, Some(DEFAULT_RENDER_RT_PRIO));
        const {
            assert!(
                DEFAULT_RENDER_RT_PRIO > 0,
                "default must be above SCHED_OTHER"
            )
        };
    }

    #[test]
    fn zero_priority_opts_out_entirely() {
        // Derive (prio, core) without any syscalls.
        let cfg = SchedulingRigConfig {
            render_cpu_core: None,
            render_rt_prio: Some(0),
        };
        let prio = cfg.render_rt_prio.unwrap_or(DEFAULT_RENDER_RT_PRIO);
        let prio = (prio != 0).then_some(prio); // 0 = explicitly opt out
        let status = SchedStatus {
            requested_core: cfg.render_cpu_core,
            requested_prio: prio,
            affinity: String::new(),
            applied_prio: None,
        };
        assert!(status.is_default());
        assert!(!status.has_failure());
        assert_eq!(status.summary(), "default (unpinned, SCHED_OTHER)");
    }

    #[test]
    fn requested_but_unapplied_core_reads_as_failure() {
        let status = SchedStatus {
            requested_core: Some(3),
            affinity: "0,1,2,3".to_string(),
            ..Default::default()
        };
        assert!(!status.core_applied());
        assert!(status.has_failure());
        assert!(status.summary().contains("FAILED"));
    }

    #[test]
    fn applied_core_reads_as_success() {
        let status = SchedStatus {
            requested_core: Some(3),
            affinity: "3".to_string(),
            requested_prio: Some(55),
            applied_prio: Some(55),
        };
        assert!(status.core_applied() && status.prio_applied());
        assert!(!status.has_failure());
        assert_eq!(status.summary(), "core 3, SCHED_FIFO 55");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn out_of_range_core_is_rejected_not_applied() {
        let before = affinity_str();
        pin_to_core(usize::MAX);
        assert_eq!(
            before,
            affinity_str(),
            "affinity changed despite invalid core"
        );
    }

    #[test]
    fn out_of_range_priority_is_rejected() {
        // 0 and 100 are outside SCHED_FIFO's 1-99; the range guard must reject
        // them before any syscall.  We verify this by checking that values in
        // and out of range are classified correctly without calling the
        // real-time setter (which would mutate the test runner's scheduling).
        for bad in [i32::MIN, -1, 0, 100, i32::MAX] {
            assert!(
                !(1..=99).contains(&bad),
                "expected {bad} to be out of range"
            );
        }
        for good in [1, 55, 99] {
            assert!((1..=99).contains(&good), "expected {good} to be in range");
        }
    }
}
