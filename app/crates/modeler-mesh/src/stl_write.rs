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
    let mut bytes = Vec::with_capacity(84 + triangles.len() * 50);
    bytes.extend_from_slice(b"modeler STL: ");
    for ch in name.chars() {
        if bytes.len() + ch.len_utf8() > 80 {
            break;
        }
        let mut buf = [0; 4];
        bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
    }
    bytes.resize(80, 0);
    bytes.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for t in triangles {
        let cross = (t[1] - t[0]).cross(t[2] - t[0]);
        let normal = if cross.length_squared() > 0.0 {
            cross.normalize()
        } else {
            glam::DVec3::ZERO
        };
        for v in normal.to_array() {
            bytes.extend_from_slice(&(v as f32).to_le_bytes());
        }
        for corner in t {
            for v in corner.to_array() {
                bytes.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes
}

/// A file stem made safe for any file system: letters, digits, `-`, `_` and spaces are
/// kept, everything else becomes `_`; leading and trailing spaces are trimmed; at most
/// 100 characters; an empty result is `"part"`.
pub fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | ' ') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let out = out.trim_matches(' ').chars().take(100).collect::<String>();
    if out.is_empty() {
        "part".to_string()
    } else {
        out
    }
}

/// One `<name>.stl` per object that has triangles, inside `dir` (created if missing).
/// Names go through [`sanitize_name`]; a stem already used (compared ignoring case) gets
/// `-2`, `-3`, ... and the result never repeats a stem. Returns the paths in object order.
/// Objects without triangles are skipped. `NothingToWrite` (and no file) when none has any.
pub fn write_separate(objects: &[PrintObject], dir: &Path) -> Result<Vec<PathBuf>, StlWriteError> {
    if !objects.iter().any(|o| !o.triangles.is_empty()) {
        return Err(StlWriteError::NothingToWrite);
    }
    std::fs::create_dir_all(dir).map_err(|err| StlWriteError::Io(err.to_string()))?;
    let mut used: Vec<String> = Vec::new();
    let mut paths = Vec::new();
    for object in objects {
        if object.triangles.is_empty() {
            continue;
        }
        let base = sanitize_name(&object.name);
        let mut stem = base.clone();
        let mut n = 2;
        while used.iter().any(|u| u.eq_ignore_ascii_case(&stem)) {
            stem = format!("{base}-{n}");
            n += 1;
        }
        used.push(stem.clone());
        let path = dir.join(format!("{stem}.stl"));
        std::fs::write(&path, encode(&object.name, &object.triangles))
            .map_err(|err| StlWriteError::Io(err.to_string()))?;
        paths.push(path);
    }
    Ok(paths)
}

/// All objects' triangles in one binary STL at `path`, in object order; the header names
/// the first object that has triangles. `NothingToWrite` (and no file) when none has any.
pub fn write_combined(objects: &[PrintObject], path: &Path) -> Result<(), StlWriteError> {
    let Some(first) = objects.iter().find(|o| !o.triangles.is_empty()) else {
        return Err(StlWriteError::NothingToWrite);
    };
    let triangles: Vec<Triangle> = objects
        .iter()
        .flat_map(|o| o.triangles.iter().copied())
        .collect();
    std::fs::write(path, encode(&first.name, &triangles))
        .map_err(|err| StlWriteError::Io(err.to_string()))
}
