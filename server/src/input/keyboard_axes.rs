//! Arrow-key state for the keyboard input-device backend.
//!
//! Process-wide atomics rather than plumbing: both render backends (winit and
//! the libinput console) call [`set_arrow`] from their key handlers, and a
//! device overridden to the keyboard reads [`direction`] every frame.

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

static UP: AtomicBool = AtomicBool::new(false);
static DOWN: AtomicBool = AtomicBool::new(false);
static LEFT: AtomicBool = AtomicBool::new(false);
static RIGHT: AtomicBool = AtomicBool::new(false);

fn flag(a: Arrow) -> &'static AtomicBool {
    match a {
        Arrow::Up => &UP,
        Arrow::Down => &DOWN,
        Arrow::Left => &LEFT,
        Arrow::Right => &RIGHT,
    }
}

/// Record an arrow key going down or up.
pub fn set_arrow(arrow: Arrow, pressed: bool) {
    flag(arrow).store(pressed, Ordering::Relaxed);
}

/// −1, 0 or +1 for a keyboard-driven axis: axis 0 is Up (+) / Down (−),
/// axis 1 is Right (+) / Left (−), any further axis is 0.
pub fn direction(axis: usize) -> f64 {
    let held = |a| flag(a).load(Ordering::Relaxed) as i8 as f64;
    match axis {
        0 => held(Arrow::Up) - held(Arrow::Down),
        1 => held(Arrow::Right) - held(Arrow::Left),
        _ => 0.0,
    }
}
