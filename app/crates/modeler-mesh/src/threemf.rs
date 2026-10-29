// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF reader (`docs/specs/m7-import-3mf.md`).

use std::fmt;

use roxmltree::Node;

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
    let parts = zip_parts(bytes)?;
    let (_, part) = find_model_part(&parts)?;
    parse_model_part(&part)
}

// ---------------------------------------------------------------------------
// ZIP
// ---------------------------------------------------------------------------

fn zip_err(message: &str) -> ThreeMfError {
    ThreeMfError::Zip(message.to_string())
}

/// Entry name and bytes of every archive member, in central-directory order.
fn zip_parts(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ThreeMfError> {
    // End of central directory: signature 50 4b 05 06, searched backwards from the
    // last 22 bytes over at most 65535 bytes of trailing comment.
    let (count, cdir) = {
        let low = bytes.len().saturating_sub(65535 + 22);
        let mut eocd = None;
        for end in (low..=bytes.len()).rev() {
            if end + 4 > bytes.len() {
                continue;
            }
            if &bytes[end..end + 4] == b"PK\x05\x06" {
                eocd = Some(end);
                break;
            }
        }
        let eocd = eocd.ok_or(ThreeMfError::NotZip)?;
        if eocd + 22 > bytes.len() {
            return Err(ThreeMfError::NotZip);
        }
        let count = le16(bytes, eocd + 10)? as usize;
        let cdir = le32(bytes, eocd + 16)? as u64;
        (count, cdir)
    };
    if cdir > bytes.len() as u64 {
        return Err(zip_err("central directory offset past end of file"));
    }
    let mut pos = cdir as usize;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        if pos + 46 > bytes.len() {
            return Err(zip_err("truncated central directory"));
        }
        if &bytes[pos..pos + 4] != b"PK\x01\x02" {
            return Err(zip_err("bad central directory signature"));
        }
        let method = le16(bytes, pos + 10)?;
        let csize = le32(bytes, pos + 20)?;
        let usize_ = le32(bytes, pos + 24)? as u64;
        let name_len = le16(bytes, pos + 28)? as usize;
        let extra_len = le16(bytes, pos + 30)? as usize;
        let comment_len = le16(bytes, pos + 32)? as usize;
        let local = le32(bytes, pos + 42)? as u64;
        if pos + 46 + name_len > bytes.len() {
            return Err(zip_err("truncated central directory entry name"));
        }
        let name = String::from_utf8(bytes[pos + 46..pos + 46 + name_len].to_vec())
            .map_err(|_| zip_err("entry name is not UTF-8"))?;
        pos += 46 + name_len + extra_len + comment_len;
        if pos > bytes.len() {
            return Err(zip_err("central directory entry runs past end of file"));
        }
        let data = entry_data(bytes, method, csize, usize_, local)?;
        parts.push((name, data));
    }
    Ok(parts)
}

/// The bytes of one entry after its local header.
fn entry_data(
    bytes: &[u8],
    method: u16,
    csize: u32,
    usize_: u64,
    local: u64,
) -> Result<Vec<u8>, ThreeMfError> {
    if usize_ > 1 << 30 {
        return Err(zip_err("entry larger than 1 GiB"));
    }
    let usize_ = usize_ as usize;
    let local = local as usize;
    if local + 30 > bytes.len() {
        return Err(zip_err("local header offset past end of file"));
    }
    if &bytes[local..local + 4] != b"PK\x03\x04" {
        return Err(zip_err("bad local header signature"));
    }
    let name_len = le16(bytes, local + 26)? as usize;
    let extra_len = le16(bytes, local + 28)? as usize;
    let start = local + 30 + name_len + extra_len;
    if start + csize as usize > bytes.len() {
        return Err(zip_err("entry data runs past end of file"));
    }
    let raw = &bytes[start..start + csize as usize];
    let data = match method {
        0 => raw.to_vec(),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, usize_)
            .map_err(|e| zip_err(&format!("bad deflate data: {e}")))?,
        m => {
            return Err(zip_err(&format!("unsupported compression method {m}")));
        }
    };
    if data.len() != usize_ {
        return Err(zip_err("entry size mismatch"));
    }
    Ok(data)
}

fn le16(bytes: &[u8], pos: usize) -> Result<u16, ThreeMfError> {
    bytes
        .get(pos..pos + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| zip_err("truncated zip"))
}

fn le32(bytes: &[u8], pos: usize) -> Result<u32, ThreeMfError> {
    bytes
        .get(pos..pos + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| zip_err("truncated zip"))
}

// ---------------------------------------------------------------------------
// Model location and XML
// ---------------------------------------------------------------------------

/// The model part: `3D/3dmodel.model`, or the target of the first 3D-model
/// relationship in `_rels/.rels` if that file is present.
fn find_model_part(parts: &[(String, Vec<u8>)]) -> Result<(String, Vec<u8>), ThreeMfError> {
    const DEFAULT: &str = "3D/3dmodel.model";
    let name = match parts.iter().find(|(n, _)| n == "_rels/.rels") {
        None => DEFAULT.to_string(),
        Some((_, rels)) => {
            let doc = roxmltree::Document::parse(
                std::str::from_utf8(rels).map_err(|e| ThreeMfError::Xml(e.to_string()))?,
            )
            .map_err(|e| ThreeMfError::Xml(e.to_string()))?;
            doc.descendants()
                .filter(|n| n.tag_name().name() == "Relationship")
                .find_map(|n| {
                    let ty = n.attribute("Type")?;
                    if ty.ends_with("/3dmodel") {
                        n.attribute("Target").map(str::to_string)
                    } else {
                        None
                    }
                })
                .ok_or(ThreeMfError::MissingModel)?
                .trim_start_matches('/')
                .to_string()
        }
    };
    parts
        .iter()
        .find(|(n, _)| n == &name)
        .map(|(n, b)| (n.clone(), b.clone()))
        .ok_or(ThreeMfError::MissingModel)
}

/// One `object` element: its mesh and components.
struct Object {
    id: u32,
    name: String,
    vertices: Vec<[f64; 3]>,
    triangles: Vec<[usize; 3]>,
    components: Vec<(u32, [f64; 12])>,
}

/// 3x3 homogeneous matrix, row-vector convention: column-major storage in
/// `m[col * 3 + row]`, translation in `m[9..12]`.
#[derive(Clone, Copy)]
struct Matrix {
    m: [f64; 12],
}

impl Matrix {
    fn identity() -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        }
    }

    /// `p` transformed as a row vector.
    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let m = self.m;
        [
            p[0] * m[0] + p[1] * m[3] + p[2] * m[6] + m[9],
            p[0] * m[1] + p[1] * m[4] + p[2] * m[7] + m[10],
            p[0] * m[2] + p[1] * m[5] + p[2] * m[8] + m[11],
        ]
    }

    /// `self` then `other` (row-vector: the point is transformed by `self`
    /// first, `other` last).
    fn then(&self, other: &Matrix) -> Matrix {
        let a = self.m;
        let b = other.m;
        let rot = [
            a[0] * b[0] + a[3] * b[1] + a[6] * b[2],
            a[0] * b[3] + a[3] * b[4] + a[6] * b[5],
            a[0] * b[6] + a[3] * b[7] + a[6] * b[8],
            a[1] * b[0] + a[4] * b[1] + a[7] * b[2],
            a[1] * b[3] + a[4] * b[4] + a[7] * b[5],
            a[1] * b[6] + a[4] * b[7] + a[7] * b[8],
            a[2] * b[0] + a[5] * b[1] + a[8] * b[2],
            a[2] * b[3] + a[5] * b[4] + a[8] * b[5],
            a[2] * b[6] + a[5] * b[7] + a[8] * b[8],
        ];
        Matrix {
            m: [
                rot[0],
                rot[1],
                rot[2],
                rot[3],
                rot[4],
                rot[5],
                rot[6],
                rot[7],
                rot[8],
                a[9] * b[0] + a[10] * b[3] + a[11] * b[6] + b[9],
                a[9] * b[1] + a[10] * b[4] + a[11] * b[7] + b[10],
                a[9] * b[2] + a[10] * b[5] + a[11] * b[8] + b[11],
            ],
        }
    }

    /// Determinant of the 3x3 rotation part.
    fn determinant(&self) -> f64 {
        let m = self.m;
        m[0] * (m[4] * m[8] - m[5] * m[7]) - m[3] * (m[1] * m[8] - m[5] * m[2])
            + m[6] * (m[1] * m[7] - m[4] * m[2])
    }
}

/// Parses the model part XML into its unit and the flattened objects.
fn parse_model_part(part: &[u8]) -> Result<ThreeMf, ThreeMfError> {
    let text = String::from_utf8(part.to_vec())
        .map_err(|_| ThreeMfError::Xml("model part is not UTF-8".to_string()))?;
    let doc = roxmltree::Document::parse(&text).map_err(|e| ThreeMfError::Xml(e.to_string()))?;
    let root = doc.root_element();
    let unit_meters = unit_meters(root.attribute("unit"))?;
    let objects = root
        .descendants()
        .filter(|el| el.tag_name().name() == "object")
        .map(parse_object)
        .collect::<Result<Vec<Object>, _>>()?;
    let items: Vec<Node> = root
        .descendants()
        .filter(|el| el.tag_name().name() == "build")
        .flat_map(|b| b.descendants().filter(|e| e.tag_name().name() == "item"))
        .collect();
    let result = if items.is_empty() {
        let used: std::collections::HashSet<u32> = objects
            .iter()
            .flat_map(|o| o.components.iter().map(|(id, _)| *id))
            .collect();
        let mut out = Vec::new();
        for o in &objects {
            if used.contains(&o.id) {
                continue;
            }
            let mut tris = Vec::new();
            collect(
                &o.id,
                &objects,
                &Matrix::identity(),
                &mut tris,
                &mut Vec::new(),
            )?;
            out.push(MeshObject {
                name: o.name.clone(),
                triangles: tris,
            });
        }
        out
    } else {
        let mut out = Vec::new();
        for item in items {
            let id = attr_usize(item, "objectid", "objectid")?;
            let matrix = item
                .attribute("transform")
                .map(transform)
                .transpose()?
                .unwrap_or(Matrix::identity());
            let obj = objects.iter().find(|o| o.id == id).ok_or_else(|| {
                ThreeMfError::Model(format!("item references unknown object {id}"))
            })?;
            let mut tris = Vec::new();
            collect(&id, &objects, &matrix, &mut tris, &mut Vec::new())?;
            out.push(MeshObject {
                name: obj.name.clone(),
                triangles: tris,
            });
        }
        out
    };
    Ok(ThreeMf {
        unit_meters,
        objects: result,
    })
}

fn unit_meters(unit: Option<&str>) -> Result<f64, ThreeMfError> {
    match unit {
        None | Some("millimeter") => Ok(1e-3),
        Some("micron") => Ok(1e-6),
        Some("centimeter") => Ok(1e-2),
        Some("inch") => Ok(0.0254),
        Some("foot") => Ok(0.3048),
        Some("meter") => Ok(1.0),
        Some(u) => Err(ThreeMfError::Model(format!("unknown unit {u:?}"))),
    }
}

fn parse_object(el: Node) -> Result<Object, ThreeMfError> {
    let id = attr_usize(el, "id", "id")?;
    let name = el
        .attribute("name")
        .map(str::to_string)
        .unwrap_or_else(|| format!("Object {id}"));
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let mut components = Vec::new();
    for child in el.children() {
        match child.tag_name().name() {
            "mesh" => {
                for section in child.children() {
                    match section.tag_name().name() {
                        "vertices" => {
                            for v in section.children() {
                                if v.tag_name().name() != "vertex" {
                                    continue;
                                }
                                vertices.push([
                                    attr_num(v, "x")?,
                                    attr_num(v, "y")?,
                                    attr_num(v, "z")?,
                                ]);
                            }
                        }
                        "triangles" => {
                            for t in section.children() {
                                if t.tag_name().name() != "triangle" {
                                    continue;
                                }
                                let tri = [
                                    corner_index(t, "v1")?,
                                    corner_index(t, "v2")?,
                                    corner_index(t, "v3")?,
                                ];
                                for ix in tri {
                                    if ix >= vertices.len() {
                                        return Err(ThreeMfError::Model(format!(
                                            "triangle references vertex {ix}, mesh has {}",
                                            vertices.len()
                                        )));
                                    }
                                }
                                triangles.push(tri);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "components" => {
                for c in child.children() {
                    if c.tag_name().name() != "component" {
                        continue;
                    }
                    let target = attr_usize(c, "objectid", "objectid")?;
                    let matrix = c
                        .attribute("transform")
                        .map(transform)
                        .transpose()?
                        .unwrap_or(Matrix::identity());
                    components.push((target, matrix.m));
                }
            }
            _ => {}
        }
    }
    Ok(Object {
        id,
        name,
        vertices,
        triangles,
        components,
    })
}

/// The triangles of object `id`, depth first: the object's own mesh, then its
/// components. `matrix` is the combined transform applied to everything
/// collected here (component matrices first, the item's last, so callers pass
/// the item matrix and the components multiply their own on top).
fn collect(
    id: &u32,
    objects: &[Object],
    matrix: &Matrix,
    out: &mut Vec<[[f32; 3]; 3]>,
    stack: &mut Vec<u32>,
) -> Result<(), ThreeMfError> {
    if stack.contains(id) {
        return Err(ThreeMfError::Model("component cycle".to_string()));
    }
    let obj = objects
        .iter()
        .find(|o| o.id == *id)
        .ok_or_else(|| ThreeMfError::Model(format!("component references unknown object {id}")))?;
    stack.push(*id);
    let mirrored = matrix.determinant() < 0.0;
    for tri in &obj.triangles {
        let pa = matrix.apply(obj.vertices[tri[0]]);
        let mut pb = matrix.apply(obj.vertices[tri[1]]);
        let mut pc = matrix.apply(obj.vertices[tri[2]]);
        if mirrored {
            std::mem::swap(&mut pb, &mut pc);
        }
        out.push([
            [pa[0] as f32, pa[1] as f32, pa[2] as f32],
            [pb[0] as f32, pb[1] as f32, pb[2] as f32],
            [pc[0] as f32, pc[1] as f32, pc[2] as f32],
        ]);
    }
    for (target, m) in &obj.components {
        let combined = matrix.then(&Matrix { m: *m });
        collect(target, objects, &combined, out, stack)?;
    }
    stack.pop();
    Ok(())
}

/// Parses the 12-number, row-vector `transform` attribute.
fn transform(s: &str) -> Result<Matrix, ThreeMfError> {
    let mut nums = [0.0_f64; 12];
    let mut i = 0;
    for tok in s.split_whitespace() {
        if i == 12 {
            return Err(ThreeMfError::Model(
                "transform needs 12 numbers".to_string(),
            ));
        }
        nums[i] = parse_num(tok)?;
        i += 1;
    }
    if i != 12 {
        return Err(ThreeMfError::Model(
            "transform needs 12 numbers".to_string(),
        ));
    }
    Ok(Matrix { m: nums })
}

fn parse_num(s: &str) -> Result<f64, ThreeMfError> {
    let f: f64 = s
        .parse()
        .map_err(|_| ThreeMfError::Model(format!("bad number \"{s}\"")))?;
    if !f.is_finite() {
        return Err(ThreeMfError::Model(format!("bad number \"{s}\"")));
    }
    Ok(f)
}

fn attr_num(el: Node, name: &str) -> Result<f64, ThreeMfError> {
    let s = el
        .attribute(name)
        .ok_or_else(|| ThreeMfError::Model(format!("missing {name}")))?;
    parse_num(s)
}

fn corner_index(el: Node, name: &str) -> Result<usize, ThreeMfError> {
    let s = el
        .attribute(name)
        .ok_or_else(|| ThreeMfError::Model(format!("missing {name}")))?;
    let f: f64 = s
        .parse()
        .map_err(|_| ThreeMfError::Model(format!("bad number \"{s}\"")))?;
    if !f.is_finite() || f < 0.0 || f.fract() != 0.0 {
        return Err(ThreeMfError::Model(format!("bad number \"{s}\"")));
    }
    Ok(f as usize)
}

fn attr_usize(el: Node, name: &str, what: &str) -> Result<u32, ThreeMfError> {
    let s = el
        .attribute(name)
        .ok_or_else(|| ThreeMfError::Model(format!("{what} element is missing {name}")))?;
    let f: f64 = s
        .parse()
        .map_err(|_| ThreeMfError::Model(format!("bad number \"{s}\"")))?;
    if !f.is_finite() || f < 0.0 || f.fract() != 0.0 {
        return Err(ThreeMfError::Model(format!("bad number \"{s}\"")));
    }
    Ok(f as u32)
}
