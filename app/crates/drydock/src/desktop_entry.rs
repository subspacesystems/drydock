// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Linux only in practice: the desktop entry and hicolor icons that let the dock and task
//! switcher show Drydock's icon (GNOME matches the window's app id to `<APP_ID>.desktop`).
//! Everything except [`install_for_current_user`] is pure or takes the data directory as an
//! argument, so it is tested on a temp dir and on any OS.
//!
//! Behaviour: `docs/specs/m9-linux-desktop-entry.md`.

use std::path::{Path, PathBuf};

/// Icon sizes written under `icons/hicolor/<n>x<n>/apps/`, in this order. The 256 px icon is
/// `window_icon::ICON_PNG` itself; the others are scaled down from it.
pub const ICON_SIZES: [u32; 3] = [48, 128, 256];

/// One file the app keeps up to date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopFile {
    /// Absolute path: `data_dir` joined with the relative path in the spec.
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// What [`install`] did. Files that were already right are in neither list.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub written: Vec<PathBuf>,
    /// Path and the error text.
    pub failed: Vec<(PathBuf, String)>,
}

/// The user data directory: `xdg_data_home` when it is set, not empty and absolute, else
/// `<home>/.local/share` when `home` is set and not empty, else `None`.
pub fn data_dir_from(xdg_data_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let _ = (xdg_data_home, home);
    unimplemented!("m9-linux-desktop-entry")
}

/// The `Exec` value for the program at `exe`: the path as one double-quoted argument
/// followed by ` %f`, escaped as the spec describes (quoting rules, then the file's
/// string-escape rule, then `%` doubled).
pub fn exec_line(exe: &Path) -> String {
    let _ = exe;
    unimplemented!("m9-linux-desktop-entry")
}

/// The text of `<APP_ID>.desktop` for the program at `exe`.
pub fn desktop_entry_text(exe: &Path) -> String {
    let _ = exe;
    unimplemented!("m9-linux-desktop-entry")
}

/// Every file to keep up to date under `data_dir`, with its wanted bytes: the desktop entry
/// first, then the icons in [`ICON_SIZES`] order.
pub fn files_for(data_dir: &Path, exe: &Path) -> Vec<DesktopFile> {
    let _ = (data_dir, exe);
    unimplemented!("m9-linux-desktop-entry")
}

/// Write each file of [`files_for`] that is missing or whose bytes differ, creating the
/// folders it needs. Never touches other files, never panics, never stops at a failure: the
/// failed file goes in `failed` and the rest are still tried.
pub fn install(data_dir: &Path, exe: &Path) -> Report {
    let _ = (data_dir, exe);
    unimplemented!("m9-linux-desktop-entry")
}

/// What `main` calls at startup on a normal run: on Linux, [`install`] into the directory
/// from [`data_dir_from`] (environment `XDG_DATA_HOME`, `HOME`) for `std::env::current_exe()`,
/// then, only when something was written, best-effort `update-desktop-database` and
/// `gtk-update-icon-cache` (ignored when not installed or failing). Failures are printed to
/// stderr as one line each, never returned. Does nothing on other systems.
pub fn install_for_current_user() {
    unimplemented!("m9-linux-desktop-entry")
}
