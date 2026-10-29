// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! STL reader, binary and ASCII (`docs/specs/m7-import-stl.md`).

use std::fmt;

use crate::MeshObject;

/// Why bytes couldn't be read as STL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StlError {
    /// Neither a binary STL of the right length nor text starting with `solid`.
    NotStl,
    /// A binary STL whose length doesn't match its triangle count: `expected` is
    /// `84 + 50 * count`, `actual` the byte count.
    Truncated { expected: usize, actual: usize },
    /// An ASCII STL that breaks the grammar; `line` is 1-based.
    Syntax { line: usize, message: String },
}

impl fmt::Display for StlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f;
        unimplemented!()
    }
}

impl std::error::Error for StlError {}

/// Reads binary or ASCII STL. Solids without a name get `default_name`.
pub fn read(bytes: &[u8], default_name: &str) -> Result<Vec<MeshObject>, StlError> {
    let _ = (bytes, default_name);
    unimplemented!()
}
