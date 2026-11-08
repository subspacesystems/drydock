// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-title-fresh-doc: the app opens on a fresh document and its window is titled with that
//! document ("untitled.drydock - Drydock"), not the bare creation title "Drydock". Runs the
//! real binary under Xvfb with xdotool and MODELER_GUI_TESTS=1; skips itself otherwise.
//! With the DRYDOCK_OPEN_DIALOG test hook the main window keeps the bare title (the
//! real-window dialog tests look for it by that name).

use drydock::print_setup::PrintSettings;
use drydock::start::Fresh;

#[test]
fn a_fresh_document_has_the_untitled_title() {
    let fresh = Fresh::new(&PrintSettings::default());
    assert_eq!(fresh.title, "untitled.drydock - Drydock");
    assert_eq!(fresh.title, fresh.document.title());
}

use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn windows(pattern: &str) -> Vec<String> {
    let out = Command::new("xdotool")
        .args(["search", "--name", pattern])
        .output()
        .expect("xdotool runs");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn wait_until(mut ready: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(40);
    while Instant::now() < end {
        if ready() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

#[test]
fn the_real_window_is_titled_with_the_fresh_document_after_startup() {
    if std::env::var_os("MODELER_GUI_TESTS").is_none() {
        eprintln!("skipped: MODELER_GUI_TESTS is unset");
        return;
    }
    let _app = Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env_remove("DRYDOCK_OPEN_DIALOG")
            .spawn()
            .expect("drydock starts"),
    );
    assert!(
        wait_until(|| !windows("^untitled.drydock - Drydock$").is_empty()),
        "no window titled 'untitled.drydock - Drydock' appeared"
    );
    assert!(
        windows("^Drydock$").is_empty(),
        "the window still has the bare creation title"
    );
}
