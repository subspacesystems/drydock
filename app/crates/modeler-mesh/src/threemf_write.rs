// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF writer (`docs/specs/m7-export-3mf.md`).

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::print_mesh::{PrintObject, Triangle};

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
    let mut table = [0u32; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// A ZIP archive holding `entries` (name, bytes) in order, every entry stored without
/// compression, with correct CRC-32 and sizes, no extra fields, no data descriptors, a fixed
/// modification time (1980-01-01 00:00) so equal input gives equal bytes.
pub fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    const DATE: u16 = 0x0021; // 1980-01-01
    let mut out = Vec::new();
    let mut cdir = Vec::new();
    for (name, data) in entries {
        let name = name.as_bytes();
        let crc = crc32(data);
        let off = out.len() as u32;
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // stored
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // no extra
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        cdir.extend_from_slice(b"PK\x01\x02");
        cdir.extend_from_slice(&20u16.to_le_bytes()); // made by
        cdir.extend_from_slice(&20u16.to_le_bytes()); // version needed
        cdir.extend_from_slice(&0u16.to_le_bytes()); // flags
        cdir.extend_from_slice(&0u16.to_le_bytes()); // stored
        cdir.extend_from_slice(&0u16.to_le_bytes()); // time
        cdir.extend_from_slice(&DATE.to_le_bytes());
        cdir.extend_from_slice(&crc.to_le_bytes());
        cdir.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cdir.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cdir.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cdir.extend_from_slice(&0u16.to_le_bytes()); // no extra
        cdir.extend_from_slice(&0u16.to_le_bytes()); // no comment
        cdir.extend_from_slice(&0u16.to_le_bytes()); // disk
        cdir.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        cdir.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        cdir.extend_from_slice(&off.to_le_bytes());
        cdir.extend_from_slice(name);
    }
    let cdir_off = out.len() as u32;
    out.extend_from_slice(&cdir);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cdir disk
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cdir.len() as u32).to_le_bytes());
    out.extend_from_slice(&cdir_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // no comment
    out
}

/// The bytes of a 3MF package for `objects` (millimeters). `NothingToWrite` when no object
/// has a usable triangle.
pub fn encode(objects: &[PrintObject]) -> Result<Vec<u8>, ThreeMfWriteError> {
    let kept: Vec<&PrintObject> = objects
        .iter()
        .filter(|o| o.triangles.iter().any(usable))
        .collect();
    if kept.is_empty() {
        return Err(ThreeMfWriteError::NothingToWrite);
    }

    // Colors, in order of first use.
    let mut bases: Vec<String> = Vec::new();
    let mut base_of: HashMap<String, usize> = HashMap::new();
    let mut pindex: Vec<Option<usize>> = Vec::new();
    for o in &kept {
        match o.color {
            Some(c) => {
                let s = color_hex(c);
                let ix = match base_of.get(&s) {
                    Some(&ix) => ix,
                    None => {
                        bases.push(s);
                        base_of.insert(bases.last().unwrap().clone(), bases.len() - 1);
                        bases.len() - 1
                    }
                };
                pindex.push(Some(ix));
            }
            None => pindex.push(None),
        }
    }
    let has_colors = !bases.is_empty();
    let mat_id: Option<u32> = has_colors.then_some(1);
    let obj_id = |i: usize| (if has_colors { 2 } else { 1 }) + i as u32;

    let mut model = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">",
    );
    model.push_str("<resources>");
    if let Some(id) = mat_id {
        model.push_str(&format!("<basematerials id=\"{id}\">"));
        for b in &bases {
            model.push_str(&format!("<base displaycolor=\"{b}\"/>"));
        }
        model.push_str("</basematerials>");
    }
    for (i, o) in kept.iter().enumerate() {
        let (vertices, triangles) = weld(o.triangles.iter().filter(|t| usable(t)).copied());
        let name = if o.name.is_empty() {
            "Object"
        } else {
            o.name.as_str()
        };
        model.push_str(&format!(
            "<object id=\"{}\" type=\"model\" name=\"{}\"",
            obj_id(i),
            escape(name)
        ));
        match pindex[i] {
            Some(ix) if has_colors => {
                model.push_str(&format!(" pid=\"{}\" pindex=\"{ix}\"", mat_id.unwrap()))
            }
            _ => {}
        }
        model.push_str("><mesh><vertices>");
        for v in &vertices {
            model.push_str(&format!(
                "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                v.0, v.1, v.2
            ));
        }
        model.push_str("</vertices><triangles>");
        for t in &triangles {
            model.push_str(&format!(
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                t[0], t[1], t[2]
            ));
        }
        model.push_str("</triangles></mesh></object>");
    }
    model.push_str("</resources><build>");
    for i in 0..kept.len() {
        model.push_str(&format!("<item objectid=\"{}\"/>", obj_id(i)));
    }
    model.push_str("</build></model>");
    model.push('\n');

    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\
         </Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" \
         Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\
         </Relationships>";
    let parts: [(&str, Vec<u8>); 3] = [
        ("[Content_Types].xml", content_types.as_bytes().to_vec()),
        ("_rels/.rels", rels.as_bytes().to_vec()),
        ("3D/3dmodel.model", model.into_bytes()),
    ];
    Ok(zip_stored(&[
        (parts[0].0, &parts[0].1),
        (parts[1].0, &parts[1].1),
        (parts[2].0, &parts[2].1),
    ]))
}

/// `encode`, written to `path` (the parent folder is not created). No file when
/// `NothingToWrite`.
/// A triangle is usable when every corner is finite and no two corners share a vertex.
fn usable(t: &Triangle) -> bool {
    let [a, b, c] = *t;
    for p in [a, b, c] {
        if !p.is_finite() {
            return false;
        }
    }
    if a == b || a == c || b == c {
        return false;
    }
    true
}

type F32 = (f32, f32, f32);

/// The corners of `triangles` as `(x, y, z)` in `f32`, welded by first use.
fn weld(triangles: impl Iterator<Item = Triangle>) -> (Vec<F32>, Vec<[usize; 3]>) {
    let mut vertices: Vec<F32> = Vec::new();
    let mut known: HashMap<(u32, u32, u32), usize> = HashMap::new();
    let key = |v: (f32, f32, f32)| {
        // `-0.0` and `0.0` weld together, so normalize negative zero first.
        let n = |x: f32| {
            if x == 0.0 {
                0.0_f32.to_bits()
            } else {
                x.to_bits()
            }
        };
        (n(v.0), n(v.1), n(v.2))
    };
    let mut indices = Vec::new();
    for t in triangles {
        let mut ix = [0usize; 3];
        for (k, p) in t.iter().enumerate() {
            let v = (p.x as f32, p.y as f32, p.z as f32);
            let ky = key(v);
            match known.get(&ky) {
                Some(&n) => ix[k] = n,
                None => {
                    vertices.push(v);
                    let n = vertices.len() - 1;
                    known.insert(ky, n);
                    ix[k] = n;
                }
            }
        }
        indices.push(ix);
    }
    (vertices, indices)
}

/// `Some` components quantized to bytes; NaN counts as 0.
fn color_hex(c: [f32; 4]) -> String {
    let b = |x: f32| {
        if x.is_nan() {
            0
        } else {
            (x * 255.0).round().clamp(0.0, 255.0) as u8
        }
    };
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        b(c[0]),
        b(c[1]),
        b(c[2]),
        b(c[3])
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn write(objects: &[PrintObject], path: &Path) -> Result<(), ThreeMfWriteError> {
    let bytes = encode(objects)?;
    std::fs::write(path, bytes).map_err(|e| ThreeMfWriteError::Io(e.to_string()))?;
    Ok(())
}
