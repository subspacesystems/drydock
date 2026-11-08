// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-winit-parent: every egui viewport other than the root names the main window as its
//! transient parent. Only X11 can be checked here: the test opens a root window, one immediate
//! and one deferred viewport on X11 and reads their properties with `xprop`. It runs under
//! Xvfb with DRYDOCK_GUI_TESTS=1 and skips itself otherwise.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use winit::window::{WindowAttributes, WindowId};

const ROOT: &str = "parent-probe-root";
const DIALOG: &str = "parent-probe-dialog";
const TOOL: &str = "parent-probe-tool";

#[test]
fn transient_parent_attribute_round_trips() {
    let attributes = WindowAttributes::default();
    assert_eq!(attributes.transient_parent(), None, "default is no parent");
    let parent = WindowId::from(0x40_0001);
    let attributes = attributes.with_transient_parent(Some(parent));
    assert_eq!(attributes.transient_parent(), Some(parent));
    assert_eq!(
        attributes.with_transient_parent(None).transient_parent(),
        None
    );
}

struct Probe {
    quit: Arc<AtomicBool>,
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.label("root");
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(DIALOG),
            egui::ViewportBuilder::default()
                .with_title(DIALOG)
                .with_inner_size([200.0, 120.0]),
            |ui, _class| {
                ui.label("dialog");
            },
        );
        // A deferred viewport with its own X11 type: it gets the parent but keeps its type.
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of(TOOL),
            egui::ViewportBuilder::default()
                .with_title(TOOL)
                .with_inner_size([200.0, 120.0])
                .with_window_type(egui::X11WindowType::Utility),
            |ui, _class| {
                ui.label("tool");
            },
        );
        if self.quit.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

/// Runs the probe app on X11 in this (non-main) thread until `quit` is set.
fn run_probe(quit: Arc<AtomicBool>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(ROOT)
            .with_inner_size([320.0, 240.0]),
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11().with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "parent-probe",
        options,
        Box::new(move |_cc| Ok(Box::new(Probe { quit }))),
    )
    .map_err(|err| err.to_string())
}

/// The id of the window titled exactly `title`, once it exists.
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

fn xprop(id: u32, property: &str) -> String {
    let out = Command::new("xprop")
        .args(["-id", &id.to_string(), property])
        .output()
        .expect("xprop runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// `WM_TRANSIENT_FOR(WINDOW): window id # 0x400003` -> Some(0x400003); "not found" -> None.
fn transient_for(id: u32) -> Option<u32> {
    let line = xprop(id, "WM_TRANSIENT_FOR");
    let hex = line.split("# 0x").nth(1)?;
    u32::from_str_radix(hex.trim(), 16).ok()
}

/// `_NET_WM_WINDOW_TYPE(ATOM) = _NET_WM_WINDOW_TYPE_DIALOG` -> ["_NET_WM_WINDOW_TYPE_DIALOG"].
fn window_types(id: u32) -> Vec<String> {
    let line = xprop(id, "_NET_WM_WINDOW_TYPE");
    match line.split_once(" = ") {
        Some((_, types)) => types.split(", ").map(|t| t.trim().to_owned()).collect(),
        None => Vec::new(),
    }
}

#[test]
fn dialog_viewports_name_the_main_window_as_transient_parent() {
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
        for (slot, title) in ids.iter_mut().zip([ROOT, DIALOG, TOOL]) {
            if slot.is_none() {
                *slot = find_window(title);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    thread::sleep(Duration::from_secs(1));
    let [root, dialog, tool] = ids.map(Option::unwrap);

    let root_transient = transient_for(root);
    let root_types = window_types(root);
    let dialog_transient = transient_for(dialog);
    let dialog_types = window_types(dialog);
    let tool_transient = transient_for(tool);
    let tool_types = window_types(tool);

    quit.store(true, Ordering::SeqCst);
    let end = Instant::now() + Duration::from_secs(20);
    while !app.is_finished() && Instant::now() < end {
        thread::sleep(Duration::from_millis(100));
    }
    if app.is_finished() {
        let result = app.join().expect("probe thread does not panic");
        assert!(result.is_ok(), "probe app failed: {result:?}");
    }

    assert_eq!(root_transient, None, "the main window has no parent");
    assert_eq!(
        root_types,
        ["_NET_WM_WINDOW_TYPE_NORMAL"],
        "the main window stays normal"
    );
    assert_eq!(
        dialog_transient,
        Some(root),
        "an immediate viewport is transient for the main window ({root:#x})"
    );
    assert_eq!(
        dialog_types,
        ["_NET_WM_WINDOW_TYPE_DIALOG"],
        "a viewport with the default type becomes a dialog"
    );
    assert_eq!(
        tool_transient,
        Some(root),
        "a deferred viewport is transient for the main window ({root:#x})"
    );
    assert_eq!(
        tool_types,
        ["_NET_WM_WINDOW_TYPE_UTILITY"],
        "a viewport that asked for a type keeps it"
    );
}
