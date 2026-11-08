// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-winit-modal: a viewport whose title was marked with `egui_winit::mark_modal_title` gets
//! a modal window. Only X11 can be checked here: the test opens a root window, one marked and
//! one unmarked viewport on X11 and reads `_NET_WM_STATE` with `xprop`. It runs under Xvfb with
//! DRYDOCK_GUI_TESTS=1 and skips itself otherwise.
#![cfg(target_os = "linux")]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use winit::window::WindowAttributes;

const ROOT: &str = "modal-probe-root";
const PROMPT: &str = "modal-probe-prompt";
const PLAIN: &str = "modal-probe-plain";
const MODAL_STATE: &str = "_NET_WM_STATE_MODAL";

#[test]
fn modal_attribute_round_trips() {
    let attributes = WindowAttributes::default();
    assert!(!attributes.modal(), "default is not modal");
    let attributes = attributes.with_modal(true);
    assert!(attributes.modal());
    assert!(!attributes.with_modal(false).modal());
}

#[test]
fn marked_titles_are_remembered_per_context() {
    let ctx = egui::Context::default();
    assert!(
        !egui_winit::is_modal_title(&ctx, PROMPT),
        "nothing marked yet"
    );
    egui_winit::mark_modal_title(&ctx, PROMPT);
    egui_winit::mark_modal_title(&ctx, PROMPT);
    assert!(egui_winit::is_modal_title(&ctx, PROMPT));
    assert!(
        !egui_winit::is_modal_title(&ctx, PLAIN),
        "only the marked title"
    );
    assert!(
        !egui_winit::is_modal_title(&ctx, "modal-probe"),
        "titles match exactly, not by prefix"
    );
    let other = egui::Context::default();
    assert!(
        !egui_winit::is_modal_title(&other, PROMPT),
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
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(PROMPT),
            egui::ViewportBuilder::default()
                .with_title(PROMPT)
                .with_inner_size([200.0, 120.0]),
            |ui, _class| {
                ui.label("prompt");
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
        "modal-probe",
        options,
        Box::new(move |cc| {
            // Marked before the prompt is first shown; the root window exists already and the
            // root is never modal, even if its title is marked.
            egui_winit::mark_modal_title(&cc.egui_ctx, PROMPT);
            egui_winit::mark_modal_title(&cc.egui_ctx, ROOT);
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

/// `_NET_WM_STATE(ATOM) = _NET_WM_STATE_MODAL, _NET_WM_STATE_ABOVE` -> both atom names;
/// `_NET_WM_STATE:  not found.` or an empty list -> no names.
fn wm_states(id: u32) -> Vec<String> {
    let out = Command::new("xprop")
        .args(["-id", &id.to_string(), "_NET_WM_STATE"])
        .output()
        .expect("xprop runs");
    let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    match line.split_once(" = ") {
        Some((_, states)) => states
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

#[test]
fn a_marked_viewport_is_modal_and_the_others_are_not() {
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
        for (slot, title) in ids.iter_mut().zip([ROOT, PROMPT, PLAIN]) {
            if slot.is_none() {
                *slot = find_window(title);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    thread::sleep(Duration::from_secs(1));
    let [root, prompt, plain] = ids.map(Option::unwrap);

    let root_states = wm_states(root);
    let prompt_states = wm_states(prompt);
    let plain_states = wm_states(plain);

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
        prompt_states.iter().any(|s| s == MODAL_STATE),
        "the marked viewport's window has {MODAL_STATE}: {prompt_states:?}"
    );
    assert_eq!(
        prompt_states.iter().filter(|s| *s == MODAL_STATE).count(),
        1,
        "{MODAL_STATE} is listed once: {prompt_states:?}"
    );
    assert!(
        !plain_states.iter().any(|s| s == MODAL_STATE),
        "an unmarked viewport is not modal: {plain_states:?}"
    );
    assert!(
        !root_states.iter().any(|s| s == MODAL_STATE),
        "the main window is never modal: {root_states:?}"
    );
}
