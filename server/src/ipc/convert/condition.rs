//! Conditions on the wire.
//!
//! The wire spells an absent name as `""` (proto3 has no bare optional string
//! here), the scene as `None`. That one difference is the whole conversion, and
//! it lives here rather than in either type so neither has to know about the
//! other's spelling of "no name".

use crate::proto;
use crate::scene::conditions::{Condition, ConditionAction};

use super::nonempty;

pub(in crate::ipc) fn condition_from_proto(c: proto::Condition) -> Condition {
    Condition::new(c.index, nonempty(c.name))
}

pub(in crate::ipc) fn condition_to_proto(c: &Condition) -> proto::Condition {
    proto::Condition {
        index: c.index,
        name: c.name.clone().unwrap_or_default(),
    }
}

/// Decode the per-animation switch policy. `Unspecified` — the field left unset
/// by an older client, or by one that does not care — means the default.
pub(in crate::ipc) fn condition_action_from_proto(v: i32) -> ConditionAction {
    match proto::ConditionAction::try_from(v).unwrap_or(proto::ConditionAction::Unspecified) {
        proto::ConditionAction::Unspecified | proto::ConditionAction::Reset => {
            ConditionAction::Reset
        }
        proto::ConditionAction::Hold => ConditionAction::Hold,
        proto::ConditionAction::Stop => ConditionAction::Stop,
    }
}

pub(in crate::ipc) fn condition_action_to_proto(a: ConditionAction) -> proto::ConditionAction {
    match a {
        ConditionAction::Reset => proto::ConditionAction::Reset,
        ConditionAction::Hold => proto::ConditionAction::Hold,
        ConditionAction::Stop => proto::ConditionAction::Stop,
    }
}
