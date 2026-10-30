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
    let mut header = b"modeler STL: ".to_vec();
    for ch in name.chars() {
        if header.len() + ch.len_utf8() > 80 {
            break;
        }
        header.extend_from_slice(ch.encode_utf8(&mut [0u8; 4]).as_bytes());
    }
    while header.len() < 80 {
        header.push(0);
    }

    let mut bytes = header;
    bytes.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for [a, b, c] in triangles {
        for component in triangle_normal(*a, *b, *c) {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
        for p in [*a, *b, *c] {
            bytes.extend_from_slice(&(p.x as f32).to_le_bytes());
            bytes.extend_from_slice(&(p.y as f32).to_le_bytes());
            bytes.extend_from_slice(&(p.z as f32).to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes
}

/// `(b - a) x (c - a)` normalized, or zero for a degenerate triangle, as f32.
fn triangle_normal(a: DVec3, b: DVec3, c: DVec3) -> [f32; 3] {
    let cross = (b - a).cross(c - a);
    let length = cross.length();
    if length <= 0.0 {
        return [0.0, 0.0, 0.0];
    }
    let n = cross / length;
    [n.x as f32, n.y as f32, n.z as f32]
}

/// A file stem made safe for any file system: letters, digits, `-`, `_` and spaces are
/// kept, everything else becomes `_`; leading and trailing spaces are trimmed; at most
/// 100 characters; an empty result is `"part"`.
pub fn sanitize_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    out = out.trim_matches(' ').to_string();
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
    std::fs::create_dir_all(dir).map_err(|e| StlWriteError::Io(e.to_string()))?;

    let mut used = Vec::new();
    let mut paths = Vec::new();
    for object in objects {
        if object.triangles.is_empty() {
            continue;
        }
        let stem = unique_stem(sanitize_name(&object.name), &mut used);
        let path = dir.join(format!("{stem}.stl"));
        std::fs::write(&path, encode(&object.name, &object.triangles))
            .map_err(|e| StlWriteError::Io(e.to_string()))?;
        paths.push(path);
    }
    Ok(paths)
}

/// Appends `-2`, `-3`, ... until the stem is taken by no earlier object, ignoring case.
fn unique_stem(base: String, used: &mut Vec<String>) -> String {
    if !used.iter().any(|u| u.eq_ignore_ascii_case(&base)) {
        used.push(base.clone());
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !used.iter().any(|u| u.eq_ignore_ascii_case(&candidate)) {
            used.push(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

/// All objects' triangles in one binary STL at `path`, in object order; the header names
/// the first object that has triangles. `NothingToWrite` (and no file) when none has any.
pub fn write_combined(objects: &[PrintObject], path: &Path) -> Result<(), StlWriteError> {
    let name = match objects.iter().find(|o| !o.triangles.is_empty()) {
        Some(o) => o.name.clone(),
        None => return Err(StlWriteError::NothingToWrite),
    };
    let triangles: Vec<Triangle> = objects
        .iter()
        .flat_map(|o| o.triangles.iter().copied())
        .collect();
    std::fs::write(path, encode(&name, &triangles)).map_err(|e| StlWriteError::Io(e.to_string()))
}
