// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Real-window probe for `modal_dialog_window` (`docs/specs/m11-modal-window.md`). The main
//! window is a text box that asks for focus every frame above one big button; its title
//! reports what reached it: `probe clicks=N typed=T answer=A`. `--modal` opens a "Probe Name" prompt at startup
//! (a Name box, OK and Cancel); `answer` becomes `ok:<name>` or `cancel`. Not part of the
//! app: `tests/modal_window_x11.rs` starts it under Xvfb.

use drydock::dialog::{
    DialogButton, DialogWindow, button_row, focus_when_window_active, modal_dialog_window,
};
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
            let field = ui.text_edit_singleline(&mut self.typed);
            if !self.open {
                focus_when_window_active(ui, &field);
            }
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
                let field = ui.text_edit_singleline(&mut self.name);
                focus_when_window_active(ui, &field);
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

/// Holds the display while a probe runs, so the real-window tests do not put two probe
/// windows on the same display at once. The app's dialog test hook uses the same name.
#[cfg(target_os = "linux")]
fn probe_lock() -> Option<std::os::unix::net::UnixListener> {
    use std::os::linux::net::SocketAddrExt;
    use std::time::{Duration, Instant};

    let address =
        std::os::unix::net::SocketAddr::from_abstract_name(b"drydock-dialog-test").ok()?;
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        match std::os::unix::net::UnixListener::bind_addr(&address) {
            Ok(listener) => return Some(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn probe_lock() {}

fn main() -> eframe::Result {
    let _lock = probe_lock();
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
        Box::new(move |cc| {
            drydock::look::apply(&cc.egui_ctx);
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
