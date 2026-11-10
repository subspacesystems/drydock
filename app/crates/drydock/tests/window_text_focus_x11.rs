// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-window-text-focus, real windows: needs Xvfb (no window manager, no input method
//! server), xdotool, xprop and `DRYDOCK_GUI_TESTS=1`; each test skips itself otherwise. The
//! probe is `examples/modal_probe.rs` (its main-window text box and its prompt's Name box both
//! ask for focus on every frame). See docs/specs/m11-window-text-focus.md.

use std::path::PathBuf;
use std::process::{Child, Command, Output};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The probes share window titles, so only one test may drive this display at a time.
static DISPLAY_LOCK: Mutex<()> = Mutex::new(());

fn gui_tests() -> bool {
    std::env::var_os("DRYDOCK_GUI_TESTS").is_some()
}

fn serialize() -> MutexGuard<'static, ()> {
    DISPLAY_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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

/// Starts the app with `start` and waits for the single window matching `name`. The display
/// lets go of its state when its last client leaves (as the previous test's app is killed), and
/// an app that connects at that moment exits at once (`XOpenDisplayFailed`), so an app that
/// exits before its window appears is started again.
fn launch(start: impl Fn() -> Command, name: &str) -> (Kill, String) {
    let spawn = || Kill(start().spawn().expect("the app under test starts"));
    let mut app = spawn();
    let end = Instant::now() + Duration::from_secs(40);
    loop {
        if let [id] = windows(name).as_slice() {
            return (app, id.clone());
        }
        assert!(Instant::now() < end, "timed out waiting for {name}");
        if matches!(app.0.try_wait(), Ok(Some(_))) {
            app = spawn();
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Starts `examples/modal_probe` (cargo test builds it) and waits for its main window.
fn probe(args: &[&str]) -> (Kill, String) {
    // target/<profile>/deps/<this test> -> target/<profile>/examples/modal_probe
    let exe = std::env::current_exe().unwrap();
    let profile = exe.parent().unwrap().parent().unwrap();
    let path: PathBuf = profile.join("examples").join("modal_probe");
    launch(
        || {
            let mut command = Command::new(&path);
            command.args(args);
            command
        },
        "^probe clicks",
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
    let _serial = serialize();
    let (_app, main) = probe(&[]);
    type_into(&main, "abcdef");
    wait_for("the typed text", || title(&main).contains(" typed=abcdef "));
}

#[test]
fn a_prompt_name_box_gets_its_keys_in_order_and_enter_answers_it() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let _serial = serialize();
    let (_app, main) = probe(&["--modal"]);
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
    let _serial = serialize();
    let (_app, prompt) = launch(
        || {
            let mut command = Command::new(env!("CARGO_BIN_EXE_drydock"));
            command.env("DRYDOCK_OPEN_DIALOG", "group");
            command
        },
        "^Make Group$",
    );
    type_into(&prompt, "Chair");
    tool("xdotool", &["key", "Return"]);
    wait_for("the prompt to go", || windows("^Make Group$").is_empty());
    assert_eq!(windows("^Drydock$").len(), 1, "the main window stays");
}
