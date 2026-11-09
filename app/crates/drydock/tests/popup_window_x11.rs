// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-winit-popup: a viewport whose title was marked with `egui_winit::mark_popup_title` gets
//! a pop-up window. Only X11 can be checked here: the test opens a root window, one marked and
//! one unmarked viewport on X11 and reads their properties with `xprop` and `xdotool`. It runs
//! under Xvfb with DRYDOCK_GUI_TESTS=1 and skips itself otherwise.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use winit::window::WindowAttributes;

const ROOT: &str = "popup-probe-root";
const POPUP: &str = "popup-probe-menu";
const PLAIN: &str = "popup-probe-plain";
/// Where the pop-up is asked to open (screen pixels, scale factor 1 under Xvfb).
const AT: (i32, i32) = (320, 210);

#[test]
fn popup_attribute_round_trips() {
    let attributes = WindowAttributes::default();
    assert!(!attributes.popup(), "default is not a pop-up");
    let attributes = attributes.with_popup(true);
    assert!(attributes.popup());
    assert!(!attributes.with_popup(false).popup());
}

#[test]
fn marked_popup_titles_are_remembered_per_context() {
    let ctx = egui::Context::default();
    assert!(
        !egui_winit::is_popup_title(&ctx, POPUP),
        "nothing marked yet"
    );
    egui_winit::mark_popup_title(&ctx, POPUP);
    egui_winit::mark_popup_title(&ctx, POPUP);
    assert!(egui_winit::is_popup_title(&ctx, POPUP));
    assert!(
        !egui_winit::is_popup_title(&ctx, PLAIN),
        "only the marked title"
    );
    assert!(
        !egui_winit::is_popup_title(&ctx, "popup-probe"),
        "titles match exactly, not by prefix"
    );
    assert!(
        !egui_winit::is_modal_title(&ctx, POPUP),
        "popup marks and modal marks are separate"
    );
    let other = egui::Context::default();
    assert!(
        !egui_winit::is_popup_title(&other, POPUP),
        "another context has its own marks"
    );
}

struct Probe {
    quit: Arc<AtomicBool>,
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.label("root");
        // Decorations are left at egui's default (on): the patch must turn them off.
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(POPUP),
            egui::ViewportBuilder::default()
                .with_title(POPUP)
                .with_position([AT.0 as f32, AT.1 as f32])
                .with_inner_size([160.0, 100.0]),
            |ui, _class| {
                ui.label("menu");
            },
        );
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of(PLAIN),
            egui::ViewportBuilder::default()
                .with_title(PLAIN)
                .with_inner_size([200.0, 120.0]),
            |ui, _class| {
                ui.label("plain");
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
        "popup-probe",
        options,
        Box::new(move |cc| {
            // The root is never a pop-up, even if its title is marked.
            egui_winit::mark_popup_title(&cc.egui_ctx, POPUP);
            egui_winit::mark_popup_title(&cc.egui_ctx, ROOT);
            Ok(Box::new(Probe { quit }))
        }),
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

/// The text after ` = ` of `xprop -id ID PROP`, or "" when the property is not set.
fn xprop(id: u32, prop: &str) -> String {
    let out = Command::new("xprop")
        .args(["-id", &id.to_string(), prop])
        .output()
        .expect("xprop runs");
    let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    match line.split_once(" = ") {
        Some((_, value)) => value.trim().to_owned(),
        None => match line.split_once("): ") {
            // `WM_TRANSIENT_FOR(WINDOW): window id # 0x1400004`
            Some((_, value)) => value.trim().to_owned(),
            None => String::new(),
        },
    }
}

/// `_NET_WM_STATE(ATOM) = A, B` -> ["A", "B"]; not set -> [].
fn names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The window's top-left corner on the screen, from `xdotool getwindowgeometry`
/// ("  Position: 320,210 (screen: 0)").
fn position(id: u32) -> (i32, i32) {
    let out = Command::new("xdotool")
        .args(["getwindowgeometry", &id.to_string()])
        .output()
        .expect("xdotool runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("Position:"))
        .unwrap_or_else(|| panic!("no Position line in {text:?}"));
    let coords = line.trim_start().trim_start_matches("Position:").trim();
    let coords = coords.split_whitespace().next().unwrap_or("");
    let (x, y) = coords.split_once(',').expect("x,y");
    (x.parse().expect("x"), y.parse().expect("y"))
}

/// `_MOTIF_WM_HINTS` says "no decorations": the decorations flag (bit 1 of the flags) is set and
/// the decorations field is 0. The property is `flags, functions, decorations, input, status`.
fn motif_undecorated(id: u32) -> bool {
    let fields: Vec<u64> = names(&xprop(id, "_MOTIF_WM_HINTS"))
        .iter()
        .filter_map(|f| u64::from_str_radix(f.trim_start_matches("0x"), 16).ok())
        .collect();
    fields.len() >= 3 && fields[0] & 2 != 0 && fields[2] == 0
}

#[test]
fn a_marked_viewport_is_a_popup_and_the_others_are_not() {
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
        for (slot, title) in ids.iter_mut().zip([ROOT, POPUP, PLAIN]) {
            if slot.is_none() {
                *slot = find_window(title);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    thread::sleep(Duration::from_secs(1));
    let [root, popup, plain] = ids.map(Option::unwrap);

    let read = |id: u32| {
        (
            xprop(id, "_NET_WM_WINDOW_TYPE"),
            names(&xprop(id, "_NET_WM_STATE")),
            xprop(id, "WM_TRANSIENT_FOR"),
        )
    };
    let (root_type, root_state, root_parent) = read(root);
    let (popup_type, popup_state, popup_parent) = read(popup);
    let (plain_type, plain_state, _) = read(plain);
    let popup_undecorated = motif_undecorated(popup);
    let plain_undecorated = motif_undecorated(plain);
    let popup_at = position(popup);

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
        popup_type.contains("_NET_WM_WINDOW_TYPE_POPUP_MENU"),
        "the pop-up has the pop-up menu type: {popup_type:?}"
    );
    for atom in ["_NET_WM_STATE_SKIP_TASKBAR", "_NET_WM_STATE_SKIP_PAGER"] {
        assert!(
            popup_state.iter().any(|s| s == atom),
            "the pop-up has {atom}: {popup_state:?}"
        );
        assert_eq!(
            popup_state.iter().filter(|s| *s == atom).count(),
            1,
            "{atom} is listed once: {popup_state:?}"
        );
    }
    assert!(
        popup_parent.contains("0x") && popup_parent.contains(&format!("{root:#x}")),
        "the pop-up is transient for the main window {root:#x}: {popup_parent:?}"
    );
    assert!(popup_undecorated, "the pop-up has no decorations");
    assert!(
        (popup_at.0 - AT.0).abs() <= 2 && (popup_at.1 - AT.1).abs() <= 2,
        "the pop-up opens at {AT:?}, found {popup_at:?}"
    );

    assert!(
        !plain_type.contains("POPUP_MENU"),
        "an unmarked viewport is no pop-up: {plain_type:?}"
    );
    assert!(
        !plain_state
            .iter()
            .any(|s| s == "_NET_WM_STATE_SKIP_TASKBAR"),
        "an unmarked viewport keeps its taskbar entry: {plain_state:?}"
    );
    assert!(
        !plain_undecorated,
        "an unmarked viewport keeps its decorations"
    );
    assert!(
        !root_type.contains("POPUP_MENU")
            && !root_state.iter().any(|s| s == "_NET_WM_STATE_SKIP_TASKBAR"),
        "the main window is never a pop-up: {root_type:?} {root_state:?}"
    );
    assert!(
        root_parent.is_empty(),
        "the main window has no parent: {root_parent:?}"
    );
}
