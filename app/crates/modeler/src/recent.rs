// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The File menu's recent files: a short list, most recent first, kept in a text file in
//! the user's config folder between runs.
//!
//! Behaviour: `docs/specs/m4-ui-menus.md`.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// The longest the list gets; adding to a full list drops the oldest entry.
pub const MAX_RECENT: usize = 8;

/// The recent files, most recent first, no duplicates, at most `MAX_RECENT`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentFiles {
    paths: Vec<PathBuf>,
}

impl RecentFiles {
    /// The files, most recent first.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Put `path` (made absolute) at the front, dropping an older entry for the same path
    /// and the oldest entry when the list is over `MAX_RECENT`.
    pub fn add(&mut self, path: &Path) {
        let _ = path;
        unimplemented!("m4-ui-menus")
    }

    /// Drop the entry for `path` (made absolute), if there is one.
    pub fn remove(&mut self, path: &Path) {
        let _ = path;
        unimplemented!("m4-ui-menus")
    }

    /// The list from the file's text: one path per line.
    pub fn parse(text: &str) -> RecentFiles {
        let _ = text;
        unimplemented!("m4-ui-menus")
    }

    /// The file's text for this list: each path on its own line.
    pub fn to_text(&self) -> String {
        unimplemented!("m4-ui-menus")
    }

    /// The list stored in `file`; empty when the file is missing or can't be read.
    pub fn load(file: &Path) -> RecentFiles {
        let _ = file;
        unimplemented!("m4-ui-menus")
    }

    /// Store the list in `file`, creating its folder when needed.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let _ = file;
        unimplemented!("m4-ui-menus")
    }
}

/// The app's config folder, from environment variables read through `var`.
pub fn config_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let _ = var;
    unimplemented!("m4-ui-menus")
}

/// The file the recent list is kept in: `recent-files` in `config_dir`.
pub fn recent_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    config_dir(var).map(|dir| dir.join("recent-files"))
}
