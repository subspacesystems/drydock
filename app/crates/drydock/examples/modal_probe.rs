// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Real-window probe for `modal_dialog_window` (`docs/specs/m11-modal-window.md`). The main
//! window is a text box that asks for focus every frame above one big button; its title
//! reports what reached it: `probe clicks=N typed=T answer=A`. `--modal` opens a "Probe Name" prompt at startup
//! (a Name box, OK and Cancel); `answer` becomes `ok:<name>` or `cancel`. Not part of the
//! app: `tests/modal_window_x11.rs` starts it under Xvfb.

use drydock::dialog::{DialogButton, DialogWindow, button_row, modal_dialog_window};
use eframe::egui;

struct Probe {
    clicks: u32,
    typed: String,
    answer: String,
    open: bool,
    name: String,
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.text_edit_singleline(&mut self.typed).request_focus();
            let size = ui.available_size();
            if ui.add_sized(size, egui::Button::new("Count")).clicked() {
                self.clicks += 1;
            }
        });
        let window = DialogWindow {
            title: "Probe Name",
            size: egui::vec2(320.0, 120.0),
        };
        let mut open = self.open;
        let mut answer = None;
        modal_dialog_window(
            &ctx,
            egui::Id::new("probe-name"),
            &window,
            &mut open,
            |ui| {
                ui.text_edit_singleline(&mut self.name).request_focus();
                match button_row(
                    ui,
                    &[
                        DialogButton {
                            label: "OK",
                            default: true,
                            enabled: true,
                        },
                        DialogButton {
                            label: "Cancel",
                            default: false,
                            enabled: true,
                        },
                    ],
                ) {
                    Some(0) => answer = Some(format!("ok:{}", self.name)),
                    Some(1) => answer = Some("cancel".to_owned()),
                    _ => {}
                }
            },
        );
        if answer.is_none() && self.open && !open {
            answer = Some("cancel".to_owned());
        }
        if let Some(answer) = answer {
            self.answer = answer;
            open = false;
        }
        self.open = open;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "probe clicks={} typed={} answer={}",
            self.clicks,
            self.typed,
            if self.answer.is_empty() {
                "none"
            } else {
                &self.answer
            }
        )));
    }
}

fn main() -> eframe::Result {
    let open = std::env::args().any(|arg| arg == "--modal");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("probe")
            .with_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "modal-probe",
        options,
        Box::new(move |_| {
            Ok(Box::new(Probe {
                clicks: 0,
                typed: String::new(),
                answer: String::new(),
                open,
                name: String::new(),
            }))
        }),
    )
}
