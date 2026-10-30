// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF writer (`docs/specs/m7-export-3mf.md`).

use std::collections::HashMap;
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
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        table[i as usize] = c;
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
    let mut bytes = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let crc = crc32(data);
        let offset = bytes.len() as u32;
        let name = name.as_bytes();
        bytes.extend_from_slice(b"PK\x03\x04");
        bytes.extend_from_slice(&20u16.to_le_bytes()); // version needed
        bytes.extend_from_slice(&0u16.to_le_bytes()); // flags
        bytes.extend_from_slice(&0u16.to_le_bytes()); // stored
        bytes.extend_from_slice(&0u16.to_le_bytes()); // time
        bytes.extend_from_slice(&0x21u16.to_le_bytes()); // 1980-01-01
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes()); // no extra
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(data);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes()); // made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes()); // flags
        central.extend_from_slice(&0u16.to_le_bytes()); // stored
        central.extend_from_slice(&0u16.to_le_bytes()); // time
        central.extend_from_slice(&0x21u16.to_le_bytes()); // 1980-01-01
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        central.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let cdir_offset = bytes.len() as u32;
    bytes.extend_from_slice(&central);
    bytes.extend_from_slice(b"PK\x05\x06");
    bytes.extend_from_slice(&0u16.to_le_bytes()); // disk
    bytes.extend_from_slice(&0u16.to_le_bytes()); // central directory disk
    bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // on this disk
    bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // total
    bytes.extend_from_slice(&(central.len() as u32).to_le_bytes()); // central size
    bytes.extend_from_slice(&cdir_offset.to_le_bytes()); // central offset
    bytes.extend_from_slice(&0u16.to_le_bytes()); // comment length
    bytes
}

/// The bytes of a 3MF package for `objects` (millimeters). `NothingToWrite` when no object
/// has a usable triangle.
pub fn encode(objects: &[PrintObject]) -> Result<Vec<u8>, ThreeMfWriteError> {
    struct Encoded {
        name: String,
        vertices: Vec<[f32; 3]>,
        triangles: Vec<[usize; 3]>,
        color: Option<String>,
    }

    let mut encoded = Vec::new();
    for object in objects {
        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut indices: HashMap<(u32, u32, u32), usize> = HashMap::new();
        let mut triangles = Vec::new();
        for triangle in &object.triangles {
            let corners = triangle.map(|c| [c.x as f32, c.y as f32, c.z as f32]);
            if corners.iter().flatten().any(|v| !v.is_finite()) {
                continue;
            }
            let keys = corners.map(vertex_key);
            if keys[0] == keys[1] || keys[0] == keys[2] || keys[1] == keys[2] {
                continue;
            }
            let mut ids = [0usize; 3];
            for k in 0..3 {
                let next = vertices.len();
                let id = *indices.entry(keys[k]).or_insert_with(|| {
                    vertices.push(corners[k]);
                    next
                });
                ids[k] = id;
            }
            triangles.push(ids);
        }
        if triangles.is_empty() {
            continue;
        }
        encoded.push(Encoded {
            name: object.name.clone(),
            vertices,
            triangles,
            color: object.color.map(quantize_color),
        });
    }
    if encoded.is_empty() {
        return Err(ThreeMfWriteError::NothingToWrite);
    }

    let mut colors: Vec<String> = Vec::new();
    for object in &encoded {
        if let Some(color) = &object.color
            && !colors.contains(color)
        {
            colors.push(color.clone());
        }
    }
    let materials_id = (!colors.is_empty()).then_some(1u32);
    let first_object_id = if materials_id.is_some() { 2 } else { 1 };

    let mut model = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"><resources>",
    );
    if materials_id.is_some() {
        model.push_str("<basematerials id=\"1\">");
        for (i, color) in colors.iter().enumerate() {
            model.push_str(&format!(
                "<base name=\"color {}\" displaycolor=\"{}\" />",
                i, color
            ));
        }
        model.push_str("</basematerials>");
    }
    for (i, object) in encoded.iter().enumerate() {
        let id = first_object_id + i as u32;
        let name = if object.name.is_empty() {
            "Object"
        } else {
            &object.name
        };
        model.push_str(&format!(
            "<object id=\"{id}\" type=\"model\" name=\"{}\"",
            escape_xml(name)
        ));
        if let (Some(pid), Some(color)) = (materials_id, &object.color) {
            let pindex = colors.iter().position(|c| c == color).unwrap_or(0);
            model.push_str(&format!(" pid=\"{pid}\" pindex=\"{pindex}\""));
        }
        model.push_str("><mesh><vertices>");
        for vertex in &object.vertices {
            model.push_str(&format!(
                "<vertex x=\"{}\" y=\"{}\" z=\"{}\" />",
                vertex[0], vertex[1], vertex[2]
            ));
        }
        model.push_str("</vertices><triangles>");
        for triangle in &object.triangles {
            model.push_str(&format!(
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\" />",
                triangle[0], triangle[1], triangle[2]
            ));
        }
        model.push_str("</triangles></mesh></object>");
    }
    model.push_str("</resources><build>");
    for i in 0..encoded.len() {
        model.push_str(&format!(
            "<item objectid=\"{}\" />",
            first_object_id + i as u32
        ));
    }
    model.push_str("</build></model>");

    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\" /><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\" /></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\" /></Relationships>";
    Ok(zip_stored(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ]))
}

fn vertex_key(v: [f32; 3]) -> (u32, u32, u32) {
    let bits = |x: f32| if x == 0.0 { 0 } else { x.to_bits() };
    (bits(v[0]), bits(v[1]), bits(v[2]))
}

fn quantize_color(color: [f32; 4]) -> String {
    let byte = |c: f32| {
        if c.is_nan() {
            0u32
        } else {
            (c * 255.0).round().clamp(0.0, 255.0) as u32
        }
    };
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        byte(color[0]),
        byte(color[1]),
        byte(color[2]),
        byte(color[3])
    )
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// `encode`, written to `path` (the parent folder is not created). No file when
/// `NothingToWrite`.
pub fn write(objects: &[PrintObject], path: &Path) -> Result<(), ThreeMfWriteError> {
    let bytes = encode(objects)?;
    std::fs::write(path, bytes).map_err(|e| ThreeMfWriteError::Io(e.to_string()))
}
