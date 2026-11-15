// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-keep-position: in a real X11 window, a dialog window (plain and modal) that was
//! moved aside stays there when the main window moves, and a reopened one is centred over the
//! main window's new place. Runs under Xvfb with DRYDOCK_GUI_TESTS=1 and skips itself otherwise.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use drydock::dialog::{DialogWindow, dialog_window, modal_dialog_window};
use eframe::egui;

struct Probe {
    title: &'static str,
    modal: bool,
    open: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.label("root");
        let mut open = self.open.load(Ordering::SeqCst);
        let window = DialogWindow {
            title: self.title,
            size: egui::vec2(240.0, 140.0),
        };
        let content = |ui: &mut egui::Ui| {
            ui.label("dialog body");
        };
        if self.modal {
            modal_dialog_window(
                &ctx,
                egui::Id::new("keep-probe"),
                &window,
                &mut open,
                content,
            );
        } else {
            dialog_window(&ctx, &window, &mut open, content);
        }
        self.open.store(open, Ordering::SeqCst);
        if self.quit.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn run_probe(probe: Probe, root_title: &'static str) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(root_title)
            .with_position([100.0, 100.0])
            .with_inner_size([480.0, 320.0]),
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11().with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        root_title,
        options,
        Box::new(move |_cc| Ok(Box::new(probe))),
    )
    .map_err(|err| err.to_string())
}

fn find_window(title: &str) -> Option<u32> {
    let out = Command::new("xdotool")
        .args(["search", "--onlyvisible", "--name", &format!("^{title}$")])
        .output()
        .expect("xdotool runs");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
}

/// Starts a probe and waits until its root window is visible. The display resets when its last
/// client leaves, so an app that connects at that moment can exit before opening a window;
/// such a probe is started again.
fn launch(
    root_title: &'static str,
    dialog_title: &'static str,
    modal: bool,
    open: &Arc<AtomicBool>,
    quit: &Arc<AtomicBool>,
) -> (thread::JoinHandle<Result<(), String>>, u32) {
    let spawn = || {
        let probe = Probe {
            title: dialog_title,
            modal,
            open: Arc::clone(open),
            quit: Arc::clone(quit),
        };
        thread::spawn(move || run_probe(probe, root_title))
    };
    let mut app = spawn();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(id) = find_window(root_title) {
            return (app, id);
        }
        if Instant::now() >= deadline {
            panic!("window {root_title:?} never became visible");
        }
        if app.is_finished() {
            let _ = app.join();
            app = spawn();
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn wait_for(title: &str, present: bool) -> Option<u32> {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        let found = find_window(title);
        if found.is_some() == present {
            return found;
        }
        thread::sleep(Duration::from_millis(200));
    }
    panic!(
        "window {title:?} never became {}",
        if present { "visible" } else { "gone" }
    );
}

/// The window's absolute upper-left corner, from `xwininfo`.
fn position(id: u32) -> (i32, i32) {
    let out = Command::new("xwininfo")
        .args(["-id", &id.to_string()])
        .output()
        .expect("xwininfo runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |name: &str| -> i32 {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(name))
            .and_then(|rest| rest.trim().parse().ok())
            .unwrap_or_else(|| panic!("no {name} in xwininfo output: {text}"))
    };
    (
        field("Absolute upper-left X:"),
        field("Absolute upper-left Y:"),
    )
}

fn move_window(id: u32, x: i32, y: i32) {
    Command::new("xdotool")
        .args([
            "windowmove",
            &id.to_string(),
            &x.to_string(),
            &y.to_string(),
        ])
        .status()
        .expect("xdotool runs");
}

fn near(got: (i32, i32), want: (i32, i32), tolerance: i32, what: &str) {
    assert!(
        (got.0 - want.0).abs() <= tolerance && (got.1 - want.1).abs() <= tolerance,
        "{what}: at {got:?}, expected {want:?} (+-{tolerance})"
    );
}

fn run_case(root_title: &'static str, dialog_title: &'static str, modal: bool) {
    if std::env::var_os("DRYDOCK_GUI_TESTS").is_none() || std::env::var_os("DISPLAY").is_none() {
        eprintln!("skipped: needs DRYDOCK_GUI_TESTS=1 and an X display");
        return;
    }
    let open = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let (app, root) = launch(root_title, dialog_title, modal, &open, &quit);
    thread::sleep(Duration::from_secs(1));
    move_window(root, 100, 100);
    thread::sleep(Duration::from_millis(500));

    // Opens centred over the main window.
    open.store(true, Ordering::SeqCst);
    let dialog = wait_for(dialog_title, true).expect("dialog window");
    thread::sleep(Duration::from_secs(2));
    let root_at = position(root);
    let first = position(dialog);
    let offset = (first.0 - root_at.0, first.1 - root_at.1);
    assert!(
        (offset.0 - 120).abs() <= 16,
        "dialog should be centred horizontally over the main window: offset {offset:?}"
    );
    assert!(
        offset.1 > 20 && offset.1 < 140,
        "dialog should be centred vertically over the main window: offset {offset:?}"
    );

    // Moved aside, it stays put while the main window moves.
    move_window(dialog, 40, 60);
    thread::sleep(Duration::from_secs(1));
    near(
        position(dialog),
        (40, 60),
        4,
        "dialog after being moved aside",
    );
    move_window(root, 300, 200);
    thread::sleep(Duration::from_secs(3));
    near(
        position(root),
        (300, 200),
        4,
        "main window after being moved",
    );
    near(
        position(dialog),
        (40, 60),
        4,
        "dialog after the main window moved",
    );

    // Closed and reopened, it is centred over the main window's new place.
    open.store(false, Ordering::SeqCst);
    wait_for(dialog_title, false);
    thread::sleep(Duration::from_millis(500));
    open.store(true, Ordering::SeqCst);
    let dialog = wait_for(dialog_title, true).expect("reopened dialog window");
    thread::sleep(Duration::from_secs(2));
    let root_at = position(root);
    let again = position(dialog);

    quit.store(true, Ordering::SeqCst);
    let end = Instant::now() + Duration::from_secs(20);
    while !app.is_finished() && Instant::now() < end {
        thread::sleep(Duration::from_millis(100));
    }

    near(
        again,
        (root_at.0 + offset.0, root_at.1 + offset.1),
        16,
        "reopened dialog centred over the moved main window",
    );
}

#[test]
fn plain_dialog_stays_where_it_was_put() {
    run_case("keep-pos-root-plain", "keep-pos-dialog-plain", false);
}

#[test]
fn modal_dialog_stays_where_it_was_put() {
    run_case("keep-pos-root-modal", "keep-pos-dialog-modal", true);
}
