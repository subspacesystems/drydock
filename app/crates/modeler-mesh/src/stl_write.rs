// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Binary STL writer (`docs/specs/m7-export-stl.md`).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::print_mesh::{PrintObject, Triangle};

/// Why nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StlWriteError {
    /// No object has a triangle.
    NothingToWrite,
    /// A file or folder couldn't be written; the system's message.
    Io(String),
}

impl fmt::Display for StlWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StlWriteError::NothingToWrite => write!(f, "nothing to export"),
            StlWriteError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for StlWriteError {}

/// The bytes of one binary STL: 80-byte header (`"modeler STL: "` plus `name`, cut at 80
/// bytes on a character boundary, zero padded), u32 count, then 50-byte records (unit
/// normal from the winding, three corners as f32, attribute 0). Values are written as
/// given (the print mesh is already in mm). A degenerate triangle gets a zero normal.
pub fn encode(name: &str, triangles: &[Triangle]) -> Vec<u8> {
    let _ = (name, triangles);
    unimplemented!("m7-export-stl")
}

/// A file stem made safe for any file system: letters, digits, `-`, `_` and spaces are
/// kept, everything else becomes `_`; leading and trailing spaces are trimmed; at most
/// 100 characters; an empty result is `"part"`.
pub fn sanitize_name(name: &str) -> String {
    let _ = name;
    unimplemented!("m7-export-stl")
}

/// One `<name>.stl` per object that has triangles, inside `dir` (created if missing).
/// Names go through [`sanitize_name`]; a stem already used (compared ignoring case) gets
/// `-2`, `-3`, ... and the result never repeats a stem. Returns the paths in object order.
/// Objects without triangles are skipped. `NothingToWrite` (and no file) when none has any.
pub fn write_separate(objects: &[PrintObject], dir: &Path) -> Result<Vec<PathBuf>, StlWriteError> {
    let _ = (objects, dir);
    unimplemented!("m7-export-stl")
}

/// All objects' triangles in one binary STL at `path`, in object order; the header names
/// the first object that has triangles. `NothingToWrite` (and no file) when none has any.
pub fn write_combined(objects: &[PrintObject], path: &Path) -> Result<(), StlWriteError> {
    let _ = (objects, path);
    unimplemented!("m7-export-stl")
}
