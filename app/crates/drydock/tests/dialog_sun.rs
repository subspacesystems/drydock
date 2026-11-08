// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-sun: Shadows and Geolocation as dialog windows. See docs/specs/m11-dialog-sun.md.

use drydock::editor::Editor;
use drydock::sun_windows::SunWindows;
use drydock_kernel::model::Model;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    windows: SunWindows,
    model: Model,
    editor: Editor,
}

fn harness(shadows: bool, geo: bool) -> Harness<'static, State> {
    let mut windows = SunWindows::new();
    windows.shadows_open = shadows;
    windows.geo_open = geo;
    let mut h = Harness::new_ui_state(
        |ui, s: &mut State| {
            let ctx = ui.ctx().clone();
            s.windows.show(&ctx, &mut s.model, &mut s.editor);
        },
        State {
            windows,
            model: Model::new(),
            editor: Editor::new(),
        },
    );
    h.run();
    h
}

#[test]
fn shadows_is_a_titled_window() {
    let h = harness(true, false);
    assert!(h.ctx.embed_viewports());
    h.get_by_label("Shadows");
    h.get_by_label("Close");
    assert!(h.query_by_label("Geolocation").is_none());
}

#[test]
fn escape_closes_shadows() {
    let mut h = harness(true, false);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().windows.shadows_open);
    assert!(h.query_by_label("Shadows").is_none());
}

#[test]
fn escape_closes_geolocation_and_reopening_rebuilds_it() {
    let mut h = harness(false, true);
    h.get_by_label("Geolocation");
    h.get_by_label("Apply");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().windows.geo_open);
    assert!(h.query_by_label("Geolocation").is_none());
    h.state_mut().windows.geo_open = true;
    h.run();
    h.get_by_label("Geolocation");
}

#[test]
fn both_windows_can_be_open_together() {
    let h = harness(true, true);
    h.get_by_label("Shadows");
    h.get_by_label("Geolocation");
}

// ---- real window -------------------------------------------------------------------

use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

fn tool(cmd: &str, args: &[&str]) -> Output {
    Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

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

fn own_window_test(hook: &str, title: &str) {
    if std::env::var_os("MODELER_GUI_TESTS").is_none() {
        eprintln!("skipped: MODELER_GUI_TESTS is unset");
        return;
    }
    let _app = Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env("DRYDOCK_OPEN_DIALOG", hook)
            .spawn()
            .expect("drydock starts"),
    );
    wait_for(title, || !windows(title).is_empty());
    let dialog = windows(title)[0].clone();
    let main = windows("^Drydock$");
    assert_eq!(main.len(), 1, "one main window: {main:?}");
    assert_ne!(main[0], dialog);
    let props =
        String::from_utf8_lossy(&tool("xprop", &["-id", &dialog, "WM_TRANSIENT_FOR"]).stdout)
            .into_owned();
    let parent = props
        .split("window id # ")
        .nth(1)
        .unwrap_or_else(|| panic!("{title} has no WM_TRANSIENT_FOR: {props}"))
        .trim()
        .to_owned();
    assert_eq!(
        u64::from_str_radix(parent.trim_start_matches("0x"), 16).unwrap(),
        main[0].parse::<u64>().unwrap(),
        "WM_TRANSIENT_FOR is the main window"
    );
    tool("xdotool", &["windowfocus", "--sync", &dialog]);
    tool("xdotool", &["key", "Escape"]);
    wait_for("the dialog to go", || windows(title).is_empty());
    assert_eq!(windows("^Drydock$").len(), 1, "the main window stays");
}

#[test]
fn shadows_opens_in_its_own_x_window() {
    own_window_test("shadows", "Shadows");
}

#[test]
fn geolocation_opens_in_its_own_x_window() {
    own_window_test("geolocation", "Geolocation");
}
