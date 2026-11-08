// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-fit-content: dialog windows take their height from their content. The unit part
//! runs everywhere; the real-window part needs Xvfb, xdotool, xwininfo and DRYDOCK_GUI_TESTS=1
//! and skips itself otherwise. See docs/specs/m11-dialog-fit-content.md.

use drydock::dialog::{DialogFit, SCREEN_MARGIN, fit_height, needs_resize};

#[test]
fn content_that_fits_sets_the_height() {
    assert_eq!(
        fit_height(250.0, Some(1080.0)),
        DialogFit {
            height: 250.0,
            scroll: false
        }
    );
}

#[test]
fn unknown_monitor_does_not_clamp() {
    assert_eq!(
        fit_height(5000.0, None),
        DialogFit {
            height: 5000.0,
            scroll: false
        }
    );
}

#[test]
fn content_taller_than_the_screen_is_clamped_and_scrolls() {
    assert_eq!(
        fit_height(2000.0, Some(1000.0)),
        DialogFit {
            height: 1000.0 - SCREEN_MARGIN,
            scroll: true
        }
    );
}

#[test]
fn content_exactly_at_the_limit_does_not_scroll() {
    let limit = 1000.0 - SCREEN_MARGIN;
    assert_eq!(
        fit_height(limit, Some(1000.0)),
        DialogFit {
            height: limit,
            scroll: false
        }
    );
}

#[test]
fn unusable_monitor_heights_do_not_clamp() {
    for m in [0.0, -5.0, SCREEN_MARGIN, f32::NAN] {
        let fit = fit_height(900.0, Some(m));
        assert_eq!(fit.height, 900.0, "monitor {m}");
        assert!(!fit.scroll, "monitor {m}");
    }
}

#[test]
fn negative_content_gives_zero() {
    assert_eq!(fit_height(-3.0, None).height, 0.0);
}

#[test]
fn resize_only_when_more_than_a_point_apart() {
    assert!(!needs_resize(300.0, 300.0));
    assert!(!needs_resize(300.0, 300.9));
    assert!(!needs_resize(300.0, 299.1));
    assert!(needs_resize(300.0, 301.5));
    assert!(needs_resize(300.0, 280.0));
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

/// The inner height in pixels of X window `id`.
fn height(id: &str) -> u32 {
    let out = tool("xwininfo", &["-id", id]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.trim().strip_prefix("Height:"))
        .and_then(|h| h.trim().parse().ok())
        .expect("xwininfo prints a Height")
}

/// Opens the dialog `title` through the hook and returns its height once it has settled
/// (the same value on two reads half a second apart).
fn settled_height(hook: &str, title: &str) -> (Kill, u32) {
    let app = Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env("DRYDOCK_OPEN_DIALOG", hook)
            .spawn()
            .expect("drydock starts"),
    );
    wait_for(title, || !windows(title).is_empty());
    let id = windows(title)[0].clone();
    let mut last = height(&id);
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        std::thread::sleep(Duration::from_millis(600));
        let now = height(&id);
        if now == last || Instant::now() > end {
            return (app, now);
        }
        last = now;
    }
}

#[test]
fn model_info_grows_past_its_first_size_to_show_everything() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let (_app, h) = settled_height("model-info", "Model Info");
    assert!(h > 300, "Model Info is {h} px high, still the fixed 300");
}

#[test]
fn model_info_geolocation_button_is_reachable() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let (_app, h) = settled_height("model-info", "Model Info");
    let id = windows("Model Info")[0].clone();
    // The Geolocation... button is the last control before Close, near the bottom: click along
    // the bottom 90 px band in a few places and see whether a Geolocation window opens.
    tool("xdotool", &["windowfocus", "--sync", &id]);
    'found: for dy in [90u32, 80, 70, 60, 50] {
        for x in [60u32, 80, 100] {
            if h < dy {
                continue;
            }
            tool(
                "xdotool",
                &[
                    "mousemove",
                    "--window",
                    &id,
                    &x.to_string(),
                    &(h - dy).to_string(),
                    "click",
                    "1",
                ],
            );
            std::thread::sleep(Duration::from_millis(250));
            if !windows("Geolocation").is_empty() {
                break 'found;
            }
        }
    }
    assert!(
        !windows("Geolocation").is_empty(),
        "no Geolocation window after clicking the bottom of Model Info ({h} px)"
    );
}

#[test]
fn about_is_no_taller_than_its_content() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let (_app, h) = settled_height("about", "About Drydock");
    assert!(h < 400, "About is {h} px high, far more than its content");
}

#[test]
fn customize_toolbars_fits_the_screen() {
    if !gui_tests() {
        eprintln!("skipped: DRYDOCK_GUI_TESTS is unset");
        return;
    }
    let (_app, h) = settled_height("customize", "Customize Toolbars");
    let screen = String::from_utf8_lossy(&tool("xdotool", &["getdisplaygeometry"]).stdout)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .expect("display geometry");
    assert!(
        h <= screen,
        "Customize Toolbars ({h} px) is taller than the screen ({screen})"
    );
}
