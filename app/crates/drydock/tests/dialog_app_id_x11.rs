// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-app-id: on X11 a dialog window and a floating panel have the same `WM_CLASS` as
//! the main window (derived from the application id), so the desktop groups them under
//! Drydock. Runs under Xvfb with DRYDOCK_GUI_TESTS=1 and skips itself otherwise.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use drydock::dialog::{DialogWindow, dialog_window};
use drydock::dock::PanelId;
use drydock::floating::floating_panel_window;
use drydock::window_icon::with_app_identity;
use eframe::egui::{self, Rect, pos2, vec2};

const ROOT: &str = "app-id-probe-root";
const DIALOG: &str = "app-id-probe-dialog";
const PANEL: &str = "Materials";

struct Probe {
    quit: Arc<AtomicBool>,
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.label("root");
        let mut open = true;
        dialog_window(
            &ctx,
            &DialogWindow {
                title: DIALOG,
                size: vec2(200.0, 120.0),
            },
            &mut open,
            |ui| {
                ui.label("dialog");
            },
        );
        floating_panel_window(
            &ctx,
            PanelId::Materials,
            Rect::from_min_size(pos2(20.0, 20.0), vec2(240.0, 160.0)),
            |ui| {
                ui.label("panel");
            },
        );
        if self.quit.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn run_probe(quit: Arc<AtomicBool>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: with_app_identity(
            egui::ViewportBuilder::default()
                .with_title(ROOT)
                .with_inner_size([320.0, 240.0]),
        ),
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11().with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "app-id-probe",
        options,
        Box::new(move |_cc| Ok(Box::new(Probe { quit }))),
    )
    .map_err(|err| err.to_string())
}

fn find_window(title: &str) -> Option<u32> {
    let out = Command::new("xdotool")
        .args(["search", "--name", &format!("^{title}$")])
        .output()
        .expect("xdotool runs");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
}

/// `WM_CLASS(STRING) = "a", "b"` -> the text after `=`; empty when not set.
fn wm_class(id: u32) -> String {
    let out = Command::new("xprop")
        .args(["-id", &id.to_string(), "WM_CLASS"])
        .output()
        .expect("xprop runs");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    text.split_once(" = ")
        .map(|(_, class)| class.to_owned())
        .unwrap_or_default()
}

#[test]
fn dialog_and_floating_panel_have_the_main_windows_class() {
    if std::env::var_os("DRYDOCK_GUI_TESTS").is_none() || std::env::var_os("DISPLAY").is_none() {
        eprintln!("skipped: needs DRYDOCK_GUI_TESTS=1 and an X display");
        return;
    }
    let quit = Arc::new(AtomicBool::new(false));
    let app = {
        let quit = Arc::clone(&quit);
        thread::spawn(move || run_probe(quit))
    };

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut ids = [None; 3];
    while ids.iter().any(Option::is_none) {
        if app.is_finished() {
            panic!("probe app ended early: {:?}", app.join());
        }
        assert!(Instant::now() < deadline, "windows never appeared: {ids:?}");
        for (slot, title) in ids.iter_mut().zip([ROOT, DIALOG, PANEL]) {
            if slot.is_none() {
                *slot = find_window(title);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    thread::sleep(Duration::from_secs(1));
    let [root, dialog, panel] = ids.map(Option::unwrap);
    let classes = [wm_class(root), wm_class(dialog), wm_class(panel)];

    quit.store(true, Ordering::SeqCst);
    let end = Instant::now() + Duration::from_secs(20);
    while !app.is_finished() && Instant::now() < end {
        thread::sleep(Duration::from_millis(100));
    }
    if app.is_finished() {
        let result = app.join().expect("probe thread does not panic");
        assert!(result.is_ok(), "probe app failed: {result:?}");
    }

    assert!(
        classes[0].contains("Drydock"),
        "the main window's class comes from the app id: {:?}",
        classes[0]
    );
    assert_eq!(classes[1], classes[0], "dialog window class");
    assert_eq!(classes[2], classes[0], "floating panel class");
}
