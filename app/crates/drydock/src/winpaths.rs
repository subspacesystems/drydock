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
    let _ = (os, var);
    unimplemented!("m8-win-exe")
}
