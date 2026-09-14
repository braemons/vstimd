//! Input devices as the scene sees them: named, with axes, sampled once per
//! frame into values an animation can use, whatever produced the numbers.
//!
//! The backend is the producer's shared-memory segment (`vinput`) in
//! production, or the keyboard when `--input-override` stands one in on a desk
//! with no rig hardware. Axes, scales and semantics come from the rig-config
//! either way, so the animations and the experiment script do not change.
//!
//! ## Staleness — the safety-critical part
//!
//! A producer silent for longer than `stale_after` is treated as gone:
//!
//! | semantic   | while stale                              |
//! |------------|------------------------------------------|
//! | rate       | reads 0 — motion stops                   |
//! | cumulative | no delta — held at its last value        |
//! | absolute   | holds its last sample                    |
//!
//! When it comes back, a cumulative axis adopts the new count as its baseline
//! instead of differencing against the count from before the crash. Nothing is
//! disabled and no frame fails: the experiment runs on with the stimulus still,
//! which is the safe way to fail.
//!
//! ## Threads
//!
//! [`InputRegistry::sample_all`] runs on the render thread, under the scene
//! write lock, every frame: atomic loads and arithmetic, no syscall, no
//! allocation. Opening a segment is a syscall, so it happens on the input
//! reconnect thread ([`spawn_reconnector`]), which opens outside the lock and
//! swaps the result in under it.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use vinput::{MAX_AXES, Semantic, VinputClient};

use crate::input::keyboard_axes;
use crate::rig_config::{InputDeviceRigConfig, InputRigConfig};
use crate::scene::SceneState;

/// One axis as configured on the rig.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub name: String,
    pub semantic: Semantic,
    pub scale: f32,
    pub deadzone: f32,
}

/// Where a device's raw numbers come from.
pub enum Backend {
    /// A producer's segment. `None` until it has been opened, and again after a
    /// failed reopen.
    Shm { shm_name: String, client: Option<VinputClient> },
    /// Arrow keys (see [`keyboard_axes`]), at `speed` axis units per second for
    /// rate and cumulative axes.
    Keyboard { speed: f64 },
}

impl Backend {
    /// A short label for the overlay and logs.
    pub fn label(&self) -> String {
        match self {
            Backend::Shm { shm_name, .. } => format!("shm {shm_name}"),
            Backend::Keyboard { speed } => format!("keyboard ({speed}/s)"),
        }
    }
}

/// This frame's reading of one device.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisFrame {
    /// Scaled value: an absolute axis' position, a rate axis' velocity (units
    /// per second), a cumulative axis' running total.
    pub value: f64,
    /// Cumulative axes: the scaled change since the previous frame. Zero for
    /// the other semantics, and on any frame without a valid pair of readings.
    pub delta: f64,
}

pub struct InputDevice {
    pub name: String,
    pub axes: Vec<Axis>,
    pub stale_after: Duration,
    pub backend: Backend,
    /// True while the producer is missing or silent. Logged on each transition.
    pub stale: bool,
    pub frame: [AxisFrame; MAX_AXES],
    /// Reads abandoned because the producer was mid-write.
    pub torn_reads: u64,
    raw: [f64; MAX_AXES],
    /// The previous raw reading of each cumulative axis, and whether it is a
    /// valid baseline to difference against.
    baseline: [f64; MAX_AXES],
    has_baseline: bool,
    last_write_count: u64,
    /// Raw keyboard integration for cumulative axes.
    keyboard_total: [f64; MAX_AXES],
}

impl InputDevice {
    pub fn from_rig(cfg: &InputDeviceRigConfig) -> Self {
        let axes = cfg
            .axis
            .iter()
            .take(MAX_AXES)
            .map(|a| Axis {
                name: a.name.clone(),
                semantic: a.semantic.into(),
                scale: a.scale,
                deadzone: a.deadzone,
            })
            .collect();
        Self::new(
            &cfg.name,
            axes,
            Duration::from_millis(cfg.stale_after_ms),
            Backend::Shm { shm_name: cfg.shm.clone(), client: None },
        )
    }

    pub fn new(name: &str, axes: Vec<Axis>, stale_after: Duration, backend: Backend) -> Self {
        let stale = matches!(backend, Backend::Shm { .. });
        Self {
            name: name.to_string(),
            axes,
            stale_after,
            backend,
            stale,
            frame: [AxisFrame::default(); MAX_AXES],
            torn_reads: 0,
            raw: [0.0; MAX_AXES],
            baseline: [0.0; MAX_AXES],
            has_baseline: false,
            last_write_count: 0,
            keyboard_total: [0.0; MAX_AXES],
        }
    }

    /// Index of the axis called `name`.
    pub fn axis_index(&self, name: &str) -> Option<usize> {
        self.axes.iter().position(|a| a.name == name)
    }

    /// Whether a segment is mapped (always true for the keyboard).
    pub fn is_connected(&self) -> bool {
        match &self.backend {
            Backend::Shm { client, .. } => client.is_some(),
            Backend::Keyboard { .. } => true,
        }
    }

    /// Read the backend and update [`Self::frame`]. Render thread; no syscall,
    /// no allocation.
    pub fn sample(&mut self, dt_s: f64) {
        let n = self.axes.len();
        let fresh = match &self.backend {
            Backend::Shm { client: Some(c), .. } => {
                let alive = c.age_ns() <= self.stale_after.as_nanos() as u64;
                if alive {
                    // A write count that went backwards is a producer that
                    // restarted within the staleness window: its counters
                    // restarted too, so the old baseline is meaningless.
                    let count = c.write_count();
                    if count < self.last_write_count {
                        self.has_baseline = false;
                    }
                    self.last_write_count = count;
                    match c.read_into(&mut self.raw[..n]) {
                        Ok(_) => {}
                        Err(vinput::Torn) => self.torn_reads += 1,
                    }
                }
                alive
            }
            Backend::Shm { client: None, .. } => false,
            Backend::Keyboard { speed } => {
                for (i, axis) in self.axes.iter().enumerate() {
                    let dir = keyboard_axes::direction(i);
                    let per_raw = f64::from(axis.scale).max(f64::MIN_POSITIVE);
                    self.raw[i] = match axis.semantic {
                        Semantic::Absolute => dir / per_raw,
                        Semantic::Rate => dir * speed / per_raw,
                        Semantic::Cumulative => {
                            self.keyboard_total[i] += dir * speed * dt_s / per_raw;
                            self.keyboard_total[i]
                        }
                    };
                }
                true
            }
        };

        if fresh == self.stale {
            self.stale = !fresh;
            if self.stale {
                log::warn!(
                    "input: device '{}' is stale ({}) — rate axes read 0 until it resumes",
                    self.name,
                    self.backend.label()
                );
            } else {
                log::info!("input: device '{}' resumed ({})", self.name, self.backend.label());
                // Adopt the resumed producer's counts as the new baseline.
                self.has_baseline = false;
            }
        }

        for (i, axis) in self.axes.iter().enumerate() {
            let scale = f64::from(axis.scale);
            let deadzone = f64::from(axis.deadzone);
            let f = &mut self.frame[i];
            match axis.semantic {
                Semantic::Absolute => {
                    if fresh {
                        let v = self.raw[i] * scale;
                        f.value = if v.abs() <= deadzone { 0.0 } else { v };
                    }
                    f.delta = 0.0;
                }
                Semantic::Rate => {
                    let v = self.raw[i] * scale;
                    f.value = if !fresh || v.abs() <= deadzone { 0.0 } else { v };
                    f.delta = 0.0;
                }
                Semantic::Cumulative => {
                    f.delta = if fresh && self.has_baseline {
                        (self.raw[i] - self.baseline[i]) * scale
                    } else {
                        0.0
                    };
                    if fresh {
                        f.value = self.raw[i] * scale;
                        self.baseline[i] = self.raw[i];
                    }
                }
            }
        }
        if fresh {
            self.has_baseline = true;
        }
    }
}

/// Every input device the rig declares, by name. Runtime state: never saved
/// into a scene-config, which names devices and nothing more.
#[derive(Default)]
pub struct InputRegistry {
    pub devices: Vec<InputDevice>,
}

impl InputRegistry {
    pub fn from_rig(cfg: &InputRigConfig) -> Self {
        Self { devices: cfg.device.iter().map(InputDevice::from_rig).collect() }
    }

    pub fn get(&self, name: &str) -> Option<&InputDevice> {
        self.devices.iter().find(|d| d.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut InputDevice> {
        self.devices.iter_mut().find(|d| d.name == name)
    }

    /// Replace a device's backend, keeping its axes — `--input-override`.
    pub fn override_backend(&mut self, name: &str, backend: Backend) -> Result<(), String> {
        let device = self
            .get_mut(name)
            .ok_or_else(|| format!("no input device named '{name}' in the rig-config"))?;
        log::warn!(
            "input: device '{name}' overridden: {} → {} — not the rig's hardware",
            device.backend.label(),
            backend.label()
        );
        device.stale = !matches!(backend, Backend::Keyboard { .. });
        device.backend = backend;
        device.has_baseline = false;
        Ok(())
    }

    /// Sample every device for this frame. Render thread, under the scene lock.
    pub fn sample_all(&mut self, dt_s: f64) {
        for d in &mut self.devices {
            d.sample(dt_s);
        }
    }
}

/// Open `shm_name` and check it carries the axes the rig expects: same count
/// or more, same semantics in the same order.
pub fn open_checked(shm_name: &str, axes: &[Axis]) -> Result<VinputClient, String> {
    let client = VinputClient::open(shm_name).map_err(|e| format!("{shm_name}: {e}"))?;
    let seg_axes = client.axes();
    if seg_axes.len() < axes.len() {
        return Err(format!(
            "{shm_name} has {} axes, the rig-config expects {}",
            seg_axes.len(),
            axes.len()
        ));
    }
    for (want, got) in axes.iter().zip(&seg_axes) {
        if want.semantic != got.semantic {
            return Err(format!(
                "{shm_name}: axis '{}' is {:?} in the segment but {:?} in the rig-config",
                want.name, got.semantic, want.semantic
            ));
        }
    }
    Ok(client)
}

/// Open, or reopen, the segments of devices that are not delivering.
///
/// Runs every `interval` on its own thread. A device is retried when it has no
/// segment or its producer has gone stale — a restarted producer recreates its
/// segment under the same name, and the old mapping would stay silent forever.
/// Opening happens outside the scene lock; only the swap takes it.
pub fn spawn_reconnector(scene: Arc<RwLock<SceneState>>, interval: Duration) {
    let spawned = std::thread::Builder::new().name("input-reconnect".into()).spawn(move || {
        let mut reported: std::collections::HashSet<String> = Default::default();
        loop {
            if crate::process::shutdown::is_requested() {
                return;
            }
            let wanted: Vec<(String, String, Vec<Axis>)> = {
                let sc = scene.read().expect("scene lock poisoned");
                sc.runtime
                    .input
                    .devices
                    .iter()
                    .filter_map(|d| match &d.backend {
                        Backend::Shm { shm_name, client } if client.is_none() || d.stale => {
                            Some((d.name.clone(), shm_name.clone(), d.axes.clone()))
                        }
                        _ => None,
                    })
                    .collect()
            };
            for (name, shm_name, axes) in wanted {
                match open_checked(&shm_name, &axes) {
                    Ok(client) => {
                        // A stale producer's old segment reopens as the same
                        // silent segment; only swap in one that is writing.
                        let live = client.age_ns() < 1_000_000_000;
                        let mut sc = scene.write().expect("scene lock poisoned");
                        if let Some(d) = sc.runtime.input.get_mut(&name)
                            && let Backend::Shm { client: slot, .. } = &mut d.backend
                            && (slot.is_none() || live)
                        {
                            if slot.is_none() {
                                log::info!("input: device '{name}' connected to {shm_name}");
                            }
                            *slot = Some(client);
                            d.has_baseline = false;
                            d.last_write_count = 0;
                        }
                        reported.remove(&name);
                    }
                    Err(e) => {
                        if reported.insert(name.clone()) {
                            log::warn!("input: device '{name}' not available: {e}");
                        }
                    }
                }
            }
            std::thread::sleep(interval);
        }
    });
    if let Err(e) = spawned {
        log::error!("input: could not start the reconnect thread: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vinput::{AxisDesc, VinputOwner};

    fn shm(tag: &str) -> String {
        format!("/vstimd_input_test_{}_{tag}", std::process::id())
    }

    fn device(tag: &str, semantic: Semantic, scale: f32) -> (VinputOwner, InputDevice) {
        let name = shm(tag);
        let owner = VinputOwner::create(&name, &[AxisDesc::new("a", semantic, 1.0)]).unwrap();
        let axes = vec![Axis { name: "a".into(), semantic, scale, deadzone: 0.0 }];
        let client = open_checked(&name, &axes).unwrap();
        let dev = InputDevice::new(
            "dev",
            axes,
            Duration::from_millis(50),
            Backend::Shm { shm_name: name, client: Some(client) },
        );
        (owner, dev)
    }

    #[test]
    fn cumulative_axes_yield_scaled_deltas() {
        let (mut owner, mut dev) = device("cum", Semantic::Cumulative, 0.5);
        owner.write(&[100.0]);
        dev.sample(1.0 / 60.0);
        assert_eq!(dev.frame[0].delta, 0.0, "the first reading is only a baseline");
        owner.write(&[110.0]);
        dev.sample(1.0 / 60.0);
        assert_eq!(dev.frame[0].delta, 5.0);
        assert_eq!(dev.frame[0].value, 55.0);
        dev.sample(1.0 / 60.0);
        assert_eq!(dev.frame[0].delta, 0.0);
    }

    #[test]
    fn a_stale_rate_axis_reads_zero_and_recovers() {
        let (mut owner, mut dev) = device("rate", Semantic::Rate, 2.0);
        owner.write(&[3.0]);
        dev.sample(1.0 / 60.0);
        assert_eq!(dev.frame[0].value, 6.0);
        assert!(!dev.stale);
        std::thread::sleep(Duration::from_millis(70));
        dev.sample(1.0 / 60.0);
        assert!(dev.stale);
        assert_eq!(dev.frame[0].value, 0.0, "a silent producer must stop motion");
        owner.write(&[3.0]);
        dev.sample(1.0 / 60.0);
        assert!(!dev.stale);
        assert_eq!(dev.frame[0].value, 6.0);
    }

    #[test]
    fn a_cumulative_axis_does_not_jump_when_its_producer_restarts() {
        let (mut owner, mut dev) = device("restart", Semantic::Cumulative, 1.0);
        owner.write(&[1000.0]);
        dev.sample(0.016);
        owner.write(&[1010.0]);
        dev.sample(0.016);
        assert_eq!(dev.frame[0].delta, 10.0);
        // Producer crashes; the count restarts from zero after a gap.
        std::thread::sleep(Duration::from_millis(70));
        dev.sample(0.016);
        assert!(dev.stale);
        assert_eq!(dev.frame[0].delta, 0.0);
        owner.write(&[5.0]);
        dev.sample(0.016);
        assert_eq!(dev.frame[0].delta, 0.0, "the new count is adopted, not differenced");
        owner.write(&[7.0]);
        dev.sample(0.016);
        assert_eq!(dev.frame[0].delta, 2.0);
    }

    #[test]
    fn semantics_must_match_the_segment() {
        let name = shm("mismatch");
        let _owner = VinputOwner::create(&name, &[AxisDesc::new("a", Semantic::Rate, 1.0)]).unwrap();
        let axes = vec![Axis { name: "a".into(), semantic: Semantic::Cumulative, scale: 1.0, deadzone: 0.0 }];
        assert!(open_checked(&name, &axes).is_err());
    }

    #[test]
    fn keyboard_drives_rate_and_cumulative_axes() {
        let axes = vec![
            Axis { name: "fwd".into(), semantic: Semantic::Cumulative, scale: 2.0, deadzone: 0.0 },
            Axis { name: "turn".into(), semantic: Semantic::Rate, scale: 1.0, deadzone: 0.0 },
        ];
        let mut dev = InputDevice::new("kb", axes, Duration::from_millis(50), Backend::Keyboard { speed: 30.0 });
        keyboard_axes::set_arrow(keyboard_axes::Arrow::Up, true);
        keyboard_axes::set_arrow(keyboard_axes::Arrow::Left, true);
        dev.sample(0.1);
        dev.sample(0.1);
        keyboard_axes::set_arrow(keyboard_axes::Arrow::Up, false);
        keyboard_axes::set_arrow(keyboard_axes::Arrow::Left, false);
        assert!(!dev.stale);
        assert!((dev.frame[0].delta - 3.0).abs() < 1e-9, "30 units/s for 0.1 s: {}", dev.frame[0].delta);
        assert_eq!(dev.frame[1].value, -30.0);
    }
}
