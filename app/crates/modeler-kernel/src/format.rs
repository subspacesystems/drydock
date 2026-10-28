// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The native model file: a versioned text format, read and written exactly.
//!
//! Format: `docs/format.md`. Behaviour: `docs/specs/m1-format.md`. The reader builds the
//! model through `Model::apply`, so a file can only name things that already exist.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use glam::{DMat4, DVec2};

use crate::model::{
    Body, BodyId, DefinitionId, DefinitionInfo, FaceMaterials, Instance, InstanceId, Material,
    MaterialId, Model, ModelError, ModelRecord, Texture,
};
use crate::paint::Side;
use crate::sun::{Geolocation, LocalTime, ShadowSettings};
use crate::tape::{Guide, GuideId};
use crate::topology::{
    Edge, EdgeId, EdgeUse, Face, FaceId, Loop, Record, TopoError, Vertex, VertexId,
};
use crate::units::{LengthFormat, Unit};

/// The version this code writes, and the newest it reads.
pub const FORMAT_VERSION: u32 = 5;

/// Why a model can't be written. Nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// A coordinate, color or transform entry is NaN or infinite.
    NonFinite,
}

/// Why a file's text can't be read, and the 1-based line where that was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
    pub line: usize,
    pub kind: ReadErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadErrorKind {
    /// The first line isn't `modeler-model <version>`.
    NotAModelFile,
    /// The header names a version this reader doesn't know.
    UnsupportedVersion(u32),
    /// The line's first token isn't a known record name.
    UnknownRecord,
    /// Wrong number of tokens, a malformed number, string or edge use, or a non-finite number.
    BadField,
    /// A vertex, edge, face, paint or instance record before any `body` record.
    NoBody,
    /// A second `body` record for the same body.
    DuplicateBody,
    /// The record is well formed but the model refused it.
    Rejected(ModelError),
    /// The text ended without an `end` record.
    MissingEnd,
    /// Something other than blank lines after `end`.
    TrailingData,
}

/// Why `save` or `load` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileError {
    Io {
        kind: std::io::ErrorKind,
        message: String,
    },
    /// The file isn't valid UTF-8.
    NotUtf8,
    Read(ReadError),
    Write(WriteError),
}

fn check_finite(value: f64) -> Result<(), WriteError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(WriteError::NonFinite)
    }
}

fn push_quoted(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_edge_use(out: &mut String, use_: EdgeUse) {
    out.push(if use_.reversed { '-' } else { '+' });
    out.push_str(&use_.edge.0.to_string());
}

fn push_optional_id(out: &mut String, id: Option<u64>) {
    match id {
        Some(id) => out.push_str(&id.to_string()),
        None => out.push('-'),
    }
}

fn write_body(out: &mut String, id: BodyId, body: &Body) -> Result<(), WriteError> {
    match id {
        BodyId::Root => out.push_str("body root\n"),
        BodyId::Definition(definition) => {
            out.push_str(&format!("body {}\n", definition.0));
        }
    }

    for (vid, vertex) in body.geometry().vertices() {
        let p = vertex.position;
        check_finite(p.x)?;
        check_finite(p.y)?;
        check_finite(p.z)?;
        out.push_str(&format!("vertex {} {} {} {}\n", vid.0, p.x, p.y, p.z));
    }

    for (eid, edge) in body.geometry().edges() {
        out.push_str(&format!(
            "edge {} {} {}\n",
            eid.0, edge.ends[0].0, edge.ends[1].0
        ));
    }

    for (fid, face) in body.geometry().faces() {
        out.push_str(&format!("face {}", fid.0));
        for use_ in &face.outer.uses {
            out.push(' ');
            push_edge_use(out, *use_);
        }
        for inner in &face.inner {
            out.push_str(" |");
            for use_ in &inner.uses {
                out.push(' ');
                push_edge_use(out, *use_);
            }
        }
        out.push('\n');
    }

    for (fid, materials) in body.face_material_entries() {
        if body.geometry().face(fid).is_none() {
            continue;
        }
        if materials.front.is_none() && materials.back.is_none() {
            continue;
        }
        out.push_str(&format!("paint {}", fid.0));
        out.push(' ');
        push_optional_id(out, materials.front.map(|m| m.0));
        out.push(' ');
        push_optional_id(out, materials.back.map(|m| m.0));
        out.push('\n');
    }

    for (fid, materials) in body.face_material_entries() {
        if body.geometry().face(fid).is_none() {
            continue;
        }
        for (side, map) in [("front", materials.front_map), ("back", materials.back_map)] {
            let Some(map) = map else {
                continue;
            };
            for value in map.u.to_array().into_iter().chain(map.v.to_array()) {
                check_finite(value)?;
            }
            out.push_str(&format!(
                "texmap {} {} {} {} {} {} {} {} {} {}\n",
                fid.0, side, map.u.x, map.u.y, map.u.z, map.u.w, map.v.x, map.v.y, map.v.z, map.v.w
            ));
        }
    }

    for (iid, instance) in body.instances() {
        let cols = instance.transform.to_cols_array();
        for value in cols {
            check_finite(value)?;
        }
        out.push_str(&format!("instance {} {} ", iid.0, instance.definition.0));
        push_optional_id(out, instance.material.map(|m| m.0));
        for value in cols {
            out.push(' ');
            out.push_str(&value.to_string());
        }
        out.push(' ');
        push_quoted(out, &instance.name);
        out.push('\n');
    }

    for (gid, guide) in body.guides().iter() {
        match guide {
            Guide::Line { point, dir } => {
                check_finite(point.x)?;
                check_finite(point.y)?;
                check_finite(point.z)?;
                check_finite(dir.x)?;
                check_finite(dir.y)?;
                check_finite(dir.z)?;
                out.push_str(&format!(
                    "guide {} line {} {} {} {} {} {}\n",
                    gid.0, point.x, point.y, point.z, dir.x, dir.y, dir.z
                ));
            }
            Guide::Point(point) => {
                check_finite(point.x)?;
                check_finite(point.y)?;
                check_finite(point.z)?;
                out.push_str(&format!(
                    "guide {} point {} {} {}\n",
                    gid.0, point.x, point.y, point.z
                ));
            }
        }
    }

    Ok(())
}

/// The model as file text.
pub fn to_text(model: &Model) -> Result<String, WriteError> {
    let bodies = || {
        std::iter::once(model.root()).chain(
            model
                .definitions()
                .filter_map(|(id, _)| model.body(BodyId::Definition(id))),
        )
    };
    let needs_three = model.materials().any(|(_, material)| {
        material
            .texture
            .as_ref()
            .is_some_and(|texture| texture.size != DVec2::ONE)
    }) || bodies().any(|body| {
        body.face_material_entries().any(|(face, materials)| {
            body.geometry().face(face).is_some()
                && (materials.front_map.is_some() || materials.back_map.is_some())
        })
    });
    let has_guides = bodies().any(|body| !body.guides().is_empty());
    let has_settings = model.geolocation() != Geolocation::default()
        || model.shadow_settings() != ShadowSettings::default();
    let has_units = model.length_format() != LengthFormat::default();
    let version = if has_units {
        5
    } else if has_settings {
        4
    } else if needs_three {
        3
    } else if has_guides {
        2
    } else {
        1
    };
    let mut out = String::new();
    out.push_str(&format!("modeler-model {version}\n"));

    if model.geolocation() != Geolocation::default() {
        let g = model.geolocation();
        check_finite(g.latitude)?;
        check_finite(g.longitude)?;
        check_finite(g.north_angle)?;
        check_finite(g.utc_offset)?;
        out.push_str(&format!(
            "geolocation {} {} {} {}\n",
            g.latitude, g.longitude, g.north_angle, g.utc_offset
        ));
    }
    if model.shadow_settings() != ShadowSettings::default() {
        let s = model.shadow_settings();
        check_finite(s.light)?;
        check_finite(s.dark)?;
        let flag = |on: bool| if on { "on" } else { "off" };
        out.push_str(&format!(
            "sun {} {} {} {} {} {} {} {} {} {}\n",
            s.time.year,
            s.time.month,
            s.time.day,
            s.time.hour,
            s.time.minute,
            flag(s.shadows),
            flag(s.sun_shading),
            s.light,
            s.dark,
            flag(s.on_ground)
        ));
    }

    if has_units {
        match model.length_format() {
            LengthFormat::Decimal { unit, decimals } => {
                let unit = match unit {
                    Unit::Millimeter => "mm",
                    Unit::Centimeter => "cm",
                    Unit::Meter => "m",
                    Unit::Inch => "in",
                    Unit::Foot => "ft",
                };
                out.push_str(&format!("units decimal {unit} {decimals}\n"));
            }
            LengthFormat::Architectural { denominator } => {
                out.push_str(&format!("units architectural {denominator}\n"));
            }
        }
    }

    for (id, material) in model.materials() {
        for component in material.color {
            check_finite(component as f64)?;
        }
        out.push_str(&format!(
            "material {} {} {} {} {} ",
            id.0, material.color[0], material.color[1], material.color[2], material.color[3]
        ));
        push_quoted(&mut out, &material.name);
        out.push(' ');
        match &material.texture {
            Some(texture) => {
                check_finite(texture.size.x)?;
                check_finite(texture.size.y)?;
                push_quoted(&mut out, &texture.path);
                if version >= 3 {
                    out.push(' ');
                    out.push_str(&texture.size.x.to_string());
                    out.push(' ');
                    out.push_str(&texture.size.y.to_string());
                }
            }
            None => out.push('-'),
        }
        out.push('\n');
    }

    for (id, info) in model.definitions() {
        out.push_str(&format!(
            "definition {} {} ",
            id.0,
            if info.group { "group" } else { "component" }
        ));
        push_quoted(&mut out, &info.name);
        out.push('\n');
    }

    write_body(&mut out, BodyId::Root, model.root())?;
    for (id, _) in model.definitions() {
        let body = model
            .body(BodyId::Definition(id))
            .expect("definition bodies always exist");
        write_body(&mut out, BodyId::Definition(id), body)?;
    }

    out.push_str("end\n");
    Ok(out)
}

enum Token {
    Bare(String),
    Quoted(String),
}

fn tokenize(line: &str) -> Result<Vec<Token>, ReadErrorKind> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    loop {
        while i < n && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        if i >= n {
            break;
        }

        if chars[i] == '"' {
            i += 1;
            let mut text = String::new();
            loop {
                if i >= n {
                    return Err(ReadErrorKind::BadField);
                }
                let c = chars[i];
                if c == '"' {
                    i += 1;
                    if i < n && chars[i] != ' ' && chars[i] != '\t' {
                        return Err(ReadErrorKind::BadField);
                    }
                    break;
                }
                if c == '\\' {
                    i += 1;
                    if i >= n {
                        return Err(ReadErrorKind::BadField);
                    }
                    match chars[i] {
                        '\\' => text.push('\\'),
                        '"' => text.push('"'),
                        'n' => text.push('\n'),
                        'r' => text.push('\r'),
                        't' => text.push('\t'),
                        'u' => {
                            i += 1;
                            if i >= n || chars[i] != '{' {
                                return Err(ReadErrorKind::BadField);
                            }
                            i += 1;
                            let start = i;
                            while i < n && chars[i].is_ascii_hexdigit() {
                                i += 1;
                            }
                            let digits = i - start;
                            if !(1..=6).contains(&digits) || i >= n || chars[i] != '}' {
                                return Err(ReadErrorKind::BadField);
                            }
                            let hex: String = chars[start..i].iter().collect();
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| ReadErrorKind::BadField)?;
                            let c = char::from_u32(code).ok_or(ReadErrorKind::BadField)?;
                            text.push(c);
                        }
                        _ => return Err(ReadErrorKind::BadField),
                    }
                    i += 1;
                } else {
                    text.push(c);
                    i += 1;
                }
            }
            tokens.push(Token::Quoted(text));
        } else {
            let start = i;
            while i < n && chars[i] != ' ' && chars[i] != '\t' {
                if chars[i] == '"' {
                    return Err(ReadErrorKind::BadField);
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token::Bare(text));
        }
    }

    Ok(tokens)
}

fn expect_count(tokens: &[Token], count: usize) -> Result<(), ReadErrorKind> {
    if tokens.len() == count {
        Ok(())
    } else {
        Err(ReadErrorKind::BadField)
    }
}

fn bare(token: &Token) -> Result<&str, ReadErrorKind> {
    match token {
        Token::Bare(text) => Ok(text),
        Token::Quoted(_) => Err(ReadErrorKind::BadField),
    }
}

fn quoted(token: &Token) -> Result<&str, ReadErrorKind> {
    match token {
        Token::Quoted(text) => Ok(text),
        Token::Bare(_) => Err(ReadErrorKind::BadField),
    }
}

fn parse_u64(token: &Token) -> Result<u64, ReadErrorKind> {
    bare(token)?
        .parse::<u64>()
        .map_err(|_| ReadErrorKind::BadField)
}

fn parse_f64(token: &Token) -> Result<f64, ReadErrorKind> {
    let value = bare(token)?
        .parse::<f64>()
        .map_err(|_| ReadErrorKind::BadField)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ReadErrorKind::BadField)
    }
}

fn parse_f32(token: &Token) -> Result<f32, ReadErrorKind> {
    let value = bare(token)?
        .parse::<f32>()
        .map_err(|_| ReadErrorKind::BadField)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ReadErrorKind::BadField)
    }
}

fn parse_i32(token: &Token) -> Result<i32, ReadErrorKind> {
    bare(token)?
        .parse::<i32>()
        .map_err(|_| ReadErrorKind::BadField)
}

fn parse_u32(token: &Token) -> Result<u32, ReadErrorKind> {
    bare(token)?
        .parse::<u32>()
        .map_err(|_| ReadErrorKind::BadField)
}

fn parse_flag(token: &Token) -> Result<bool, ReadErrorKind> {
    match bare(token)? {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(ReadErrorKind::BadField),
    }
}

fn parse_optional_id(token: &Token) -> Result<Option<u64>, ReadErrorKind> {
    match bare(token)? {
        "-" => Ok(None),
        text => text
            .parse::<u64>()
            .map(Some)
            .map_err(|_| ReadErrorKind::BadField),
    }
}

fn parse_edge_use(token: &Token) -> Result<EdgeUse, ReadErrorKind> {
    let text = bare(token)?;
    let (reversed, digits) = match text.as_bytes().first() {
        Some(b'+') => (false, &text[1..]),
        Some(b'-') => (true, &text[1..]),
        _ => return Err(ReadErrorKind::BadField),
    };
    let edge = digits.parse::<u64>().map_err(|_| ReadErrorKind::BadField)?;
    Ok(EdgeUse {
        edge: EdgeId(edge),
        reversed,
    })
}

fn rejected(error: ModelError) -> ReadErrorKind {
    ReadErrorKind::Rejected(error)
}

fn apply_line(
    model: &mut Model,
    version: u32,
    current: &mut Option<BodyId>,
    seen: &mut BTreeSet<BodyId>,
    settings_seen: &mut BTreeSet<&'static str>,
    tokens: &[Token],
) -> Result<bool, ReadErrorKind> {
    let name = match tokens.first() {
        Some(Token::Bare(name)) => name.as_str(),
        _ => return Err(ReadErrorKind::UnknownRecord),
    };

    match name {
        "end" => {
            expect_count(tokens, 1)?;
            Ok(true)
        }
        "material" => {
            if version >= 3 {
                if tokens.len() != 8 && tokens.len() != 10 {
                    return Err(ReadErrorKind::BadField);
                }
            } else {
                expect_count(tokens, 8)?;
            }
            let id = MaterialId(parse_u64(&tokens[1])?);
            let color = [
                parse_f32(&tokens[2])?,
                parse_f32(&tokens[3])?,
                parse_f32(&tokens[4])?,
                parse_f32(&tokens[5])?,
            ];
            let name = quoted(&tokens[6])?.to_string();
            let texture = if version >= 3 {
                match (&tokens[7], tokens.len()) {
                    (Token::Quoted(path), 10) => Some(Texture::new(
                        path,
                        DVec2::new(parse_f64(&tokens[8])?, parse_f64(&tokens[9])?),
                    )),
                    (Token::Bare(text), 8) if text == "-" => None,
                    _ => return Err(ReadErrorKind::BadField),
                }
            } else {
                match &tokens[7] {
                    Token::Quoted(path) => Some(Texture::from(path.as_str())),
                    Token::Bare(text) if text == "-" => None,
                    Token::Bare(_) => return Err(ReadErrorKind::BadField),
                }
            };
            let material = Material {
                name,
                color,
                texture,
            };
            model
                .apply(&ModelRecord::AddMaterial(id, material))
                .map_err(rejected)?;
            Ok(false)
        }
        "definition" => {
            expect_count(tokens, 4)?;
            let id = DefinitionId(parse_u64(&tokens[1])?);
            let group = match bare(&tokens[2])? {
                "component" => false,
                "group" => true,
                _ => return Err(ReadErrorKind::BadField),
            };
            let name = quoted(&tokens[3])?.to_string();
            model
                .apply(&ModelRecord::AddDefinition(
                    id,
                    DefinitionInfo { name, group },
                ))
                .map_err(rejected)?;
            Ok(false)
        }
        "body" => {
            expect_count(tokens, 2)?;
            let text = bare(&tokens[1])?;
            let body = if text == "root" {
                BodyId::Root
            } else {
                let id = text.parse::<u64>().map_err(|_| ReadErrorKind::BadField)?;
                BodyId::Definition(DefinitionId(id))
            };
            if !seen.insert(body) {
                return Err(ReadErrorKind::DuplicateBody);
            }
            if let BodyId::Definition(id) = body
                && model.definition(id).is_none()
            {
                return Err(rejected(ModelError::MissingBody(body)));
            }
            *current = Some(body);
            Ok(false)
        }
        "vertex" => {
            expect_count(tokens, 5)?;
            let id = VertexId(parse_u64(&tokens[1])?);
            let position = glam::DVec3::new(
                parse_f64(&tokens[2])?,
                parse_f64(&tokens[3])?,
                parse_f64(&tokens[4])?,
            );
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            model
                .apply(&ModelRecord::Geometry(
                    body,
                    Record::AddVertex(id, Vertex { position }),
                ))
                .map_err(rejected)?;
            Ok(false)
        }
        "edge" => {
            expect_count(tokens, 4)?;
            let id = EdgeId(parse_u64(&tokens[1])?);
            let ends = [
                VertexId(parse_u64(&tokens[2])?),
                VertexId(parse_u64(&tokens[3])?),
            ];
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            model
                .apply(&ModelRecord::Geometry(
                    body,
                    Record::AddEdge(id, Edge { ends }),
                ))
                .map_err(rejected)?;
            Ok(false)
        }
        "face" => {
            if tokens.len() < 2 {
                return Err(ReadErrorKind::BadField);
            }
            let id = FaceId(parse_u64(&tokens[1])?);
            let mut loops = vec![Vec::new()];
            for token in &tokens[2..] {
                let text = bare(token)?;
                if text == "|" {
                    loops.push(Vec::new());
                } else {
                    loops
                        .last_mut()
                        .expect("loops always has at least one")
                        .push(parse_edge_use(token)?);
                }
            }
            let outer = Loop {
                uses: loops.remove(0),
            };
            let inner = loops.into_iter().map(|uses| Loop { uses }).collect();
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            model
                .apply(&ModelRecord::Geometry(
                    body,
                    Record::AddFace(id, Face { outer, inner }),
                ))
                .map_err(rejected)?;
            Ok(false)
        }
        "paint" => {
            expect_count(tokens, 4)?;
            let face = FaceId(parse_u64(&tokens[1])?);
            let front = parse_optional_id(&tokens[2])?.map(MaterialId);
            let back = parse_optional_id(&tokens[3])?.map(MaterialId);
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            if model
                .body(body)
                .is_none_or(|b| b.geometry().face(face).is_none())
            {
                return Err(rejected(ModelError::Topo(TopoError::MissingFace(face))));
            }
            let from = model
                .body(body)
                .expect("current body exists")
                .face_materials(face);
            model
                .apply(&ModelRecord::SetFaceMaterials {
                    body,
                    face,
                    from,
                    to: FaceMaterials {
                        front,
                        back,
                        ..from
                    },
                })
                .map_err(rejected)?;
            Ok(false)
        }
        "texmap" => {
            if version < 3 {
                return Err(ReadErrorKind::UnknownRecord);
            }
            expect_count(tokens, 11)?;
            let face = FaceId(parse_u64(&tokens[1])?);
            let side = match bare(&tokens[2])? {
                "front" => Side::Front,
                "back" => Side::Back,
                _ => return Err(ReadErrorKind::BadField),
            };
            let u = glam::DVec4::new(
                parse_f64(&tokens[3])?,
                parse_f64(&tokens[4])?,
                parse_f64(&tokens[5])?,
                parse_f64(&tokens[6])?,
            );
            let v = glam::DVec4::new(
                parse_f64(&tokens[7])?,
                parse_f64(&tokens[8])?,
                parse_f64(&tokens[9])?,
                parse_f64(&tokens[10])?,
            );
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            if model
                .body(body)
                .is_none_or(|b| b.geometry().face(face).is_none())
            {
                return Err(rejected(ModelError::Topo(TopoError::MissingFace(face))));
            }
            let from = model
                .body(body)
                .expect("current body exists")
                .face_materials(face);
            let map = crate::texture::TextureMap { u, v };
            let to = match side {
                Side::Front => FaceMaterials {
                    front_map: Some(map),
                    ..from
                },
                Side::Back => FaceMaterials {
                    back_map: Some(map),
                    ..from
                },
            };
            model
                .apply(&ModelRecord::SetFaceMaterials {
                    body,
                    face,
                    from,
                    to,
                })
                .map_err(rejected)?;
            Ok(false)
        }
        "instance" => {
            expect_count(tokens, 21)?;
            let id = InstanceId(parse_u64(&tokens[1])?);
            let definition = DefinitionId(parse_u64(&tokens[2])?);
            let material = parse_optional_id(&tokens[3])?.map(MaterialId);
            let mut cols = [0.0f64; 16];
            for (index, value) in cols.iter_mut().enumerate() {
                *value = parse_f64(&tokens[4 + index])?;
            }
            let name = quoted(&tokens[20])?.to_string();
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            let instance = Instance {
                definition,
                transform: DMat4::from_cols_array(&cols),
                name,
                material,
            };
            model
                .apply(&ModelRecord::AddInstance(body, id, instance))
                .map_err(rejected)?;
            Ok(false)
        }
        "guide" => {
            if version < 2 {
                return Err(ReadErrorKind::UnknownRecord);
            }
            if tokens.len() < 3 {
                return Err(ReadErrorKind::BadField);
            }
            let id = GuideId(parse_u64(&tokens[1])?);
            let kind = bare(&tokens[2])?;
            let guide = match kind {
                "line" => {
                    expect_count(tokens, 9)?;
                    Guide::Line {
                        point: glam::DVec3::new(
                            parse_f64(&tokens[3])?,
                            parse_f64(&tokens[4])?,
                            parse_f64(&tokens[5])?,
                        ),
                        dir: glam::DVec3::new(
                            parse_f64(&tokens[6])?,
                            parse_f64(&tokens[7])?,
                            parse_f64(&tokens[8])?,
                        ),
                    }
                }
                "point" => {
                    expect_count(tokens, 6)?;
                    Guide::Point(glam::DVec3::new(
                        parse_f64(&tokens[3])?,
                        parse_f64(&tokens[4])?,
                        parse_f64(&tokens[5])?,
                    ))
                }
                _ => return Err(ReadErrorKind::BadField),
            };
            let body = current.ok_or(ReadErrorKind::NoBody)?;
            model
                .apply(&ModelRecord::AddGuide(body, id, guide))
                .map_err(rejected)?;
            Ok(false)
        }
        "geolocation" => {
            if version < 4 {
                return Err(ReadErrorKind::UnknownRecord);
            }
            if !settings_seen.insert("geolocation") {
                return Err(ReadErrorKind::BadField);
            }
            expect_count(tokens, 5)?;
            let geolocation = Geolocation {
                latitude: parse_f64(&tokens[1])?,
                longitude: parse_f64(&tokens[2])?,
                north_angle: parse_f64(&tokens[3])?,
                utc_offset: parse_f64(&tokens[4])?,
            };
            let from = model.geolocation();
            model
                .apply(&ModelRecord::SetGeolocation {
                    from,
                    to: geolocation,
                })
                .map_err(rejected)?;
            Ok(false)
        }
        "units" => {
            if version < 5 {
                return Err(ReadErrorKind::UnknownRecord);
            }
            if !settings_seen.insert("units") {
                return Err(ReadErrorKind::BadField);
            }
            let format = match tokens.get(1).map(bare).transpose()? {
                Some("decimal") => {
                    expect_count(tokens, 4)?;
                    let unit = match bare(&tokens[2])? {
                        "mm" => Unit::Millimeter,
                        "cm" => Unit::Centimeter,
                        "m" => Unit::Meter,
                        "in" => Unit::Inch,
                        "ft" => Unit::Foot,
                        _ => return Err(ReadErrorKind::BadField),
                    };
                    let decimals = parse_u64(&tokens[3])?;
                    let decimals =
                        u8::try_from(decimals).map_err(|_| rejected(ModelError::BadSettings))?;
                    LengthFormat::Decimal { unit, decimals }
                }
                Some("architectural") => {
                    expect_count(tokens, 3)?;
                    let denominator = parse_u64(&tokens[2])?;
                    let denominator =
                        u8::try_from(denominator).map_err(|_| rejected(ModelError::BadSettings))?;
                    LengthFormat::Architectural { denominator }
                }
                Some(_) => return Err(ReadErrorKind::BadField),
                None => return Err(ReadErrorKind::BadField),
            };
            let from = model.length_format();
            model
                .apply(&ModelRecord::SetLengthFormat { from, to: format })
                .map_err(rejected)?;
            Ok(false)
        }
        "sun" => {
            if version < 4 {
                return Err(ReadErrorKind::UnknownRecord);
            }
            if !settings_seen.insert("sun") {
                return Err(ReadErrorKind::BadField);
            }
            expect_count(tokens, 11)?;
            let time = LocalTime {
                year: parse_i32(&tokens[1])?,
                month: parse_u32(&tokens[2])?,
                day: parse_u32(&tokens[3])?,
                hour: parse_u32(&tokens[4])?,
                minute: parse_u32(&tokens[5])?,
            };
            let settings = ShadowSettings {
                time,
                shadows: parse_flag(&tokens[6])?,
                sun_shading: parse_flag(&tokens[7])?,
                light: parse_f64(&tokens[8])?,
                dark: parse_f64(&tokens[9])?,
                on_ground: parse_flag(&tokens[10])?,
            };
            let from = model.shadow_settings();
            model
                .apply(&ModelRecord::SetShadows { from, to: settings })
                .map_err(rejected)?;
            Ok(false)
        }
        _ => Err(ReadErrorKind::UnknownRecord),
    }
}

fn is_blank(line: &str) -> bool {
    line.chars().all(|c| c == ' ' || c == '\t')
}

fn is_ignored(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']);
    rest.is_empty() || rest.starts_with('#')
}

/// Read a model from file text.
pub fn from_text(text: &str) -> Result<Model, ReadError> {
    let mut lines = text.lines().enumerate();
    let Some((_, header)) = lines.next() else {
        return Err(ReadError {
            line: 1,
            kind: ReadErrorKind::NotAModelFile,
        });
    };

    let header_tokens: Vec<&str> = header.split_whitespace().collect();
    if header_tokens.len() != 2 || header_tokens[0] != "modeler-model" {
        return Err(ReadError {
            line: 1,
            kind: ReadErrorKind::NotAModelFile,
        });
    }
    let version = header_tokens[1].parse::<u32>().map_err(|_| ReadError {
        line: 1,
        kind: ReadErrorKind::NotAModelFile,
    })?;
    if !(1..=FORMAT_VERSION).contains(&version) {
        return Err(ReadError {
            line: 1,
            kind: ReadErrorKind::UnsupportedVersion(version),
        });
    }

    let mut model = Model::new();
    let mut current = None;
    let mut seen = BTreeSet::new();
    let mut settings_seen = BTreeSet::new();
    let mut ended = false;

    for (index, line) in lines {
        let line_number = index + 1;
        if ended {
            if !is_blank(line) {
                return Err(ReadError {
                    line: line_number,
                    kind: ReadErrorKind::TrailingData,
                });
            }
            continue;
        }
        if is_ignored(line) {
            continue;
        }
        let tokens = tokenize(line).map_err(|kind| ReadError {
            line: line_number,
            kind,
        })?;
        if apply_line(
            &mut model,
            version,
            &mut current,
            &mut seen,
            &mut settings_seen,
            &tokens,
        )
        .map_err(|kind| ReadError {
            line: line_number,
            kind,
        })? {
            ended = true;
        }
    }

    if !ended {
        return Err(ReadError {
            line: text.lines().count(),
            kind: ReadErrorKind::MissingEnd,
        });
    }

    Ok(model)
}

fn io_error(error: std::io::Error) -> FileError {
    FileError::Io {
        kind: error.kind(),
        message: error.to_string(),
    }
}

/// Write the model to `path` without ever leaving a partial file under that name.
pub fn save(model: &Model, path: &Path) -> Result<(), FileError> {
    let text = to_text(model).map_err(FileError::Write)?;

    let Some(file_name) = path.file_name() else {
        return Err(FileError::Io {
            kind: std::io::ErrorKind::InvalidInput,
            message: "path has no file name".to_string(),
        });
    };
    let mut temp_name = file_name.to_os_string();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);

    let write_result = (|| -> std::io::Result<()> {
        let mut file = File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp);
        return Err(io_error(error));
    }

    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(io_error(error));
    }

    #[cfg(unix)]
    if let Some(parent) = path.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }

    Ok(())
}

/// Read a model file.
pub fn load(path: &Path) -> Result<Model, FileError> {
    let bytes = std::fs::read(path).map_err(io_error)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| FileError::NotUtf8)?;
    from_text(text).map_err(FileError::Read)
}
