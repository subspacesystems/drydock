// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! An opened model file: a DAE import or a native `.modeler` file.
//!
//! Behaviour: `docs/specs/m0-document.md`, extended by `docs/specs/m1-app-native.md`.

use std::path::{Path, PathBuf};

use modeler_dae::{FlatScene, RestView, flatten, import_file, kernel_scene, to_kernel};
use modeler_kernel::editpath::EditPath;
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
    /// True for a new model that has no file yet (`Document::new_empty`) until it is saved.
    pub untitled: bool,
    /// True after `refresh` until the next successful save.
    pub modified: bool,
    /// `UndoStack::state` of the editor's stack when the file was last opened or saved.
    pub saved_state: u64,
}

impl Document {
    /// Open a `.dae` or `.modeler` file. Safe to call from a background thread. The
    /// error is a one-line message for the user that names the file.
    pub fn open(path: &Path) -> Result<Document, String> {
        let is_native = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("modeler"));
        if is_native {
            let mut model = format::load(path).map_err(|err| file_format_error(path, err))?;
            let folder = path.parent().unwrap_or(Path::new(""));
            map_texture_paths(&mut model, |texture| resolve_texture_path(texture, folder));
            let scene = kernel_scene(&model);
            let stats = stats_from_scene(&scene, Some(&model));
            return Ok(Document {
                path: path.to_path_buf(),
                kind: FileKind::Native,
                kernel: Some(model),
                kernel_error: None,
                scene,
                stats,
                untitled: false,
                modified: false,
                saved_state: 0,
            });
        }

        let model = import_file(path).map_err(|err| file_error(path, err.to_string()))?;
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
                    untitled: false,
                    modified: false,
                    saved_state: 0,
                });
            }
        };
        let mut kernel = kernel;
        if let Some(model) = kernel.as_mut() {
            map_texture_paths(model, |texture| {
                resolve_texture_path(texture, Path::new(""))
            });
        }
        let stats = stats_from_scene(&scene, kernel.as_ref());
        Ok(Document {
            path: path.to_path_buf(),
            kind: FileKind::Dae,
            kernel,
            kernel_error: None,
            scene,
            stats,
            untitled: false,
            modified: false,
            saved_state: 0,
        })
    }

    /// A new, empty native model named `untitled.modeler` (not on disk).
    pub fn new_empty() -> Document {
        Document {
            path: PathBuf::from("untitled.modeler"),
            kind: FileKind::Native,
            kernel: Some(km::Model::new()),
            kernel_error: None,
            scene: FlatScene::default(),
            stats: Stats {
                triangles: 0,
                lines: 0,
                materials: 0,
                size_m: [0.0; 3],
                faces: 0,
                edges: 0,
            },
            untitled: true,
            modified: false,
            saved_state: 0,
        }
    }

    /// Rebuild `scene` and `stats` from `kernel` after an edit and set `modified`.
    /// Does nothing without a kernel model.
    pub fn refresh(&mut self) {
        let Some(kernel) = &self.kernel else {
            return;
        };
        self.scene = kernel_scene(kernel);
        self.stats = stats_from_scene(&self.scene, Some(kernel));
        self.modified = true;
    }

    /// Like `refresh`, but draws the scene for editing inside `path`: the rest of the model
    /// as `rest` says (`modeler_dae::kernel_scene_editing`). At the root path it equals
    /// `refresh`. Behaviour: `docs/specs/m4-app-edit-view.md`.
    pub fn refresh_editing(&mut self, path: &EditPath, rest: RestView) {
        let _ = (path, rest);
        unimplemented!("m4-app-edit-view")
    }

    /// Record that the file on disk matches the model at undo-stack state `state`
    /// (`UndoStack::state`): sets `saved_state` and clears `modified`.
    pub fn mark_saved_at(&mut self, state: u64) {
        self.saved_state = state;
        self.modified = false;
    }

    /// Set `modified` to whether `state` differs from `saved_state`. The app calls this
    /// after an undo or redo instead of `refresh` setting it blindly.
    pub fn sync_modified(&mut self, state: u64) {
        self.modified = state != self.saved_state;
    }

    /// The window title: "<file name> - modeler", with a `*` after the name while
    /// the document is modified.
    pub fn title(&self) -> String {
        let star = if self.modified { "*" } else { "" };
        format!("{}{star} - modeler", file_name(&self.path))
    }

    /// Write the kernel model to `native_path(path)`; on success the document is that
    /// native file. Returns the path written.
    pub fn save_as(&mut self, path: &Path) -> Result<PathBuf, String> {
        let target = native_path(path);
        let Some(kernel) = &self.kernel else {
            return Err(file_error(&self.path, "nothing to save".to_owned()));
        };
        let folder = target.parent().unwrap_or(Path::new(""));
        let model = model_for_file(kernel, folder);
        format::save(&model, &target).map_err(|err| file_format_error(&target, err))?;
        self.path = target.clone();
        self.kind = FileKind::Native;
        self.untitled = false;
        self.modified = false;
        Ok(target)
    }

    /// Write the kernel model back to the open native file. Returns the path written.
    pub fn save(&mut self) -> Result<PathBuf, String> {
        if self.untitled {
            return Err(file_error(
                &self.path,
                "is a new model with no file; use Save As".to_owned(),
            ));
        }
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
        let folder = path.parent().unwrap_or(Path::new(""));
        let model = model_for_file(kernel, folder);
        format::save(&model, &path).map_err(|err| file_format_error(&path, err))?;
        self.untitled = false;
        self.modified = false;
        Ok(path)
    }
}

fn model_for_file(kernel: &km::Model, dir: &Path) -> km::Model {
    let mut model = kernel.clone();
    map_texture_paths(&mut model, |texture| texture_path_for_file(texture, dir));
    model
}

fn map_texture_paths(model: &mut km::Model, map: impl Fn(&str) -> String) {
    let updates: Vec<_> = model
        .materials()
        .filter_map(|(id, material)| {
            let texture = material.texture.as_ref()?;
            let mut to = material.clone();
            to.texture.as_mut().unwrap().path = map(&texture.path);
            Some((id, material.clone(), to))
        })
        .collect();
    for (id, from, to) in updates {
        let _ = model.apply(&km::ModelRecord::SetMaterial { id, from, to });
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
            let mut seen = std::collections::BTreeSet::new();
            let mut pending: Vec<_> = model
                .root()
                .instances()
                .map(|(_, instance)| instance.definition)
                .collect();
            while let Some(id) = pending.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(body) = model.body(km::BodyId::Definition(id)) else {
                    continue;
                };
                faces += body.geometry().face_count();
                edges += body.geometry().edge_count();
                pending.extend(body.instances().map(|(_, instance)| instance.definition));
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

/// A texture path as the document keeps it in memory: `texture` as is when it is absolute,
/// else joined onto `dir` (the folder of the file it was read from), as one string.
pub fn resolve_texture_path(texture: &str, dir: &Path) -> String {
    let joined = if Path::new(texture).is_absolute() {
        PathBuf::from(texture)
    } else {
        dir.join(texture)
    };
    collapse_path(&joined).to_string_lossy().into_owned()
}

/// `path` with `.` removed and `a/..` pairs cancelled, textually (nothing touches the disk
/// and a relative path stays relative).
fn collapse_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let normal = matches!(
                    out.components().next_back(),
                    Some(std::path::Component::Normal(_))
                );
                if normal {
                    out.pop();
                } else if !out.has_root() {
                    out.push(part.as_os_str());
                }
            }
            _ => out.push(part.as_os_str()),
        }
    }
    out
}

/// The text to write for an in-memory texture path in a model file saved into `dir`: the
/// path from `dir` to the image, with `/` separators and no `.` or `..` that cancel out,
/// or the absolute path when the two have no common root. Both are made absolute first
/// (relative ones against the current directory); nothing touches the disk.
pub fn texture_path_for_file(texture: &str, dir: &Path) -> String {
    let to = normalize_path(Path::new(texture));
    let from = normalize_path(dir);
    let from_parts: Vec<_> = from.components().collect();
    let to_parts: Vec<_> = to.components().collect();
    let common = from_parts
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    if common == 0 {
        return to.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = Vec::new();
    for _ in common..from_parts.len() {
        parts.push("..".to_owned());
    }
    for part in &to_parts[common..] {
        match part {
            std::path::Component::Normal(value) => {
                parts.push(value.to_string_lossy().into_owned());
            }
            std::path::Component::ParentDir => parts.push("..".to_owned()),
            std::path::Component::CurDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {}
        }
    }
    parts.join("/")
}

fn normalize_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let normal = matches!(
                    out.components().next_back(),
                    Some(std::path::Component::Normal(_))
                );
                if normal {
                    out.pop();
                } else if !out.has_root() {
                    out.push(part.as_os_str());
                }
            }
            _ => out.push(part.as_os_str()),
        }
    }
    out
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
                ReadErrorKind::NotAModelFile => "not a modeler-model file".to_owned(),
                ReadErrorKind::UnsupportedVersion(version) => {
                    format!("unsupported format version {version}")
                }
                ReadErrorKind::UnknownRecord => "unknown record".to_owned(),
                ReadErrorKind::BadField => "bad field".to_owned(),
                ReadErrorKind::NoBody => "record outside a body".to_owned(),
                ReadErrorKind::DuplicateBody => "duplicate body".to_owned(),
                ReadErrorKind::Rejected(err) => format!("the model rejected the record ({err:?})"),
                ReadErrorKind::MissingEnd => "missing end record".to_owned(),
                ReadErrorKind::TrailingData => "data after end".to_owned(),
            }
        ),
        FileError::Write(_) => "the model cannot be written".to_owned(),
    };
    file_error(path, reason)
}
