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
        let path = absolute(path);
        self.paths.retain(|entry| entry != &path);
        self.paths.insert(0, path);
        self.paths.truncate(MAX_RECENT);
    }

    /// Drop the entry for `path` (made absolute), if there is one.
    pub fn remove(&mut self, path: &Path) {
        let path = absolute(path);
        self.paths.retain(|entry| entry != &path);
    }

    /// The list from the file's text: one path per line.
    pub fn parse(text: &str) -> RecentFiles {
        let mut list = RecentFiles::default();
        for line in text.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.is_empty() || list.paths.len() >= MAX_RECENT {
                continue;
            }
            let path = PathBuf::from(line);
            if !list.paths.contains(&path) {
                list.paths.push(path);
            }
        }
        list
    }

    /// The file's text for this list: each path on its own line.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for path in &self.paths {
            if let Some(path) = path.to_str() {
                text.push_str(path);
                text.push('\n');
            }
        }
        text
    }

    /// The list stored in `file`; empty when the file is missing or can't be read.
    pub fn load(file: &Path) -> RecentFiles {
        std::fs::read_to_string(file)
            .map(|text| RecentFiles::parse(&text))
            .unwrap_or_default()
    }

    /// Store the list in `file`, creating its folder when needed.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let folder = file.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(folder)?;
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "recent-files".to_owned());
        let temp = folder.join(format!(".{name}.tmp"));
        std::fs::write(&temp, self.to_text())?;
        std::fs::rename(&temp, file)
    }
}

/// The app's config folder, from environment variables read through `var`.
pub fn config_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::winpaths::config_dir_on(crate::look::Os::current(), var)
}

/// Make `path` absolute, keeping it as given when that fails.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The file the recent list is kept in: `recent-files` in `config_dir`.
pub fn recent_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    config_dir(var).map(|dir| dir.join("recent-files"))
}
