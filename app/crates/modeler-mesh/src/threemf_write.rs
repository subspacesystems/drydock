// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF writer (`docs/specs/m7-export-3mf.md`).

use std::fmt;
use std::path::Path;

use crate::print_mesh::PrintObject;

/// Why nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeMfWriteError {
    /// No object has a triangle.
    NothingToWrite,
    /// The file couldn't be written; the system's message.
    Io(String),
}

impl fmt::Display for ThreeMfWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThreeMfWriteError::NothingToWrite => write!(f, "nothing to export"),
            ThreeMfWriteError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ThreeMfWriteError {}

/// CRC-32 (IEEE 802.3, polynomial 0xEDB88320, as ZIP uses). `crc32(b"123456789")` is
/// `0xCBF43926`.
pub fn crc32(bytes: &[u8]) -> u32 {
    let _ = bytes;
    unimplemented!("m7-export-3mf")
}

/// A ZIP archive holding `entries` (name, bytes) in order, every entry stored without
/// compression, with correct CRC-32 and sizes, no extra fields, no data descriptors, a fixed
/// modification time (1980-01-01 00:00) so equal input gives equal bytes.
pub fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let _ = entries;
    unimplemented!("m7-export-3mf")
}

/// The bytes of a 3MF package for `objects` (millimeters). `NothingToWrite` when no object
/// has a usable triangle.
pub fn encode(objects: &[PrintObject]) -> Result<Vec<u8>, ThreeMfWriteError> {
    let _ = objects;
    unimplemented!("m7-export-3mf")
}

/// `encode`, written to `path` (the parent folder is not created). No file when
/// `NothingToWrite`.
pub fn write(objects: &[PrintObject], path: &Path) -> Result<(), ThreeMfWriteError> {
    let _ = (objects, path);
    unimplemented!("m7-export-3mf")
}
