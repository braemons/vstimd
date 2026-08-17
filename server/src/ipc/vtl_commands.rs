//! Virtual-trigger-line commands, plus the proto <-> VtlBit handle
//! resolution every animation command also relies on.

use super::response::{err, ok_ack, ok_body};
use crate::proto;
use crate::scene::SceneState;
use crate::scene::VtlBit;
use crate::vtl_state::{VtlNameEntry, VtlState};

impl SceneState {
    // ── Virtual Trigger Line commands ─────────────────────────────────────────

    pub(super) fn cmd_set_virtual_trigger_line_name(
        &mut self,
        cmd: proto::SetVirtualTriggerLineNameRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        use vtl::{VtlKind, MAX_BANKS};

        if cmd.bank >= MAX_BANKS as u32 {
            return err(proto::ErrorCode::InvalidArgument, "bank out of range");
        }
        if cmd.bit >= 64 {
            return err(proto::ErrorCode::InvalidArgument, "bit must be 0..63");
        }
        let dir = match proto::VirtualTriggerLineKind::try_from(cmd.kind) {
            Ok(proto::VirtualTriggerLineKind::Input) => VtlKind::Input,
            Ok(proto::VirtualTriggerLineKind::Output) => VtlKind::Output,
            _ => {
                return err(
                    proto::ErrorCode::InvalidArgument,
                    "kind must be INPUT or OUTPUT",
                );
            }
        };
        let Some(vtl) = vtl else {
            return err(
                proto::ErrorCode::NotSupported,
                "VTL shared memory not available",
            );
        };

        if !cmd.name.is_empty()
            && vtl
                .names
                .iter()
                .any(|e| e.name == cmd.name && (e.bank != cmd.bank as u8 || e.bit != cmd.bit as u8))
        {
            return err(
                proto::ErrorCode::InvalidArgument,
                "name already assigned to a different line",
            );
        }

        vtl.names
            .retain(|e| !(e.bank == cmd.bank as u8 && e.bit == cmd.bit as u8));
        if !cmd.name.is_empty() {
            vtl.names.push(VtlNameEntry {
                name: cmd.name,
                bank: cmd.bank as u8,
                bit: cmd.bit as u8,
                kind: dir,
            });
        }

        vtl.sync_names_to_shm();
        ok_ack()
    }

    pub(super) fn cmd_list_virtual_trigger_lines(&self, vtl: Option<&VtlState>) -> proto::Response {
        let Some(vtl) = vtl else {
            return ok_body(proto::response::Body::VirtualTriggerLineList(
                proto::ListVirtualTriggerLinesResponse { lines: vec![] },
            ));
        };
        let owner = vtl.owner();

        // Enumerate every physical bit in the configured banks (not just named
        // lines) so the UIs can trigger/observe any line for debugging. A name is
        // attached when one is registered for that (bank, bit, kind).
        const BITS_PER_BANK: u8 = 64;
        let name_of = |bank: u8, bit: u8, dir: vtl::VtlKind| -> String {
            vtl.names
                .iter()
                .find(|e| e.bank == bank && e.bit == bit && e.kind == dir)
                .map(|e| e.name.clone())
                .unwrap_or_default()
        };

        let mut lines: Vec<proto::VirtualTriggerLineInfo> = Vec::new();
        for (dir, proto_dir, n_banks) in [
            (vtl::VtlKind::Input, proto::VirtualTriggerLineKind::Input, owner.num_input_banks()),
            (vtl::VtlKind::Output, proto::VirtualTriggerLineKind::Output, owner.num_output_banks()),
        ] {
            for bank in 0..n_banks as usize {
                let word = match dir {
                    vtl::VtlKind::Input => owner.input_state(bank),
                    vtl::VtlKind::Output => owner.output_state(bank),
                };
                for bit in 0..BITS_PER_BANK {
                    lines.push(proto::VirtualTriggerLineInfo {
                        name: name_of(bank as u8, bit, dir),
                        bank: bank as u32,
                        bit: bit as u32,
                        kind: proto_dir as i32,
                        high: word >> bit & 1 == 1,
                    });
                }
            }
        }
        ok_body(proto::response::Body::VirtualTriggerLineList(
            proto::ListVirtualTriggerLinesResponse { lines },
        ))
    }

    pub(super) fn cmd_set_virtual_trigger_line(
        &self,
        cmd: proto::SetVirtualTriggerLineRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        let Some(vtl) = vtl else {
            return err(
                proto::ErrorCode::NotSupported,
                "VTL shared memory not available",
            );
        };
        let bit = match resolve_vtl_handle(cmd.handle.as_ref(), &vtl.names) {
            Ok(v) => v,
            Err(e) => return *e,
        };
        match bit.kind {
            vtl::VtlKind::Input => {
                let owner = vtl.owner();
                if cmd.value {
                    if owner.set_input_bit(bit.bank, bit.bit) {
                        owner.set_input_rise(bit.bank, 1u64 << bit.bit);
                    }
                } else if owner.clear_input_bit(bit.bank, bit.bit) {
                    owner.set_input_fall(bit.bank, 1u64 << bit.bit);
                }
            }
            vtl::VtlKind::Output => vtl.set_staged_bit(bit.bank, bit.bit, cmd.value),
        }
        ok_ack()
    }

    pub(super) fn cmd_toggle_virtual_trigger_line(
        &self,
        cmd: proto::ToggleVirtualTriggerLineRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        let Some(vtl) = vtl else {
            return err(
                proto::ErrorCode::NotSupported,
                "VTL shared memory not available",
            );
        };
        let bit = match resolve_vtl_handle(cmd.handle.as_ref(), &vtl.names) {
            Ok(v) => v,
            Err(e) => return *e,
        };
        let high = match bit.kind {
            vtl::VtlKind::Input => {
                let owner = vtl.owner();
                let mask = 1u64 << bit.bit;
                let rose = owner.toggle_input_bit(bit.bank, bit.bit);
                if rose {
                    owner.set_input_rise(bit.bank, mask);
                } else {
                    owner.set_input_fall(bit.bank, mask);
                }
                rose
            }
            vtl::VtlKind::Output => {
                let high = (vtl.staged[bit.bank] >> bit.bit) & 1 == 0; // high after toggle
                vtl.set_staged_bit(bit.bank, bit.bit, high);
                high
            }
        };
        ok_body(proto::response::Body::VirtualTriggerLineState(
            proto::VirtualTriggerLineStateResponse { high },
        ))
    }

    pub(super) fn cmd_clear_virtual_trigger_line_latches(
        &self,
        cmd: proto::ClearVirtualTriggerLineLatchesRequest,
        vtl: Option<&VtlState>,
    ) -> proto::Response {
        let Some(vtl) = vtl else {
            return err(
                proto::ErrorCode::NotSupported,
                "VTL shared memory not available",
            );
        };
        let bit = match resolve_vtl_handle(cmd.handle.as_ref(), &vtl.names) {
            Ok(v) => v,
            Err(e) => return *e,
        };
        if bit.kind != vtl::VtlKind::Input {
            return err(
                proto::ErrorCode::InvalidArgument,
                "only input lines have rise/fall latches to clear",
            );
        }
        let owner = vtl.owner();
        let mask = 1u64 << bit.bit;
        owner.drain_input_rise(bit.bank, mask);
        owner.drain_input_fall(bit.bank, mask);
        ok_ack()
    }

    pub(super) fn cmd_set_virtual_trigger_line_bank(
        &self,
        cmd: proto::SetVirtualTriggerLineBankRequest,
        vtl: Option<&mut VtlState>,
    ) -> proto::Response {
        let Some(vtl) = vtl else {
            return err(
                proto::ErrorCode::NotSupported,
                "VTL shared memory not available",
            );
        };
        if cmd.bank >= vtl::MAX_BANKS as u32 {
            return err(proto::ErrorCode::InvalidArgument, "bank out of range");
        }
        let bank = cmd.bank as usize;
        let kind = match proto_kind(cmd.kind) {
            Ok(k) => k,
            Err(e) => return *e,
        };
        match kind {
            vtl::VtlKind::Input => {
                let owner = vtl.owner();
                let prev = owner.input_state(bank);
                let next = cmd.value;
                let rising = (!prev) & next;
                let falling = prev & (!next);
                owner.set_input_state(bank, next);
                if rising != 0 {
                    owner.set_input_rise(bank, rising);
                }
                if falling != 0 {
                    owner.set_input_fall(bank, falling);
                }
            }
            vtl::VtlKind::Output => vtl.set_staged_bank(bank, cmd.value),
        }
        ok_ack()
    }
}

pub(super) fn vtl_bit_to_proto(bit: VtlBit) -> proto::VirtualTriggerLineHandle {
    use proto::virtual_trigger_line_handle::Handle;
    proto::VirtualTriggerLineHandle {
        handle: Some(Handle::BankBit(proto::VirtualTriggerLineBankBit {
            bank: bit.bank as u32,
            bit: bit.bit as u32,
        })),
        kind: kind_to_proto(bit.kind) as i32,
    }
}

pub(super) fn kind_to_proto(d: vtl::VtlKind) -> proto::VirtualTriggerLineKind {
    match d {
        vtl::VtlKind::Input => proto::VirtualTriggerLineKind::Input,
        vtl::VtlKind::Output => proto::VirtualTriggerLineKind::Output,
    }
}

// ── Module-private helpers ────────────────────────────────────────────────────

/// Resolve a proto handle to a kind-carrying [`VtlBit`].
///
/// The caller is always explicit about kind: the `kind` field selects
/// the bank for a `bank_bit` handle, and for a `name` handle it selects which
/// registered entry to match (a name may be registered independently for input
/// and output). The kind is never inferred from the registry.
pub(super) fn resolve_vtl_handle(
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
                kind: proto_kind(h.kind)?,
            })
        }
        Some(Handle::Name(name)) => {
            let kind = proto_kind(h.kind)?;
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
pub(super) fn resolve_output_handle(
    handle: Option<&proto::VirtualTriggerLineHandle>,
    names: &[VtlNameEntry],
) -> Result<VtlBit, Box<proto::Response>> {
    let bit = resolve_vtl_handle(handle, names)?;
    if bit.kind != vtl::VtlKind::Output {
        return Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "trigger line must address an output line (kind=OUTPUT)",
        )));
    }
    Ok(bit)
}

/// Convert a proto kind to the internal `vtl::VtlKind`, rejecting UNSPECIFIED —
/// the caller must state input vs. output explicitly.
pub(super) fn proto_kind(d: i32) -> Result<vtl::VtlKind, Box<proto::Response>> {
    match proto::VirtualTriggerLineKind::try_from(d) {
        Ok(proto::VirtualTriggerLineKind::Input) => Ok(vtl::VtlKind::Input),
        Ok(proto::VirtualTriggerLineKind::Output) => Ok(vtl::VtlKind::Output),
        _ => Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "virtual trigger line kind must be INPUT or OUTPUT",
        ))),
    }
}
