// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The native model file: a versioned text format, read and written exactly.
//!
//! Format: `docs/format.md`. Behaviour: `docs/specs/m1-format.md`. The reader builds the
//! model through `Model::apply`, so a file can only name things that already exist.

use std::path::Path;

use crate::model::{Model, ModelError};

/// The version this code writes, and the newest it reads.
pub const FORMAT_VERSION: u32 = 1;

/// Why a model can't be written. Nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// A coordinate, color or transform entry is NaN or infinite.
    NonFinite,
}

/// Why a file's text can't be read, and the 1-based line where that was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
    pub line: usize,
    pub kind: ReadErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadErrorKind {
    /// The first line isn't `modeler-model <version>`.
    NotAModelFile,
    /// The header names a version this reader doesn't know.
    UnsupportedVersion(u32),
    /// The line's first token isn't a known record name.
    UnknownRecord,
    /// Wrong number of tokens, a malformed number, string or edge use, or a non-finite number.
    BadField,
    /// A vertex, edge, face, paint or instance record before any `body` record.
    NoBody,
    /// A second `body` record for the same body.
    DuplicateBody,
    /// The record is well formed but the model refused it.
    Rejected(ModelError),
    /// The text ended without an `end` record.
    MissingEnd,
    /// Something other than blank lines after `end`.
    TrailingData,
}

/// Why `save` or `load` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileError {
    Io {
        kind: std::io::ErrorKind,
        message: String,
    },
    /// The file isn't valid UTF-8.
    NotUtf8,
    Read(ReadError),
    Write(WriteError),
}

/// The model as file text.
pub fn to_text(model: &Model) -> Result<String, WriteError> {
    let _ = model;
    unimplemented!("m1-format")
}

/// Read a model from file text.
pub fn from_text(text: &str) -> Result<Model, ReadError> {
    let _ = text;
    unimplemented!("m1-format")
}

/// Write the model to `path` without ever leaving a partial file under that name.
pub fn save(model: &Model, path: &Path) -> Result<(), FileError> {
    let _ = (model, path);
    unimplemented!("m1-format")
}

/// Read a model file.
pub fn load(path: &Path) -> Result<Model, FileError> {
    let _ = path;
    unimplemented!("m1-format")
}
