// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! An opened model file.

use std::path::{Path, PathBuf};

use modeler_dae::{FlatScene, Model};

/// Numbers for the status bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub triangles: usize,
    pub lines: usize,
    pub materials: usize,
    /// Model size along X, Y, Z in meters (zero for an empty model).
    pub size_m: [f64; 3],
}

pub struct Document {
    pub path: PathBuf,
    pub model: Model,
    pub scene: FlatScene,
    pub stats: Stats,
}

impl Document {
    /// Import and flatten a `.dae` file. Safe to call from a background thread. The
    /// error is a one-line message for the user that names the file.
    pub fn open(path: &Path) -> Result<Document, String> {
        let _ = path;
        unimplemented!("m0-app")
    }

    /// The window title: "<file name> - modeler".
    pub fn title(&self) -> String {
        unimplemented!("m0-app")
    }
}

/// Status bar text for a size, e.g. "4.115 x 6.706 x 2.438 m".
pub fn format_size(size_m: [f64; 3]) -> String {
    let _ = size_m;
    unimplemented!("m0-app")
}
