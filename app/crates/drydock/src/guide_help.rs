// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Help > User Guide: finds the generated guide (`drydock-guide` output) and opens its
//! index page in the default browser. The browser launch sits behind `BrowserLauncher`
//! so tests stub it.
//!
//! Behaviour: `docs/specs/m8-guide-help.md`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::look::Os;

/// The guide's start page inside its folder.
pub const GUIDE_INDEX: &str = "index.html";

/// Opens a page in the user's browser.
pub trait BrowserLauncher {
    /// Start the browser on `page`; `Err` carries a short reason.
    fn open(&self, page: &Path) -> Result<(), String>;
}

/// The real launcher: runs the OS's "open this file" command (`browser_command`).
pub struct SystemBrowser;

impl BrowserLauncher for SystemBrowser {
    fn open(&self, _page: &Path) -> Result<(), String> {
        unimplemented!("m8-guide-help")
    }
}

/// Where the guide's index page may be, best first: installed layout (`guide/index.html`
/// next to the executable, when `exe_dir` is known), then the dev build folder
/// `app/target/guide/index.html` (found from this crate's manifest folder).
pub fn guide_candidates(_exe_dir: Option<&Path>) -> Vec<PathBuf> {
    unimplemented!("m8-guide-help")
}

/// The first candidate for which `exists` is true.
pub fn find_guide(_candidates: &[PathBuf], _exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    unimplemented!("m8-guide-help")
}

/// The program and arguments that open `page` in the default browser on `os`:
/// `xdg-open PAGE` (Linux), `open PAGE` (Mac), `cmd /C start "" PAGE` (Windows).
pub fn browser_command(_os: Os, _page: &Path) -> (String, Vec<OsString>) {
    unimplemented!("m8-guide-help")
}

/// Finds the guide and opens it; returns the status-bar message for the outcome.
pub fn open_guide(
    _candidates: &[PathBuf],
    _exists: impl Fn(&Path) -> bool,
    _launcher: &dyn BrowserLauncher,
) -> String {
    unimplemented!("m8-guide-help")
}
