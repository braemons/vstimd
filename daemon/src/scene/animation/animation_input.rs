//! Animations driven by input devices: create-time validation against the rig's
//! devices, and the per-frame channel updates.

use vinput::Semantic;

pub use super::animation_kind::{AxisMap, AxisRef, CameraTarget, TransformChannel};
use super::{Animation, AnimationTarget};
use crate::input::{InputDevice, InputRegistry};

/// Refuse an animation that names a device, axis or target it cannot use —
/// at create time, with a message naming what is wrong, rather than as a
/// stimulus that silently never moves.
pub fn check_animation(
    target: &AnimationTarget,
    animation: &Animation,
    input: &InputRegistry,
) -> Result<(), String> {
    let is_camera = matches!(target, AnimationTarget::Camera);
    let name = animation.type_name();
    match (animation.camera_target(), is_camera) {
        (CameraTarget::Forbidden, true) => return Err(format!("{name} cannot target the camera")),
        (CameraTarget::Required, false) => return Err(format!("{name} must target the camera")),
        _ => {}
    }
    match animation {
        Animation::LinearNav3D {
            source: Some(source),
            ..
        } => {
            let (_, semantic) = resolve(input, source)?;
            if semantic == Semantic::Absolute {
                return Err(format!(
                    "LinearNav3D needs a rate or cumulative axis; {}.{} is absolute",
                    source.device, source.axis
                ));
            }
        }
        Animation::ExternalPosition2D { shm_name, .. } => {
            let device = find_device(input, shm_name)?;
            let absolute = device
                .axes
                .iter()
                .take(2)
                .filter(|a| a.semantic == Semantic::Absolute);
            if absolute.count() < 2 {
                return Err(format!(
                    "ExternalPosition2D needs two absolute axes first; device '{}' has not",
                    device.name
                ));
            }
        }
        Animation::DeviceDrivenTransform { device, axes } => {
            if axes.is_empty() {
                return Err("DeviceDrivenTransform needs at least one axis mapping".into());
            }
            for map in axes {
                let (_, semantic) = resolve(
                    input,
                    &AxisRef {
                        device: device.clone(),
                        axis: map.axis.clone(),
                    },
                )?;
                check_map(map, semantic, is_camera)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_map(map: &AxisMap, semantic: Semantic, is_camera: bool) -> Result<(), String> {
    let ch = map.channel;
    if ch.is_camera_local() && !is_camera {
        return Err(format!(
            "channel {ch:?} moves the camera and needs a camera target"
        ));
    }
    if ch.is_camera_local() && semantic == Semantic::Absolute {
        return Err(format!(
            "axis '{}' is absolute, but {ch:?} is a movement: use a rate or cumulative axis",
            map.axis
        ));
    }
    if ch.is_scale() && is_camera {
        return Err(format!("the camera has no scale ({ch:?})"));
    }
    if !map.gain.is_finite() {
        return Err(format!("axis '{}': gain must be finite", map.axis));
    }
    if let Some([lo, hi]) = map.clamp
        && !(lo <= hi)
    {
        return Err(format!("axis '{}': clamp needs min <= max", map.axis));
    }
    if let Some(w) = map.wrap
        && !(w > 0.0)
    {
        return Err(format!("axis '{}': wrap must be positive", map.axis));
    }
    Ok(())
}

/// A device by rig-config name, or by its segment name (`ExternalPosition2D`'s
/// legacy `shm_name`).
pub fn find_device<'a>(input: &'a InputRegistry, name: &str) -> Result<&'a InputDevice, String> {
    find_device_opt(input, name)
        .ok_or_else(|| format!("no input device named '{name}' in the rig-config"))
}

/// [`find_device`] without the message, for the render thread.
pub fn find_device_opt<'a>(input: &'a InputRegistry, name: &str) -> Option<&'a InputDevice> {
    input.devices.iter().find(|d| d.name == name).or_else(|| {
        input.devices.iter().find(|d| {
            matches!(&d.backend, crate::input::devices::Backend::Shm { shm_name, .. } if shm_name == name)
        })
    })
}

fn resolve(input: &InputRegistry, r: &AxisRef) -> Result<(usize, Semantic), String> {
    let device = find_device(input, &r.device)?;
    let idx = device
        .axis_index(&r.axis)
        .ok_or_else(|| format!("input device '{}' has no axis '{}'", r.device, r.axis))?;
    Ok((idx, device.axes[idx].semantic))
}

/// How one axis changes one channel this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Update {
    Set(f64),
    Add(f64),
}

/// This frame's update from `map`, or `None` if the device or axis is gone.
/// `dt_s` is the real frame time. Render thread: lookups and arithmetic only.
pub fn axis_update(
    input: &InputRegistry,
    device: &str,
    map: &AxisMap,
    dt_s: f64,
) -> Option<Update> {
    let dev = input.get(device)?;
    let idx = dev.axis_index(&map.axis)?;
    let f = dev.frame[idx];
    let sign = if map.invert { -1.0 } else { 1.0 };
    let gain = f64::from(map.gain) * sign;
    let dead = |v: f64| {
        if v.abs() <= f64::from(map.deadzone) {
            0.0
        } else {
            v
        }
    };
    Some(match dev.axes[idx].semantic {
        Semantic::Absolute => Update::Set(dead(f.value) * gain),
        Semantic::Cumulative => Update::Add(f.delta * gain),
        Semantic::Rate => Update::Add(dead(f.value) * gain * dt_s),
    })
}

/// The camera's movement this frame from a `LinearNav3D` source, cm.
pub fn nav_step_cm(input: &InputRegistry, source: &AxisRef, dt_s: f64) -> f64 {
    let Some(dev) = input.get(&source.device) else {
        return 0.0;
    };
    let Some(idx) = dev.axis_index(&source.axis) else {
        return 0.0;
    };
    let f = dev.frame[idx];
    match dev.axes[idx].semantic {
        Semantic::Rate => f.value * dt_s,
        Semantic::Cumulative => f.delta,
        Semantic::Absolute => 0.0,
    }
}

/// Apply an update to one channel value, then clamp and wrap it.
pub fn apply(current: f32, update: Update, map: &AxisMap) -> f32 {
    let mut v = match update {
        Update::Set(v) => v,
        Update::Add(d) => f64::from(current) + d,
    };
    if let Some([lo, hi]) = map.clamp {
        v = v.clamp(f64::from(lo), f64::from(hi));
    }
    if let Some(w) = map.wrap {
        v = v.rem_euclid(f64::from(w));
    }
    v as f32
}

/// Whether a channel applies to a 2-D stimulus.
pub fn applies_to_2d(ch: TransformChannel) -> bool {
    matches!(
        ch,
        TransformChannel::PosX | TransformChannel::PosY | TransformChannel::Yaw
    )
}
