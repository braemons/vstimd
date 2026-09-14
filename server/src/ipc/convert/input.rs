//! Input device <-> proto.

use crate::input::InputDevice;
use crate::proto;

pub(crate) fn input_semantic_to_proto(s: vinput::Semantic) -> proto::InputSemantic {
    match s {
        vinput::Semantic::Absolute => proto::InputSemantic::Absolute,
        vinput::Semantic::Cumulative => proto::InputSemantic::Cumulative,
        vinput::Semantic::Rate => proto::InputSemantic::Rate,
    }
}

pub(crate) fn input_device_to_proto(d: &InputDevice) -> proto::InputDeviceInfo {
    proto::InputDeviceInfo {
        name: d.name.clone(),
        backend: d.backend.label(),
        connected: d.is_connected(),
        stale: d.stale,
        torn_reads: d.torn_reads,
        axes: d
            .axes
            .iter()
            .zip(&d.frame)
            .map(|(a, f)| proto::InputAxisInfo {
                name: a.name.clone(),
                semantic: input_semantic_to_proto(a.semantic) as i32,
                scale: a.scale,
                value: f.value,
                delta: f.delta,
            })
            .collect(),
    }
}
