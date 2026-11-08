// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-modal-window, real windows: needs Xvfb (no window manager), xdotool, xprop and
//! `DRYDOCK_GUI_TESTS=1`; each test skips itself otherwise. The probe is
//! `examples/modal_probe.rs`. See docs/specs/m11-modal-window.md.

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

/// Ids of the X windows whose name matches the regular expression `name`.
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

/// Waits for the single window matching `name` and returns its id.
fn the_window(name: &str) -> String {
    wait_for(name, || windows(name).len() == 1);
    windows(name)[0].clone()
}

fn settle() {
    std::thread::sleep(Duration::from_millis(1500));
}

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn probe_path() -> PathBuf {
    // target/<profile>/deps/<this test> -> target/<profile>/examples/modal_probe
    let exe = std::env::current_exe().unwrap();
    let profile = exe.parent().unwrap().parent().unwrap();
    let path = profile.join("examples").join("modal_probe");
    if !path.exists() {
        // A test filter does not build the examples; build the probe into the same target
        // directory so the test works from a clean `cargo test -p drydock --test`.
        let mut build = Command::new("cargo");
        build
            .args(["build", "-p", "drydock", "--examples", "--manifest-path"])
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .env("CARGO_TARGET_DIR", profile.parent().unwrap());
        if profile.file_name().and_then(|name| name.to_str()) == Some("release") {
            build.arg("--release");
        }
        let status = build
            .status()
            .expect("cargo build -p drydock --examples runs");
        assert!(status.success(), "building the modal_probe example failed");
    }
    path
}

fn property(id: &str, name: &str) -> String {
    text(&tool("xprop", &["-id", id, name]))
}

fn parse_id(s: &str) -> u64 {
    u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap()
}

fn assert_transient_for(dialog: &str, main: &str) {
    let props = property(dialog, "WM_TRANSIENT_FOR");
    let parent = props
        .split("window id # ")
        .nth(1)
        .unwrap_or_else(|| panic!("no WM_TRANSIENT_FOR: {props}"));
    assert_eq!(parse_id(parent), main.parse::<u64>().unwrap());
}

fn title(id: &str) -> String {
    text(&tool("xdotool", &["getwindowname", id]))
        .trim()
        .to_owned()
}

fn click_main(main: &str, x: &str, y: &str) {
    tool(
        "xdotool",
        &["mousemove", "--window", main, x, y, "click", "1"],
    );
}

fn type_into(window: &str, keys: &str) {
    tool("xdotool", &["windowfocus", "--sync", window]);
    tool("xdotool", &["type", "--delay", "80", keys]);
}

#[test]
fn a_modal_prompt_blocks_the_main_window_until_it_is_answered() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let _app = Kill(
        Command::new(probe_path())
            .arg("--modal")
            .spawn()
            .expect("modal_probe starts (cargo test builds the example)"),
    );
    let main = the_window("^probe clicks");
    let dialog = the_window("^Probe Name$");
    assert_ne!(main, dialog);

    // The prompt is a window of its own, owned by the main window, and says it is modal.
    assert_transient_for(&dialog, &main);
    let state = property(&dialog, "_NET_WM_STATE");
    assert!(
        state.contains("_NET_WM_STATE_MODAL"),
        "the prompt is marked modal: {state}"
    );

    // While it is open the main window gets neither clicks nor keys. (There is no window
    // manager, so nothing but the application stops them.)
    assert!(
        title(&main).contains("clicks=0 typed= "),
        "{}",
        title(&main)
    );
    click_main(&main, "40", "340");
    type_into(&main, "abc");
    settle();
    let blocked = title(&main);
    assert!(
        blocked.contains("clicks=0 typed= answer=none"),
        "the main window ignored the click and the keys: {blocked}"
    );
    assert_eq!(windows("^Probe Name$").len(), 1, "the prompt stays");

    // Typing and Enter in the prompt answer it.
    type_into(&dialog, "box");
    tool("xdotool", &["key", "Return"]);
    wait_for("the prompt to go", || windows("^Probe Name$").is_empty());
    wait_for("the answer", || title(&main).contains("answer=ok:box"));

    // Now the main window works again, and received nothing in between.
    assert!(
        title(&main).contains("clicks=0 typed= "),
        "{}",
        title(&main)
    );
    click_main(&main, "40", "340");
    type_into(&main, "xy");
    wait_for("the click and the keys to arrive", || {
        let t = title(&main);
        t.contains("clicks=1 typed=xy ")
    });
}

#[test]
fn escape_cancels_a_modal_prompt_and_unblocks_the_main_window() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let _app = Kill(
        Command::new(probe_path())
            .arg("--modal")
            .spawn()
            .expect("modal_probe starts"),
    );
    let main = the_window("^probe clicks");
    let dialog = the_window("^Probe Name$");
    tool("xdotool", &["windowfocus", "--sync", &dialog]);
    tool("xdotool", &["key", "Escape"]);
    wait_for("the prompt to go", || windows("^Probe Name$").is_empty());
    wait_for("the answer", || title(&main).contains("answer=cancel"));
    click_main(&main, "40", "340");
    wait_for("the click to arrive", || title(&main).contains("clicks=1 "));
}

#[test]
fn the_make_group_prompt_is_a_modal_window_of_the_app() {
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
    let main = windows("^Drydock$");
    assert_eq!(main.len(), 1, "one main window: {main:?}");
    assert_transient_for(&prompt, &main[0]);
    let state = property(&prompt, "_NET_WM_STATE");
    assert!(state.contains("_NET_WM_STATE_MODAL"), "modal: {state}");

    tool("xdotool", &["windowfocus", "--sync", &prompt]);
    tool("xdotool", &["key", "Escape"]);
    wait_for("the prompt to go", || windows("^Make Group$").is_empty());
    assert_eq!(windows("^Drydock$").len(), 1, "the main window stays");
}
