// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-window-text-focus, real windows: needs Xvfb (no window manager, no input method
//! server), xdotool, xprop and `DRYDOCK_GUI_TESTS=1`; each test skips itself otherwise. The
//! probe is `examples/modal_probe.rs` (its main-window text box and its prompt's Name box both
//! ask for focus on every frame). See docs/specs/m11-window-text-focus.md.

use std::path::PathBuf;
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

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Ids of the X windows whose name matches the regular expression `name` (case-insensitive).
fn windows(name: &str) -> Vec<String> {
    text(&tool("xdotool", &["search", "--name", name]))
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

fn the_window(name: &str) -> String {
    wait_for(name, || windows(name).len() == 1);
    windows(name)[0].clone()
}

fn title(id: &str) -> String {
    text(&tool("xdotool", &["getwindowname", id]))
        .trim()
        .to_owned()
}

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn probe(args: &[&str]) -> Kill {
    // target/<profile>/deps/<this test> -> target/<profile>/examples/modal_probe
    let exe = std::env::current_exe().unwrap();
    let profile = exe.parent().unwrap().parent().unwrap();
    let path: PathBuf = profile.join("examples").join("modal_probe");
    Kill(
        Command::new(path)
            .args(args)
            .spawn()
            .expect("modal_probe starts (cargo build -p drydock --examples first)"),
    )
}

fn type_into(window: &str, keys: &str) {
    tool("xdotool", &["windowfocus", "--sync", window]);
    tool("xdotool", &["type", "--delay", "80", keys]);
}

#[test]
fn a_text_box_that_asks_for_focus_as_the_window_opens_gets_its_keys_in_order() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let _app = probe(&[]);
    let main = the_window("^probe clicks");
    type_into(&main, "abcdef");
    wait_for("the typed text", || title(&main).contains(" typed=abcdef "));
}

#[test]
fn a_prompt_name_box_gets_its_keys_in_order_and_enter_answers_it() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let _app = probe(&["--modal"]);
    let main = the_window("^probe clicks");
    let dialog = the_window("^Probe Name$");
    type_into(&dialog, "hello");
    tool("xdotool", &["key", "Return"]);
    wait_for("the prompt to go", || windows("^Probe Name$").is_empty());
    wait_for("the answer, typed in order", || {
        title(&main).contains("answer=ok:hello")
    });
}

#[test]
fn typing_in_the_make_group_prompt_and_pressing_enter_closes_it() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    // DRYDOCK_OPEN_DIALOG=group opens the Make Group prompt as the app starts (test hook).
    let _app = Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env("DRYDOCK_OPEN_DIALOG", "group")
            .spawn()
            .expect("drydock starts"),
    );
    let prompt = the_window("^Make Group$");
    type_into(&prompt, "Chair");
    tool("xdotool", &["key", "Return"]);
    wait_for("the prompt to go", || windows("^Make Group$").is_empty());
    assert_eq!(windows("^Drydock$").len(), 1, "the main window stays");
}
