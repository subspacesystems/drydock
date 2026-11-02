// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Where the app keeps its settings on each OS, worked out from environment variables read
//! through a function, so tests run on any OS.
//!
//! Behaviour: `docs/specs/m8-win-exe.md`.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::look::Os;

/// The app's config folder on `os`, from the environment variables read through `var`.
/// Order and folder names are in the spec; `recent::config_dir` calls this with `Os::current()`.
pub fn config_dir_on(os: Os, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let non_empty = |name: &str| var(name).filter(|value| !value.is_empty());
    if let Some(dir) = non_empty("DRYDOCK_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    match os {
        Os::Windows => {
            if let Some(dir) = non_empty("APPDATA") {
                return Some(PathBuf::from(dir).join("Drydock"));
            }
            non_empty("USERPROFILE").map(|home| {
                PathBuf::from(home)
                    .join("AppData")
                    .join("Roaming")
                    .join("Drydock")
            })
        }
        Os::Linux | Os::Mac => {
            if let Some(dir) = non_empty("XDG_CONFIG_HOME") {
                let dir = PathBuf::from(dir);
                if dir.is_absolute() {
                    return Some(dir.join("drydock"));
                }
            }
            non_empty("HOME").map(|home| PathBuf::from(home).join(".config").join("drydock"))
        }
    }
}
