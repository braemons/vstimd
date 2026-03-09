/// Holds a live value and a staging copy for deferred mode.
/// The copy is written during deferred mode; `flip()` promotes it to live.
#[derive(Clone, Copy, Default)]
pub struct Deferred<T: Copy + Default> {
    pub live: T,
    pub copy: T,
}

impl<T: Copy + Default> Deferred<T> {
    pub fn new(value: T) -> Self {
        Self {
            live: value,
            copy: value,
        }
    }

    /// Write to the copy slot (deferred=true) or live slot (deferred=false).
    pub fn set(&mut self, deferred: bool, value: T) {
        if deferred {
            self.copy = value;
        } else {
            self.live = value;
        }
    }

    pub fn get(&self) -> &T {
        &self.live
    }

    /// Snapshot live → copy. Call at start of deferred mode.
    pub fn make_copy(&mut self) {
        self.copy = self.live;
    }

    /// Promote copy → live. Call at frame boundary after deferred mode ends.
    pub fn flip(&mut self) {
        self.live = self.copy;
    }
}
