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
    fn open(&self, page: &Path) -> Result<(), String> {
        let (program, args) = browser_command(Os::current(), page);
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Where the guide's index page may be, best first: installed layout (`guide/index.html`
/// next to the executable, when `exe_dir` is known), then the dev build folder
/// `app/target/guide/index.html` (found from this crate's manifest folder).
pub fn guide_candidates(exe_dir: Option<&Path>) -> Vec<PathBuf> {
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guide")
        .join(GUIDE_INDEX);
    match exe_dir {
        Some(dir) => vec![dir.join("guide").join(GUIDE_INDEX), dev],
        None => vec![dev],
    }
}

/// The first candidate for which `exists` is true.
pub fn find_guide(candidates: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    candidates.iter().find(|path| exists(path)).cloned()
}

/// The program and arguments that open `page` in the default browser on `os`:
/// `xdg-open PAGE` (Linux), `open PAGE` (Mac), `cmd /C start "" PAGE` (Windows).
pub fn browser_command(os: Os, page: &Path) -> (String, Vec<OsString>) {
    let page = page.as_os_str().to_owned();
    match os {
        Os::Linux => ("xdg-open".to_owned(), vec![page]),
        Os::Mac => ("open".to_owned(), vec![page]),
        Os::Windows => (
            "cmd".to_owned(),
            vec![
                OsString::from("/C"),
                OsString::from("start"),
                OsString::from(""),
                page,
            ],
        ),
    }
}

/// Finds the guide and opens it; returns the status-bar message for the outcome.
pub fn open_guide(
    candidates: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
    launcher: &dyn BrowserLauncher,
) -> String {
    let Some(page) = find_guide(candidates, exists) else {
        let looked_in = candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return format!("The Drydock User Guide was not found. Looked in: {looked_in}");
    };
    match launcher.open(&page) {
        Ok(()) => "Opened the Drydock User Guide in your browser.".to_owned(),
        Err(reason) => format!("Could not open the Drydock User Guide: {reason}"),
    }
}
