// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-window: the dialog-window helper. Unit and kittest parts run everywhere; the
//! real-window part needs Xvfb, xdotool, xprop and DRYDOCK_GUI_TESTS=1 and skips itself
//! otherwise. See docs/specs/m11-dialog-window.md.

use std::cell::RefCell;
use std::rc::Rc;

use drydock::dialog::{DialogWindow, centered_position, dialog_viewport_id, dialog_window};
use eframe::egui::{self, Pos2, Rect, Vec2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn centered_over_the_root_window() {
    let root = Rect::from_min_size(pos2(100.0, 50.0), vec2(1280.0, 800.0));
    assert_eq!(
        centered_position(root, vec2(400.0, 300.0)),
        pos2(540.0, 300.0)
    );
}

#[test]
fn centered_position_is_never_negative() {
    let root = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0));
    let p = centered_position(root, vec2(400.0, 300.0));
    assert_eq!(p, Pos2::ZERO);
    let root = Rect::from_min_size(pos2(-30.0, 10.0), vec2(300.0, 300.0));
    let p = centered_position(root, vec2(100.0, 100.0));
    assert_eq!(p, pos2(70.0, 110.0));
}

#[test]
fn viewport_id_follows_the_title() {
    assert_eq!(
        dialog_viewport_id("Model Info"),
        dialog_viewport_id("Model Info")
    );
    assert_ne!(
        dialog_viewport_id("Model Info"),
        dialog_viewport_id("About Drydock")
    );
    assert_ne!(dialog_viewport_id("Model Info"), egui::ViewportId::ROOT);
}

fn harness(open: Rc<RefCell<bool>>) -> Harness<'static> {
    Harness::new_ui(move |ui| {
        let mut o = *open.borrow();
        let win = DialogWindow {
            title: "Test Dialog",
            size: Vec2::new(300.0, 120.0),
        };
        dialog_window(ui.ctx(), &win, &mut o, |ui| {
            ui.label("hello from the dialog");
        });
        *open.borrow_mut() = o;
    })
}

#[test]
fn embedded_dialog_is_a_titled_window_with_the_content() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open.clone());
    assert!(h.ctx.embed_viewports(), "kittest embeds viewports");
    h.run();
    h.get_by_label("Test Dialog");
    h.get_by_label("hello from the dialog");
    assert!(*open.borrow());
}

#[test]
fn nothing_is_drawn_while_closed() {
    let open = Rc::new(RefCell::new(false));
    let mut h = harness(open);
    h.run();
    assert!(h.query_by_label("hello from the dialog").is_none());
    assert!(h.query_by_label("Test Dialog").is_none());
}

#[test]
fn escape_closes_the_dialog() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open.clone());
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!*open.borrow());
}

// ---- real window -------------------------------------------------------------------

use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

fn gui_tests() -> bool {
    std::env::var_os("DRYDOCK_GUI_TESTS").is_some()
}

fn tool(cmd: &str, args: &[&str]) -> Output {
    Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

/// Ids of the X windows whose name contains `name`.
fn windows(name: &str) -> Vec<String> {
    let out = tool("xdotool", &["search", "--name", name]);
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(40);
    while Instant::now() < end {
        if ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    panic!("timed out waiting for {what}");
}

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn about_opens_in_its_own_x_window_and_escape_closes_it() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    // DRYDOCK_OPEN_DIALOG=about opens the About dialog as the app starts (test hook).
    let _app = Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env("DRYDOCK_OPEN_DIALOG", "about")
            .spawn()
            .expect("drydock starts"),
    );
    wait_for("the About window", || !windows("About Drydock").is_empty());
    let about = windows("About Drydock")[0].clone();
    // The main window is the other Drydock window, whose title is exactly "Drydock".
    let main = windows("^Drydock$");
    assert_eq!(main.len(), 1, "one main window: {main:?}");
    assert_ne!(main[0], about);
    let props =
        String::from_utf8_lossy(&tool("xprop", &["-id", &about, "WM_TRANSIENT_FOR"]).stdout)
            .into_owned();
    let parent = props
        .split("window id # ")
        .nth(1)
        .unwrap_or_else(|| panic!("About has no WM_TRANSIENT_FOR: {props}"))
        .trim()
        .to_owned();
    assert_eq!(
        u64::from_str_radix(parent.trim_start_matches("0x"), 16).unwrap(),
        main[0].parse::<u64>().unwrap(),
        "WM_TRANSIENT_FOR is the main window"
    );
    tool("xdotool", &["windowfocus", "--sync", &about]);
    tool("xdotool", &["key", "Escape"]);
    wait_for("the About window to go", || {
        windows("About Drydock").is_empty()
    });
    assert_eq!(windows("^Drydock$").len(), 1, "the main window stays");
}
