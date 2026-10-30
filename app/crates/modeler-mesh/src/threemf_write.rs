// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF writer (`docs/specs/m7-export-3mf.md`).

use std::fmt;
use std::fmt::Write as _;
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
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// A ZIP archive holding `entries` (name, bytes) in order, every entry stored without
/// compression, with correct CRC-32 and sizes, no extra fields, no data descriptors, a fixed
/// modification time (1980-01-01 00:00) so equal input gives equal bytes.
pub fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let crc = crc32(data);
        let offset = out.len() as u32;
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0x0021u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0x0021u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// The bytes of a 3MF package for `objects` (millimeters). `NothingToWrite` when no object
/// has a usable triangle.
pub fn encode(objects: &[PrintObject]) -> Result<Vec<u8>, ThreeMfWriteError> {
    struct Mesh {
        name: String,
        vertices: Vec<[f32; 3]>,
        triangles: Vec<[usize; 3]>,
        color: Option<String>,
    }

    let mut meshes: Vec<Mesh> = Vec::new();
    for object in objects {
        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut triangles = Vec::new();
        for triangle in &object.triangles {
            let corners = triangle.map(|p| [p.x as f32, p.y as f32, p.z as f32]);
            if corners
                .iter()
                .any(|c| c.iter().any(|value| !value.is_finite()))
            {
                continue;
            }
            if corners[0] == corners[1] || corners[0] == corners[2] || corners[1] == corners[2] {
                continue;
            }
            let mut indices = [0usize; 3];
            for (k, corner) in corners.iter().enumerate() {
                let index = match vertices.iter().position(|vertex| vertex == corner) {
                    Some(index) => index,
                    None => {
                        vertices.push(*corner);
                        vertices.len() - 1
                    }
                };
                indices[k] = index;
            }
            triangles.push(indices);
        }
        if triangles.is_empty() {
            continue;
        }
        meshes.push(Mesh {
            name: object.name.clone(),
            vertices,
            triangles,
            color: object.color.map(color_hex),
        });
    }
    if meshes.is_empty() {
        return Err(ThreeMfWriteError::NothingToWrite);
    }

    let mut colors: Vec<String> = Vec::new();
    for mesh in &meshes {
        if let Some(color) = &mesh.color
            && !colors.contains(color)
        {
            colors.push(color.clone());
        }
    }
    let has_materials = !colors.is_empty();
    let first_object_id = if has_materials { 2 } else { 1 };

    let content_types = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
        "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>",
        "<Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>",
        "</Types>"
    );
    let rels = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
        "<Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" ",
        "Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>",
        "</Relationships>"
    );

    let mut model = String::new();
    model.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    model.push_str(concat!(
        "<model unit=\"millimeter\" xml:lang=\"en-US\" ",
        "xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">"
    ));
    model.push_str("<resources>");
    if has_materials {
        model.push_str("<basematerials id=\"1\">");
        for (index, color) in colors.iter().enumerate() {
            write!(
                model,
                "<base name=\"Color {}\" displaycolor=\"{}\" />",
                index + 1,
                color
            )
            .unwrap();
        }
        model.push_str("</basematerials>");
    }
    for (index, mesh) in meshes.iter().enumerate() {
        let id = first_object_id + index;
        let name = if mesh.name.is_empty() {
            "Object"
        } else {
            &mesh.name
        };
        write!(
            model,
            "<object id=\"{id}\" type=\"model\" name=\"{}\"",
            escape_xml(name)
        )
        .unwrap();
        if let Some(color) = &mesh.color
            && let Some(base) = colors.iter().position(|c| c == color)
        {
            write!(model, " pid=\"1\" pindex=\"{base}\"").unwrap();
        }
        model.push_str("><mesh><vertices>");
        for vertex in &mesh.vertices {
            write!(
                model,
                "<vertex x=\"{}\" y=\"{}\" z=\"{}\" />",
                vertex[0], vertex[1], vertex[2]
            )
            .unwrap();
        }
        model.push_str("</vertices><triangles>");
        for triangle in &mesh.triangles {
            write!(
                model,
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\" />",
                triangle[0], triangle[1], triangle[2]
            )
            .unwrap();
        }
        model.push_str("</triangles></mesh></object>");
    }
    model.push_str("</resources><build>");
    for index in 0..meshes.len() {
        write!(model, "<item objectid=\"{}\" />", first_object_id + index).unwrap();
    }
    model.push_str("</build></model>");

    Ok(zip_stored(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ]))
}

fn color_hex(color: [f32; 4]) -> String {
    fn byte(value: f32) -> u8 {
        if value.is_nan() {
            0
        } else {
            (value * 255.0).round().clamp(0.0, 255.0) as u8
        }
    }
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
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// `encode`, written to `path` (the parent folder is not created). No file when
/// `NothingToWrite`.
pub fn write(objects: &[PrintObject], path: &Path) -> Result<(), ThreeMfWriteError> {
    let bytes = encode(objects)?;
    std::fs::write(path, bytes).map_err(|err| ThreeMfWriteError::Io(err.to_string()))
}
