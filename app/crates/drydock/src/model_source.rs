// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Where models come from: the [`ModelSource`] interface and [`LocalFileSource`].
//!
//! A source lists the models it has for a search text and fetches one of them as bytes plus
//! a file name and format. The app turns the result into a kernel model with
//! [`SourceModel::into_model`], which picks the reader by format (DAE and native files through
//! `components_panel::read_component_source`, STL and 3MF through `mesh_import::read_mesh`).
//!
//! The interface is synchronous. A source that talks to a network (an online library added
//! later) must be called from a worker thread, never from the UI thread; that is why
//! `ModelSource` requires `Send + Sync`.
//!
//! Searching: `list` matches the search text against titles and tags only. A source must not
//! search inside the contents of models.
//! Behaviour: `docs/specs/m10-model-source-trait.md`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use drydock_kernel::model::Model;

use crate::mesh_import::MeshUnit;

/// A model file format a source can deliver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormat {
    /// COLLADA, `.dae`.
    Dae,
    /// Binary or ASCII STL, `.stl`.
    Stl,
    /// 3MF, `.3mf`.
    ThreeMf,
    /// The native format, `.drydock` or the older `.modeler`.
    Native,
}

impl SourceFormat {
    /// The format of `path` by its extension (`dae`, `stl`, `3mf`, `drydock`, `modeler`; any
    /// letter case); `None` for anything else.
    pub fn from_path(path: &Path) -> Option<SourceFormat> {
        let ext = path.extension()?;
        if ext.eq_ignore_ascii_case("dae") {
            Some(SourceFormat::Dae)
        } else if ext.eq_ignore_ascii_case("stl") {
            Some(SourceFormat::Stl)
        } else if ext.eq_ignore_ascii_case("3mf") {
            Some(SourceFormat::ThreeMf)
        } else if crate::document::is_native_file(path) {
            Some(SourceFormat::Native)
        } else {
            None
        }
    }
}

/// One model a source offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEntry {
    /// Identifies the model within its source; for [`LocalFileSource`] the file's path.
    pub id: String,
    /// What to show the user; for [`LocalFileSource`] the file name without its extension.
    pub title: String,
    /// The format, when the source knows it before fetching.
    pub format: Option<SourceFormat>,
}

/// A fetched model: the file's bytes with its name and format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceModel {
    /// The file name with its extension, no folders.
    pub file_name: String,
    pub bytes: Vec<u8>,
    pub format: SourceFormat,
    /// The file on disk the bytes came from, when there is one (a local source). Readers use
    /// it so textures next to the file are found.
    pub origin: Option<PathBuf>,
}

impl SourceModel {
    /// The file name without its extension: the name for an imported definition.
    pub fn title(&self) -> String {
        let path = Path::new(&self.file_name);
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.file_name.clone())
    }

    /// Read the bytes into a kernel model by `format`. `unit` applies to STL only. With an
    /// `origin` the reader opens that file; without one the bytes are written to a fresh
    /// folder under the system temp dir (named `drydock-source-…`) under `file_name`, read, and the folder removed
    /// again. `Err` is a one-line message that names the file.
    pub fn into_model(&self, unit: MeshUnit) -> Result<Model, String> {
        if let Some(origin) = &self.origin {
            return self.read(origin, unit);
        }
        let dir = fresh_source_dir();
        let component = Path::new(&self.file_name)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.file_name.clone());
        let path = dir.join(&component);
        let result = (|| -> Result<Model, String> {
            fs::write(&path, &self.bytes)
                .map_err(|error| one_line(error, self.file_name.clone()))?;
            self.read(&path, unit)
        })();
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir(dir.parent().unwrap_or(&dir));
        result
    }

    /// `Err` for `path` with the file name guaranteed in the message.
    fn read_err(error: String, path: &Path) -> String {
        let name = file_name_of(path);
        let line = error.replace(['\r', '\n'], " ");
        if line.contains(&name) {
            line
        } else {
            format!("{name}: {line}")
        }
    }

    /// Dispatch to the reader for `format`; `unit` applies to STL only.
    fn read(&self, path: &Path, unit: MeshUnit) -> Result<Model, String> {
        match self.format {
            SourceFormat::Dae | SourceFormat::Native => {
                Ok(crate::components_panel::read_component_source(path)
                    .map(|(model, _)| model)
                    .map_err(|error| Self::read_err(error, path))?)
            }
            SourceFormat::Stl | SourceFormat::ThreeMf => crate::mesh_import::read_mesh(path, unit)
                .map_err(|error| Self::read_err(error, path)),
        }
    }
}

static FRESH_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh empty folder under the system temp dir, named `drydock-source-<pid>-<n>`.
fn fresh_source_dir() -> PathBuf {
    let counter = FRESH_COUNTER.fetch_add(1, Ordering::SeqCst);
    let parent = std::env::temp_dir().join(format!("drydock-tmp-{}-{counter}", std::process::id()));
    let dir = parent.join(format!("drydock-source-{}-{counter}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A place models can be listed and fetched from. Synchronous; see the module docs.
pub trait ModelSource: Send + Sync {
    /// A short name for the source, for a heading or a tab.
    fn name(&self) -> &str;

    /// The models matching `query`: a case-insensitive substring of the title (or of a tag,
    /// for sources that have tags). An empty or blank `query` lists everything. `Err` is a
    /// one-line message for the user.
    fn list(&self, query: &str) -> Result<Vec<SourceEntry>, String>;

    /// Fetch the model `entry` stands for. `Err` is a one-line message that names the model.
    fn fetch(&self, entry: &SourceEntry) -> Result<SourceModel, String>;
}

/// The models in one folder on disk.
#[derive(Clone, Debug)]
pub struct LocalFileSource {
    dir: PathBuf,
}

impl LocalFileSource {
    /// A source for the files directly in `dir` (subfolders are not searched).
    pub fn new(dir: impl Into<PathBuf>) -> LocalFileSource {
        LocalFileSource { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl ModelSource for LocalFileSource {
    fn name(&self) -> &str {
        "Local files"
    }

    fn list(&self, query: &str) -> Result<Vec<SourceEntry>, String> {
        let read_dir =
            fs::read_dir(&self.dir).map_err(|error| one_line(error, self.dir.to_string_lossy()))?;
        let needle = query.trim().to_lowercase();
        let mut entries = Vec::new();
        for file in read_dir.flatten() {
            let Ok(file_type) = file.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let path = file.path();
            let Some(format) = SourceFormat::from_path(&path) else {
                continue;
            };
            let name = file_name_of(&path);
            let title = title_of(&name);
            if !needle.is_empty() && !title.to_lowercase().contains(&needle) {
                continue;
            }
            entries.push(SourceEntry {
                id: path.to_string_lossy().into_owned(),
                title,
                format: Some(format),
            });
        }
        entries.sort_by(|a, b| {
            a.title
                .to_lowercase()
                .cmp(&b.title.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(entries)
    }

    fn fetch(&self, entry: &SourceEntry) -> Result<SourceModel, String> {
        let path = PathBuf::from(&entry.id);
        let name = file_name_of(&path);
        let Some(format) = SourceFormat::from_path(&path) else {
            return Err(one_line("not a model file", name));
        };
        let bytes = fs::read(&path).map_err(|error| one_line(error, name.clone()))?;
        Ok(SourceModel {
            file_name: name,
            bytes,
            format,
            origin: Some(path),
        })
    }
}

/// The entry for the file at `path` in the source [`local_source_for`] gives: id the path,
/// title the file name without its extension, format from the extension (`None` if unknown).
pub fn entry_for_path(path: &Path) -> SourceEntry {
    SourceEntry {
        id: path.to_string_lossy().into_owned(),
        title: title_of(&file_name_of(path)),
        format: SourceFormat::from_path(path),
    }
}

/// A [`LocalFileSource`] for the folder `path` is in (`.` when it has none).
pub fn local_source_for(path: &Path) -> LocalFileSource {
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    LocalFileSource::new(dir)
}

/// Fetch `entry` from `source` and read it as the source of an imported component: its kernel
/// model and the definition name (`SourceModel::title`). Accepts every [`SourceFormat`]; STL
/// is read in `MeshUnit::default()`. A fetch error is returned unchanged; a read error names
/// the file, on one line.
pub fn fetch_component(
    source: &dyn ModelSource,
    entry: &SourceEntry,
) -> Result<(Model, String), String> {
    let source_model = source.fetch(entry)?;
    let name = source_model.title();
    let model = source_model.into_model(MeshUnit::default())?;
    Ok((model, name))
}

/// Fetch `entry` from `source` and read it as a mesh (STL in `unit`, or 3MF). A fetched model
/// of any other format is `Err("<file name>: not an STL or 3MF file")` without reading it. A
/// fetch error is returned unchanged.
pub fn fetch_meshes(
    source: &dyn ModelSource,
    entry: &SourceEntry,
    unit: MeshUnit,
) -> Result<Model, String> {
    let source_model = source.fetch(entry)?;
    if !matches!(
        source_model.format,
        SourceFormat::Stl | SourceFormat::ThreeMf
    ) {
        return Err(format!(
            "{}: not an STL or 3MF file",
            source_model.file_name
        ));
    }
    source_model.into_model(unit)
}

/// [`fetch_component`], then `Editor::import_component` into `model`; returns the definition.
/// On any `Err` the model is unchanged.
pub fn import_component_from(
    editor: &mut crate::editor::Editor,
    model: &mut Model,
    source: &dyn ModelSource,
    entry: &SourceEntry,
) -> Result<drydock_kernel::model::DefinitionId, String> {
    let (source_model, name) = fetch_component(source, entry)?;
    editor.import_component(model, &source_model, &name)
}

/// [`fetch_meshes`], then `Editor::import_groups` into `model`; returns how many instances
/// were added. On any `Err` the model is unchanged.
pub fn import_meshes_from(
    editor: &mut crate::editor::Editor,
    model: &mut Model,
    source: &dyn ModelSource,
    entry: &SourceEntry,
    unit: MeshUnit,
) -> Result<usize, String> {
    let source_model = fetch_meshes(source, entry, unit)?;
    editor.import_groups(model, &source_model)
}

/// The last component of `path`, or its full text if it has none.
fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The file name `name` without its extension, or `name` if it has none.
fn title_of(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// `"{name}: {error}"` on one line.
fn one_line(error: impl std::fmt::Display, name: impl std::fmt::Display) -> String {
    format!("{name}: {error}").replace(['\r', '\n'], " ")
}
