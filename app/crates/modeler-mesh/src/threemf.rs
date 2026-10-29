// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF reader (`docs/specs/m7-import-3mf.md`).

use std::fmt;

use crate::MeshObject;

/// What a 3MF file holds: its length unit and the printable objects.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreeMf {
    /// Meters per file unit.
    pub unit_meters: f64,
    pub objects: Vec<MeshObject>,
}

/// Why bytes couldn't be read as 3MF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeMfError {
    /// No ZIP end-of-directory record.
    NotZip,
    /// A damaged ZIP or an entry that can't be read.
    Zip(String),
    /// The model part is not in the archive.
    MissingModel,
    /// The model part is not well-formed XML.
    Xml(String),
    /// The model breaks the 3MF rules.
    Model(String),
}

impl fmt::Display for ThreeMfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThreeMfError::NotZip => write!(f, "not a 3MF file"),
            ThreeMfError::Zip(m) => write!(f, "bad 3MF archive: {m}"),
            ThreeMfError::MissingModel => write!(f, "3MF has no model part"),
            ThreeMfError::Xml(m) => write!(f, "bad 3MF model XML: {m}"),
            ThreeMfError::Model(m) => write!(f, "bad 3MF model: {m}"),
        }
    }
}

impl std::error::Error for ThreeMfError {}

/// Reads a 3MF archive.
pub fn read(bytes: &[u8]) -> Result<ThreeMf, ThreeMfError> {
    let _ = bytes;
    unimplemented!("m7-import-3mf")
}
