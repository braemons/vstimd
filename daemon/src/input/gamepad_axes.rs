//! Gamepad stick state for the gamepad input-device backend (`gamepad` feature).
//!
//! One background thread owns `gilrs` and publishes the sticks of every
//! connected pad to atomics; a device overridden to `gamepad:N` reads pad N's
//! every frame without touching `gilrs` on the render thread.

use std::sync::atomic::{AtomicU32, Ordering};

/// Pads tracked; later pads are ignored.
pub const MAX_PADS: usize = 4;
/// Left stick Y, left stick X, right stick X, right stick Y — the axis order
/// a gamepad-overridden device's axes 0..3 read.
pub const STICKS: usize = 4;

static STATE: [[AtomicU32; STICKS]; MAX_PADS] =
    [const { [const { AtomicU32::new(0) }; STICKS] }; MAX_PADS];

/// −1..1 for axis `axis` of pad `pad` (0 when out of range or unplugged).
pub fn stick(pad: usize, axis: usize) -> f64 {
    if pad >= MAX_PADS || axis >= STICKS {
        return 0.0;
    }
    f64::from(f32::from_bits(STATE[pad][axis].load(Ordering::Relaxed)))
}

#[cfg(feature = "gamepad")]
pub fn spawn() {
    let spawned = std::thread::Builder::new().name("gamepad".into()).spawn(|| {
        let mut gilrs = match gilrs::Gilrs::new() {
            Ok(g) => g,
            Err(e) => {
                log::warn!("input: gamepad support unavailable: {e}");
                return;
            }
        };
        let order: std::collections::HashMap<gilrs::GamepadId, usize> = Default::default();
        let mut order = order;
        loop {
            if crate::process::shutdown::is_requested() {
                return;
            }
            while let Some(gilrs::Event { id, event, .. }) = gilrs.next_event_blocking(Some(std::time::Duration::from_millis(100))) {
                let next = order.len();
                let pad = *order.entry(id).or_insert(next);
                if pad >= MAX_PADS {
                    continue;
                }
                match event {
                    gilrs::EventType::AxisChanged(axis, value, _) => {
                        let slot = match axis {
                            gilrs::Axis::LeftStickY => 0,
                            gilrs::Axis::LeftStickX => 1,
                            gilrs::Axis::RightStickX => 2,
                            gilrs::Axis::RightStickY => 3,
                            _ => continue,
                        };
                        STATE[pad][slot].store(value.to_bits(), Ordering::Relaxed);
                    }
                    gilrs::EventType::Disconnected => {
                        for s in &STATE[pad] {
                            s.store(0f32.to_bits(), Ordering::Relaxed);
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    if let Err(e) = spawned {
        log::error!("input: could not start the gamepad thread: {e}");
    }
}
