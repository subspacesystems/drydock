// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-unsaved-save, real window: with unsaved changes on an untitled model, File > New asks
//! "Unsaved changes"; Save there must go on to the Save model picker (it used to only close
//! the prompt). Cancelling the picker leaves the model modified. Runs the real binary under
//! Xvfb (no window manager) with xdotool and `DRYDOCK_GUI_TESTS=1`; skips itself otherwise.
//! See docs/specs/m11-unsaved-save.md.

use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

fn tool(cmd: &str, args: &[&str]) -> Output {
    Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

fn windows(name: &str) -> Vec<String> {
    String::from_utf8_lossy(&tool("xdotool", &["search", "--name", name]).stdout)
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

fn title(id: &str) -> String {
    String::from_utf8_lossy(&tool("xdotool", &["getwindowname", id]).stdout)
        .trim()
        .to_owned()
}

fn pause(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Width and height of a window.
fn size(id: &str) -> (i32, i32) {
    let out = tool("xdotool", &["getwindowgeometry", "--shell", id]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let value = |key: &str| -> i32 {
        text.lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|rest| rest.trim().parse().ok())
            .unwrap_or_else(|| panic!("no {key} in {text}"))
    };
    (value("WIDTH="), value("HEIGHT="))
}

fn click_at(main: &str, x: i32, y: i32) {
    tool(
        "xdotool",
        &[
            "mousemove",
            "--window",
            main,
            &x.to_string(),
            &y.to_string(),
        ],
    );
    pause(300);
    tool("xdotool", &["click", "1"]);
    pause(300);
}

#[test]
fn save_in_the_unsaved_prompt_goes_on_to_the_save_picker_and_cancel_keeps_the_model() {
    if std::env::var_os("DRYDOCK_GUI_TESTS").is_none() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let _app = Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env_remove("DRYDOCK_OPEN_DIALOG")
            .env("DRYDOCK_FILE_PICKER", "builtin")
            .spawn()
            .expect("drydock starts"),
    );
    wait_for("the main window", || {
        windows("^untitled.drydock - Drydock$").len() == 1
    });
    let main = windows("^untitled.drydock - Drydock$")[0].clone();
    pause(1500);

    // Draw a rectangle so the document is modified.
    let (w, h) = size(&main);
    tool("xdotool", &["windowfocus", "--sync", &main]);
    tool("xdotool", &["key", "r"]);
    pause(300);
    click_at(&main, w / 2 - 60, h / 2);
    click_at(&main, w / 2 + 60, h / 2 + 40);
    wait_for("the title to show unsaved changes", || {
        title(&main).contains('*')
    });

    // File > New asks first.
    tool("xdotool", &["key", "ctrl+n"]);
    wait_for("the Unsaved changes window", || {
        windows("^Unsaved changes$").len() == 1
    });
    let prompt = windows("^Unsaved changes$")[0].clone();
    pause(800);

    // Save is the first control: Tab reaches it, Enter presses it.
    tool("xdotool", &["windowfocus", "--sync", &prompt]);
    tool("xdotool", &["key", "Tab"]);
    pause(300);
    tool("xdotool", &["key", "Return"]);
    wait_for("the prompt to go", || {
        windows("^Unsaved changes$").is_empty()
    });
    wait_for("the Save model picker", || {
        windows("^Save model$").len() == 1
    });

    // Cancelling the picker leaves the model as it was, and the main window usable.
    let picker = windows("^Save model$")[0].clone();
    tool("xdotool", &["windowfocus", "--sync", &picker]);
    tool("xdotool", &["key", "Escape"]);
    wait_for("the picker to go", || windows("^Save model$").is_empty());
    pause(1000);
    assert!(windows("^Unsaved changes$").is_empty());
    let after = title(&main);
    assert!(
        after.contains('*') && after.starts_with("untitled"),
        "the document is unchanged and still modified: {after}"
    );
}
