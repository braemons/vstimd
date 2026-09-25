//! VTL group — bank values, named input/output lines, and manual line firing.

use std::sync::Mutex;

use crate::vtl_state::VtlState;

#[derive(Clone, Copy, PartialEq, Default)]
enum BankFmt { Dec, Hex, #[default] Bin }

/// Persisted state for the manual VTL fire control (set any bank/bit for debug).
#[derive(Clone, Copy, Default)]
struct VtlManual {
    bank: u32,
    bit: u32,
    output: bool,
}

pub(in crate::render::overlay_ui) fn vtl_panel(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    want_focus: bool,
    vtl: Option<&Mutex<VtlState>>,
) {
    let Some(mut vtl_guard) = vtl.and_then(|v| v.try_lock().ok()) else {
        ui.label(egui::RichText::new("VTL not available").color(egui::Color32::DARK_GRAY));
        return;
    };
    let vtl_st = &mut *vtl_guard;
    // Owned copies so the read state and names don't hold a borrow of `vtl_st`
    // across the output writes (`set_staged_bit` needs `&mut`).
    let names = vtl_st.names.clone();
    let inputs:  Vec<_> = names.iter().filter(|e| e.kind == vtl::VtlKind::Input).collect();
    let outputs: Vec<_> = names.iter().filter(|e| e.kind == vtl::VtlKind::Output).collect();

    // --- Bank view (integer representation) ---
    let fmt_id = egui::Id::new("vtl_bank_fmt");
    let mut fmt: BankFmt = ctx.data(|d| d.get_temp(fmt_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Banks").strong());
        ui.separator();
        // Anchor keyboard focus here when the panel is F-keyed: this row is always
        // present, so Tab navigation starts inside the VTL panel even when there
        // are no named input lines (whose fire buttons used to be the only anchor).
        let dec = ui.selectable_value(&mut fmt, BankFmt::Dec, "Dec");
        if want_focus {
            dec.request_focus();
        }
        ui.selectable_value(&mut fmt, BankFmt::Hex, "Hex");
        ui.selectable_value(&mut fmt, BankFmt::Bin, "Bin");
    });
    ctx.data_mut(|d| d.insert_temp(fmt_id, fmt));

    let fmt_val = |val: u64| -> String {
        match fmt {
            BankFmt::Dec => format!("{}", val),
            BankFmt::Hex => format!("0x{:016X}", val),
            BankFmt::Bin => {
                let s = format!("{:064b}", val);
                s.as_bytes().chunks(8).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(" ")
            }
        }
    };

    let n_in  = vtl_st.owner().num_input_banks()  as usize;
    let n_out = vtl_st.owner().num_output_banks() as usize;
    egui::Grid::new("vtl_bank_grid").num_columns(3).spacing([8.0, 2.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Dir").strong());
        ui.label(egui::RichText::new("Bank").strong());
        ui.label(egui::RichText::new("Value").strong());
        ui.end_row();
        for b in 0..n_in {
            ui.label("In");
            ui.label(format!("{}", b));
            ui.label(egui::RichText::new(fmt_val(vtl_st.owner().input_state(b))).monospace());
            ui.end_row();
        }
        for b in 0..n_out {
            ui.label("Out");
            ui.label(format!("{}", b));
            ui.label(egui::RichText::new(fmt_val(vtl_st.owner().output_state(b))).monospace());
            ui.end_row();
        }
    });
    ui.separator();

    let dot = |ui: &mut egui::Ui, high: bool| {
        let color = if high { egui::Color32::from_rgb(80, 200, 80) } else { egui::Color32::DARK_GRAY };
        let (resp, painter) = ui.allocate_painter(egui::vec2(12.0, 12.0), egui::Sense::hover());
        painter.circle_filled(resp.rect.center(), 5.0, color);
    };

    if vtl_st.names.is_empty() {
        ui.label(egui::RichText::new("(no named lines)").color(egui::Color32::DARK_GRAY));
    }

    if !inputs.is_empty() {
        ui.label(egui::RichText::new("Inputs — Tab to a line, Enter/Space to fire").strong());
        egui::Grid::new("vtl_input_grid").striped(true).num_columns(5).spacing([8.0, 2.0]).show(ui, |ui| {
            ui.label(egui::RichText::new("Name").strong());
            ui.label(egui::RichText::new("Bank/Bit").strong());
            ui.label(egui::RichText::new("Level").strong());
            ui.label(egui::RichText::new("Rise/Fall").strong());
            ui.label(egui::RichText::new("Fire").strong());
            ui.end_row();
            for (i, e) in inputs.iter().enumerate() {
                let b = e.bank as usize;
                let mask = 1u64 << e.bit;
                let high  = vtl_st.owner().input_state(b) & mask != 0;
                let rise  = vtl_st.owner().peek_input_rise(b) & mask != 0;
                let fall  = vtl_st.owner().peek_input_fall(b) & mask != 0;
                ui.label(&e.name);
                ui.label(format!("{}/{}", e.bank, e.bit));
                dot(ui, high);
                ui.label(format!("{}/{}", rise as u8, fall as u8));
                ui.horizontal(|ui| {
                    let up = ui.button("+").on_hover_text("Fire rising edge");
                    if want_focus && i == 0 {
                        up.request_focus();
                    }
                    if up.clicked() {
                        vtl_st.owner().set_input_bit(b, e.bit);
                        vtl_st.owner().set_input_rise(b, mask);
                    }
                    if ui.button("-").on_hover_text("Fire falling edge").clicked() {
                        vtl_st.owner().clear_input_bit(b, e.bit);
                        vtl_st.owner().set_input_fall(b, mask);
                    }
                });
                ui.end_row();
            }
        });
        ui.add_space(4.0);
    }

    if !outputs.is_empty() {
        ui.label(egui::RichText::new("Outputs").strong());
        egui::Grid::new("vtl_output_grid").striped(true).num_columns(3).spacing([8.0, 2.0]).show(ui, |ui| {
            ui.label(egui::RichText::new("Name").strong());
            ui.label(egui::RichText::new("Bank/Bit").strong());
            ui.label(egui::RichText::new("Level").strong());
            ui.end_row();
            for e in &outputs {
                let b = e.bank as usize;
                let mask = 1u64 << e.bit;
                let high = vtl_st.owner().output_state(b) & mask != 0;
                ui.label(&e.name);
                ui.label(format!("{}/{}", e.bank, e.bit));
                dot(ui, high);
                ui.end_row();
            }
        });
    }

    // --- Manual fire: set any bank/bit, named or not (debug) ---
    ui.separator();
    ui.label(egui::RichText::new("Manual fire (any line)").strong());
    let manual_id = egui::Id::new("vtl_manual");
    let mut m: VtlManual = ctx.data(|d| d.get_temp(manual_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.selectable_value(&mut m.output, false, "In");
        ui.selectable_value(&mut m.output, true, "Out");
        ui.label("Bank");
        let max_bank = (if m.output { n_out } else { n_in }).saturating_sub(1) as u32;
        ui.add(egui::DragValue::new(&mut m.bank).range(0..=max_bank));
        ui.label("Bit");
        ui.add(egui::DragValue::new(&mut m.bit).range(0..=63));
    });
    ui.horizontal(|ui| {
        let bank = m.bank as usize;
        let bit = m.bit as u8;
        let mask = 1u64 << bit;
        if m.output {
            if ui.button("High").clicked() {
                vtl_st.set_staged_bit(bank, bit, true);
                log::info!("vtl: manual fire out bank={bank} bit={bit} -> high (state now {:#018x})", vtl_st.owner().output_state(bank));
            }
            if ui.button("Low").clicked() {
                vtl_st.set_staged_bit(bank, bit, false);
                log::info!("vtl: manual fire out bank={bank} bit={bit} -> low (state now {:#018x})", vtl_st.owner().output_state(bank));
            }
        } else {
            if ui.button("+ rise").clicked() {
                vtl_st.owner().set_input_bit(bank, bit);
                vtl_st.owner().set_input_rise(bank, mask);
                log::info!("vtl: manual fire in bank={bank} bit={bit} -> rise (state now {:#018x})", vtl_st.owner().input_state(bank));
            }
            if ui.button("- fall").clicked() {
                vtl_st.owner().clear_input_bit(bank, bit);
                vtl_st.owner().set_input_fall(bank, mask);
                log::info!("vtl: manual fire in bank={bank} bit={bit} -> fall (state now {:#018x})", vtl_st.owner().input_state(bank));
            }
        }
    });
    ctx.data_mut(|d| d.insert_temp(manual_id, m));
}
