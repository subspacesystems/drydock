// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! An opened model file: a DAE import or a native `.modeler` file.
//!
//! Behaviour: `docs/specs/m0-document.md`, extended by `docs/specs/m1-app-native.md`.

use std::path::{Path, PathBuf};

use modeler_dae::{FlatScene, flatten, import_file, kernel_scene, to_kernel};
use modeler_kernel::format::{self, FileError, ReadErrorKind};
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
        let is_native = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("modeler"));
        if is_native {
            let model = format::load(path).map_err(|err| file_format_error(path, err))?;
            let scene = kernel_scene(&model);
            let stats = stats_from_scene(&scene, Some(&model));
            return Ok(Document {
                path: path.to_path_buf(),
                kind: FileKind::Native,
                kernel: Some(model),
                kernel_error: None,
                scene,
                stats,
            });
        }

        let model = import_file(path).map_err(|err| file_error(path, format!("{err:?}")))?;
        let scene = flatten(&model);
        let kernel = match to_kernel(&model) {
            Ok(kernel) => Some(kernel),
            Err(err) => {
                let message = file_error(path, format!("{err:?}"));
                return Ok(Document {
                    path: path.to_path_buf(),
                    kind: FileKind::Dae,
                    kernel: None,
                    kernel_error: Some(message),
                    stats: stats_from_scene(&scene, None),
                    scene,
                });
            }
        };
        let stats = stats_from_scene(&scene, kernel.as_ref());
        Ok(Document {
            path: path.to_path_buf(),
            kind: FileKind::Dae,
            kernel,
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
        let target = native_path(path);
        let Some(kernel) = &self.kernel else {
            return Err(file_error(&self.path, "nothing to save".to_owned()));
        };
        format::save(kernel, &target).map_err(|err| file_format_error(&target, err))?;
        self.path = target.clone();
        self.kind = FileKind::Native;
        Ok(target)
    }

    /// Write the kernel model back to the open native file. Returns the path written.
    pub fn save(&mut self) -> Result<PathBuf, String> {
        if self.kind == FileKind::Dae {
            return Err(file_error(
                &self.path,
                "not a native file; use Save As".to_owned(),
            ));
        }
        let Some(kernel) = &self.kernel else {
            return Err(file_error(&self.path, "nothing to save".to_owned()));
        };
        let path = self.path.clone();
        format::save(kernel, &path).map_err(|err| file_format_error(&path, err))?;
        Ok(path)
    }
}

fn stats_from_scene(scene: &FlatScene, kernel: Option<&km::Model>) -> Stats {
    let size_m = match scene.bounds {
        Some(bounds) => [
            (bounds.max[0] - bounds.min[0]) as f64,
            (bounds.max[1] - bounds.min[1]) as f64,
            (bounds.max[2] - bounds.min[2]) as f64,
        ],
        None => [0.0, 0.0, 0.0],
    };
    let (faces, edges) = match kernel {
        Some(model) => {
            let mut faces = model.root().geometry().face_count();
            let mut edges = model.root().geometry().edge_count();
            for (id, _) in model.definitions() {
                if let Some(body) = model.body(km::BodyId::Definition(id)) {
                    faces += body.geometry().face_count();
                    edges += body.geometry().edge_count();
                }
            }
            (faces, edges)
        }
        None => (0, 0),
    };
    Stats {
        triangles: scene.triangle_count(),
        lines: scene.line_count(),
        materials: scene.materials.len(),
        size_m,
        faces,
        edges,
    }
}

/// The native file name for a save target: `.modeler` kept, `.dae` replaced, anything
/// else gets `.modeler` appended.
pub fn native_path(path: &Path) -> PathBuf {
    match path.extension() {
        Some(ext) if ext.eq_ignore_ascii_case("modeler") => path.to_path_buf(),
        Some(ext) if ext.eq_ignore_ascii_case("dae") => path.with_extension("modeler"),
        _ => match path.file_name() {
            Some(name) => {
                let mut name = name.to_os_string();
                name.push(".modeler");
                path.with_file_name(name)
            }
            None => path.to_path_buf(),
        },
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

fn file_format_error(path: &Path, error: FileError) -> String {
    let reason = match error {
        FileError::Io { message, .. } => message,
        FileError::NotUtf8 => "not valid UTF-8 text".to_owned(),
        FileError::Read(read) => format!(
            "line {}: {}",
            read.line,
            match read.kind {
                ReadErrorKind::NotAModelFile => "not a modeler-model file",
                ReadErrorKind::UnsupportedVersion(_) => "unsupported format version",
                ReadErrorKind::UnknownRecord => "unknown record",
                ReadErrorKind::BadField => "bad field",
                ReadErrorKind::NoBody => "record outside a body",
                ReadErrorKind::DuplicateBody => "duplicate body",
                ReadErrorKind::Rejected(_) => "the model rejected the record",
                ReadErrorKind::MissingEnd => "missing end record",
                ReadErrorKind::TrailingData => "data after end",
            }
        ),
        FileError::Write(_) => "the model cannot be written".to_owned(),
    };
    file_error(path, reason)
}
