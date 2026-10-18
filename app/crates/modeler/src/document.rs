// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! An opened model file.

use std::path::{Path, PathBuf};

use modeler_dae::{FlatScene, Model, flatten, import_file};

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
        let model = import_file(path).map_err(|err| file_error(path, err.to_string()))?;
        let scene = flatten(&model);
        let size_m = match scene.bounds {
            Some(bounds) => [
                (bounds.max[0] - bounds.min[0]) as f64,
                (bounds.max[1] - bounds.min[1]) as f64,
                (bounds.max[2] - bounds.min[2]) as f64,
            ],
            None => [0.0, 0.0, 0.0],
        };
        let stats = Stats {
            triangles: scene.triangle_count(),
            lines: scene.line_count(),
            materials: scene.materials.len(),
            size_m,
        };
        Ok(Document {
            path: path.to_path_buf(),
            model,
            scene,
            stats,
        })
    }

    /// The window title: "<file name> - modeler".
    pub fn title(&self) -> String {
        format!("{} - modeler", file_name(&self.path))
    }
}

/// Status bar text for a size, e.g. "4.115 x 6.706 x 2.438 m".
pub fn format_size(size_m: [f64; 3]) -> String {
    format!("{:.3} x {:.3} x {:.3} m", size_m[0], size_m[1], size_m[2])
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn file_error(path: &Path, reason: String) -> String {
    let reason = reason.replace(['\r', '\n'], " ");
    format!("{}: {reason}", file_name(path))
}
