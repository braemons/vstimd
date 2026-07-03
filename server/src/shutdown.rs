use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static REQUESTED: AtomicBool = AtomicBool::new(false);
static FATAL_REASON: OnceLock<String> = OnceLock::new();

/// Signal the process to shut down. Safe to call from signal handlers and
/// async contexts. Checked by all render loop backends each frame.
///
/// Release/Acquire pairing ensures the store from a signal handler (possibly
/// on a different thread on ARM) is visible to the render loop's load.
pub fn request() {
    REQUESTED.store(true, Ordering::Release);
}

/// Like `request()`, but marks the shutdown as triggered by an unrecoverable
/// runtime error rather than a normal quit (Ctrl+Q, SIGINT, SIGTERM). Render
/// loops still exit through the normal path so Drop guards (DRM/VT restore)
/// run; `main` checks `fatal_reason()` after the backend returns and exits
/// with a non-zero code and a printed error.
pub fn request_fatal(reason: impl Into<String>) {
    let _ = FATAL_REASON.set(reason.into());
    REQUESTED.store(true, Ordering::Release);
}

pub fn is_requested() -> bool {
    REQUESTED.load(Ordering::Acquire)
}

pub fn fatal_reason() -> Option<&'static str> {
    FATAL_REASON.get().map(String::as_str)
}
