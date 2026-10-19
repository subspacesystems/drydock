// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! An opened model file: a DAE import or a native `.modeler` file.
//!
//! Behaviour: `docs/specs/m0-document.md`, extended by `docs/specs/m1-app-native.md`.

use std::path::{Path, PathBuf};

use modeler_dae::{FlatScene, flatten, import_file};
use modeler_kernel::model as km;

/// Numbers for the status bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub triangles: usize,
    pub lines: usize,
    pub materials: usize,
    /// Model size along X, Y, Z in meters (zero for an empty model).
    pub size_m: [f64; 3],
    /// Kernel faces and edges over the root and every definition body (each definition
    /// counted once, however many instances it has). Zero without a kernel model.
    pub faces: usize,
    pub edges: usize,
}

/// What kind of file the document was read from or last saved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Dae,
    Native,
}

pub struct Document {
    pub path: PathBuf,
    pub kind: FileKind,
    /// The kernel model: the native file's model, or `to_kernel` of the DAE import.
    /// `None` only when a DAE couldn't be converted; `kernel_error` then says why.
    pub kernel: Option<km::Model>,
    pub kernel_error: Option<String>,
    /// What the viewport draws.
    pub scene: FlatScene,
    pub stats: Stats,
}

impl Document {
    /// Open a `.dae` or `.modeler` file. Safe to call from a background thread. The
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
            faces: 0,
            edges: 0,
        };
        Ok(Document {
            path: path.to_path_buf(),
            kind: FileKind::Dae,
            kernel: None,
            kernel_error: None,
            scene,
            stats,
        })
    }

    /// The window title: "<file name> - modeler".
    pub fn title(&self) -> String {
        format!("{} - modeler", file_name(&self.path))
    }

    /// Write the kernel model to `native_path(path)`; on success the document is that
    /// native file. Returns the path written.
    pub fn save_as(&mut self, path: &Path) -> Result<PathBuf, String> {
        let _ = path;
        unimplemented!()
    }

    /// Write the kernel model back to the open native file. Returns the path written.
    pub fn save(&mut self) -> Result<PathBuf, String> {
        unimplemented!()
    }
}

/// The native file name for a save target: `.modeler` kept, `.dae` replaced, anything
/// else gets `.modeler` appended.
pub fn native_path(path: &Path) -> PathBuf {
    let _ = path;
    unimplemented!()
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
