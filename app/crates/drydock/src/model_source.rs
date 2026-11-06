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

use std::path::{Path, PathBuf};

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
        let _ = path;
        unimplemented!("m10-model-source-trait")
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
        unimplemented!("m10-model-source-trait")
    }

    /// Read the bytes into a kernel model by `format`. `unit` applies to STL only. With an
    /// `origin` the reader opens that file; without one the bytes are written to a fresh
    /// folder under the system temp dir under `file_name`, read, and the folder removed
    /// again. `Err` is a one-line message that names the file.
    pub fn into_model(&self, unit: MeshUnit) -> Result<Model, String> {
        let _ = unit;
        unimplemented!("m10-model-source-trait")
    }
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
        unimplemented!("m10-model-source-trait")
    }

    fn list(&self, query: &str) -> Result<Vec<SourceEntry>, String> {
        let _ = query;
        unimplemented!("m10-model-source-trait")
    }

    fn fetch(&self, entry: &SourceEntry) -> Result<SourceModel, String> {
        let _ = entry;
        unimplemented!("m10-model-source-trait")
    }
}
