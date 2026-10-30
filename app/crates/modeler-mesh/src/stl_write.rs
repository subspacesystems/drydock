// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Binary STL writer (`docs/specs/m7-export-stl.md`).

use std::fmt;
use std::path::{Path, PathBuf};

use glam::DVec3;

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
    let mut header = [0u8; 80];
    let prefix = b"modeler STL: ";
    let mut end = prefix.len();
    for c in name.chars() {
        let mut buf = [0u8; 4];
        let cb = c.encode_utf8(&mut buf);
        if end + cb.len() > 80 {
            break;
        }
        header[end..end + cb.len()].copy_from_slice(cb.as_bytes());
        end += cb.len();
    }
    let mut bytes = Vec::with_capacity(84 + 50 * triangles.len());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for t in triangles {
        let normal = (t[1] - t[0]).cross(t[2] - t[0]);
        let normal = if normal.length_squared() == 0.0 {
            DVec3::ZERO
        } else {
            normal / normal.length()
        };
        let mut push = |v: f64| {
            bytes.extend_from_slice(&(v as f32).to_le_bytes());
        };
        for v in [normal.x, normal.y, normal.z] {
            push(v);
        }
        for c in t.iter() {
            for v in [c.x, c.y, c.z] {
                push(v);
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
    let kept: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let kept = kept
        .trim()
        .chars()
        .take(100)
        .collect::<String>()
        .trim()
        .to_owned();
    if kept.is_empty() {
        "part".to_owned()
    } else {
        kept
    }
}

/// One `<name>.stl` per object that has triangles, inside `dir` (created if missing).
/// Names go through [`sanitize_name`]; a stem already used (compared ignoring case) gets
/// `-2`, `-3`, ... and the result never repeats a stem. Returns the paths in object order.
/// Objects without triangles are skipped. `NothingToWrite` (and no file) when none has any.
pub fn write_separate(objects: &[PrintObject], dir: &Path) -> Result<Vec<PathBuf>, StlWriteError> {
    let any = objects.iter().any(|o| !o.triangles.is_empty());
    if !any {
        return Err(StlWriteError::NothingToWrite);
    }
    std::fs::create_dir_all(dir).map_err(|e| StlWriteError::Io(e.to_string()))?;
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
            .map_err(|e| StlWriteError::Io(e.to_string()))?;
        paths.push(path);
    }
    Ok(paths)
}

/// All objects' triangles in one binary STL at `path`, in object order; the header names
/// the first object that has triangles. `NothingToWrite` (and no file) when none has any.
pub fn write_combined(objects: &[PrintObject], path: &Path) -> Result<(), StlWriteError> {
    let first = objects.iter().find(|o| !o.triangles.is_empty());
    let Some(first) = first else {
        return Err(StlWriteError::NothingToWrite);
    };
    let mut triangles = Vec::new();
    for object in objects {
        triangles.extend_from_slice(&object.triangles);
    }
    std::fs::write(path, encode(&first.name, &triangles))
        .map_err(|e| StlWriteError::Io(e.to_string()))?;
    Ok(())
}
