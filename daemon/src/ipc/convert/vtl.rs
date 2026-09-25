//! VTL <-> proto conversions: line handles, kinds, and the name-registry lookup
//! every handle-taking command goes through.
//!
//! Resolving a `VirtualTriggerLineHandle` is not a plain value conversion — a `name`
//! handle only resolves against the registry the server holds — but it is still the
//! wire meeting the scene, so it belongs here with every other conversion rather
//! than inside the command module that happens to use it most.

use super::super::response::err;
use crate::proto;
use crate::scene::VtlBit;
use crate::vtl_state::VtlNameEntry;

pub(crate) fn vtl_bit_to_proto(bit: VtlBit) -> proto::VirtualTriggerLineHandle {
    use proto::virtual_trigger_line_handle::Handle;
    proto::VirtualTriggerLineHandle {
        handle: Some(Handle::BankBit(proto::VirtualTriggerLineBankBit {
            bank: bit.bank as u32,
            bit: bit.bit as u32,
        })),
        kind: vtl_kind_to_proto(bit.kind) as i32,
    }
}

pub(crate) fn vtl_kind_to_proto(d: vtl::VtlKind) -> proto::VirtualTriggerLineKind {
    match d {
        vtl::VtlKind::Input => proto::VirtualTriggerLineKind::Input,
        vtl::VtlKind::Output => proto::VirtualTriggerLineKind::Output,
    }
}

/// A proto line handle -> the kind-carrying [`VtlBit`] the scene addresses lines by.
///
/// The caller is always explicit about kind: the `kind` field selects
/// the bank for a `bank_bit` handle, and for a `name` handle it selects which
/// registered entry to match (a name may be registered independently for input
/// and output). The kind is never inferred from the registry.
pub(crate) fn vtl_bit_from_proto(
    handle: Option<&proto::VirtualTriggerLineHandle>,
    names: &[VtlNameEntry],
) -> Result<VtlBit, Box<proto::Response>> {
    use proto::virtual_trigger_line_handle::Handle;
    let Some(h) = handle else {
        return Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "handle must be set",
        )));
    };
    match h.handle.as_ref() {
        Some(Handle::BankBit(bb)) => {
            if bb.bank >= vtl::MAX_BANKS as u32 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "bank out of range",
                )));
            }
            if bb.bit >= 64 {
                return Err(Box::new(err(
                    proto::ErrorCode::InvalidArgument,
                    "bit must be 0..63",
                )));
            }
            Ok(VtlBit {
                bank: bb.bank as usize,
                bit: bb.bit as u8,
                kind: vtl_kind_from_proto(h.kind)?,
            })
        }
        Some(Handle::Name(name)) => {
            let kind = vtl_kind_from_proto(h.kind)?;
            names
                .iter()
                .find(|e| e.name == *name && e.kind == kind)
                .map(|e| VtlBit {
                    bank: e.bank as usize,
                    bit: e.bit,
                    kind,
                })
                .ok_or_else(|| {
                    Box::new(err(
                        proto::ErrorCode::InvalidArgument,
                        format!("no {kind:?} virtual trigger line named {name:?}"),
                    ))
                })
        }
        None => Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "handle must be set",
        ))),
    }
}

/// Resolve a handle that must address an *output* line (action trigger lines
/// pulse an output bit). Rejects an input-directed handle.
pub(crate) fn output_vtl_bit_from_proto(
    handle: Option<&proto::VirtualTriggerLineHandle>,
    names: &[VtlNameEntry],
) -> Result<VtlBit, Box<proto::Response>> {
    let bit = vtl_bit_from_proto(handle, names)?;
    if bit.kind != vtl::VtlKind::Output {
        return Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "trigger line must address an output line (kind=OUTPUT)",
        )));
    }
    Ok(bit)
}

/// A proto line kind -> the internal `vtl::VtlKind`, rejecting UNSPECIFIED —
/// the caller must state input vs. output explicitly.
pub(crate) fn vtl_kind_from_proto(d: i32) -> Result<vtl::VtlKind, Box<proto::Response>> {
    match proto::VirtualTriggerLineKind::try_from(d) {
        Ok(proto::VirtualTriggerLineKind::Input) => Ok(vtl::VtlKind::Input),
        Ok(proto::VirtualTriggerLineKind::Output) => Ok(vtl::VtlKind::Output),
        _ => Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "virtual trigger line kind must be INPUT or OUTPUT",
        ))),
    }
}
