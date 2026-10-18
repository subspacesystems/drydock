// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Reading a COLLADA 1.4 document into the public [`Model`] types.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::{DMat4, DVec3};
use roxmltree::{Document, Node as XNode};

use crate::{
    Definition, GeometryInstance, ImportError, Material, Mesh, Model, Node, Primitive,
    PrimitiveKind, Projection, UpAxis, ViewCamera,
};

type Result<T> = std::result::Result<T, ImportError>;

fn invalid(message: impl Into<String>) -> ImportError {
    ImportError::Invalid(message.into())
}

fn elements<'a, 'input>(node: XNode<'a, 'input>) -> impl Iterator<Item = XNode<'a, 'input>> {
    node.children().filter(|child| child.is_element())
}

fn child<'a, 'input>(node: XNode<'a, 'input>, name: &str) -> Option<XNode<'a, 'input>> {
    elements(node).find(|child| child.tag_name().name() == name)
}

fn children<'a, 'input>(
    node: XNode<'a, 'input>,
    name: &str,
) -> impl Iterator<Item = XNode<'a, 'input>> {
    elements(node).filter(move |child| child.tag_name().name() == name)
}

fn libraries<'a, 'input>(root: XNode<'a, 'input>, name: &str) -> Vec<XNode<'a, 'input>> {
    children(root, name).collect()
}

fn text_of(node: XNode<'_, '_>) -> String {
    let mut text = String::new();
    for descendant in node.descendants().filter(|n| n.is_text()) {
        if let Some(part) = descendant.text() {
            text.push_str(part);
        }
    }
    text
}

fn text_trimmed(node: XNode<'_, '_>) -> String {
    text_of(node).trim().to_string()
}

fn attr<'a>(node: XNode<'a, '_>, name: &str) -> &'a str {
    node.attribute(name).unwrap_or("")
}

fn parse_f32(text: &str) -> Result<Vec<f32>> {
    text.split_whitespace()
        .map(|token| {
            token
                .parse::<f32>()
                .map_err(|_| invalid(format!("bad number \"{token}\"")))
        })
        .collect()
}

fn parse_f64_vec(text: &str) -> Result<Vec<f64>> {
    text.split_whitespace()
        .map(|token| {
            token
                .parse::<f64>()
                .map_err(|_| invalid(format!("bad number \"{token}\"")))
        })
        .collect()
}

fn parse_usize_vec(text: &str) -> Result<Vec<usize>> {
    text.split_whitespace()
        .map(|token| {
            token
                .parse::<usize>()
                .map_err(|_| invalid(format!("bad index \"{token}\"")))
        })
        .collect()
}

fn strip_hash(value: &str) -> &str {
    value.strip_prefix('#').unwrap_or(value)
}

fn f64_array<const N: usize>(values: &[f64], default: f64) -> [f64; N] {
    std::array::from_fn(|i| values.get(i).copied().unwrap_or(default))
}

fn up_axis_from(value: &str) -> UpAxis {
    match value.trim() {
        "X_UP" => UpAxis::X,
        "Z_UP" => UpAxis::Z,
        _ => UpAxis::Y,
    }
}

fn parse_asset(root: XNode<'_, '_>) -> (f64, UpAxis) {
    let Some(asset) = child(root, "asset") else {
        return (1.0, UpAxis::Y);
    };
    let unit_meters = child(asset, "unit")
        .and_then(|unit| unit.attribute("meter"))
        .and_then(|meter| meter.trim().parse::<f64>().ok())
        .unwrap_or(1.0);
    let up_axis = child(asset, "up_axis")
        .map(text_trimmed)
        .map(|value| up_axis_from(&value))
        .unwrap_or(UpAxis::Y);
    (unit_meters, up_axis)
}

struct Source {
    count: usize,
    stride: usize,
    offset: usize,
    values: Vec<f32>,
}

impl Source {
    fn read2(&self, index: usize) -> Option<[f32; 2]> {
        let start = self.start(index)?;
        Some([*self.values.get(start)?, *self.values.get(start + 1)?])
    }

    fn read3(&self, index: usize) -> Option<[f32; 3]> {
        let start = self.start(index)?;
        Some([
            *self.values.get(start)?,
            *self.values.get(start + 1)?,
            *self.values.get(start + 2)?,
        ])
    }

    fn start(&self, index: usize) -> Option<usize> {
        if index >= self.count || self.stride == 0 {
            return None;
        }
        Some(self.offset + index * self.stride)
    }
}

fn parse_source(node: XNode<'_, '_>) -> Result<Source> {
    let values = child(node, "float_array")
        .map(|array| parse_f32(&text_of(array)))
        .transpose()?
        .unwrap_or_default();
    let accessor = child(node, "technique_common").and_then(|tc| child(tc, "accessor"));
    let stride = accessor
        .and_then(|a| a.attribute("stride"))
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(1);
    let offset = accessor
        .and_then(|a| a.attribute("offset"))
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let count = accessor
        .and_then(|a| a.attribute("count"))
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or_else(|| {
            values
                .len()
                .saturating_sub(offset)
                .checked_div(stride)
                .unwrap_or(0)
        });
    Ok(Source {
        count,
        stride,
        offset,
        values,
    })
}

#[derive(Clone)]
struct VertInput {
    semantic: String,
    source: String,
    set: Option<usize>,
}

#[derive(Clone)]
struct Vertices {
    id: String,
    inputs: Vec<VertInput>,
}

fn parse_vertices(node: XNode<'_, '_>) -> Result<Vertices> {
    let mut inputs = Vec::new();
    for input in children(node, "input") {
        inputs.push(VertInput {
            semantic: attr(input, "semantic").to_string(),
            source: strip_hash(attr(input, "source")).to_string(),
            set: input
                .attribute("set")
                .and_then(|s| s.trim().parse::<usize>().ok()),
        });
    }
    Ok(Vertices {
        id: attr(node, "id").to_string(),
        inputs,
    })
}

#[derive(Clone)]
struct Bound {
    source: String,
    offset: usize,
    set: Option<usize>,
}

struct Reindexer<'a> {
    geometry_id: &'a str,
    sources: &'a HashMap<String, Source>,
    p: &'a [usize],
    slots: usize,
    position: Option<&'a Bound>,
    normal: Option<&'a Bound>,
    texcoord: Option<&'a Bound>,
    primitive: Primitive,
    seen: HashMap<(usize, Option<usize>, Option<usize>), u32>,
}

impl Reindexer<'_> {
    fn vertex(&mut self, group: usize) -> Result<u32> {
        let base = group * self.slots;
        let position = self.position.expect("position input");
        let position_index = self.index(base, position, "position")?;
        let position_source = self.source(position, "position")?;
        let position_value = position_source.read3(position_index).ok_or_else(|| {
            invalid(format!(
                "geometry \"{}\": position index {position_index} is outside source",
                self.geometry_id
            ))
        })?;

        let normal = match self.normal {
            None => None,
            Some(bound) => {
                let index = self.index(base, bound, "normal")?;
                let source = self.source(bound, "normal")?;
                let value = source.read3(index).ok_or_else(|| {
                    invalid(format!(
                        "geometry \"{}\": normal index {index} is outside source",
                        self.geometry_id
                    ))
                })?;
                Some((index, value))
            }
        };

        let texcoord = match self.texcoord {
            None => None,
            Some(bound) => {
                let index = self.index(base, bound, "texcoord")?;
                let source = self.source(bound, "texcoord")?;
                let value = source.read2(index).ok_or_else(|| {
                    invalid(format!(
                        "geometry \"{}\": texcoord index {index} is outside source",
                        self.geometry_id
                    ))
                })?;
                Some((index, value))
            }
        };

        let key = (
            position_index,
            normal.as_ref().map(|(index, _)| *index),
            texcoord.as_ref().map(|(index, _)| *index),
        );
        if let Some(index) = self.seen.get(&key) {
            return Ok(*index);
        }

        let output = self.primitive.positions.len() as u32;
        self.primitive.positions.push(position_value);
        if let Some((_, value)) = normal {
            self.primitive.normals.push(value);
        }
        if let Some((_, value)) = texcoord {
            self.primitive.texcoords.push(value);
        }
        self.seen.insert(key, output);
        Ok(output)
    }

    fn index(&self, base: usize, bound: &Bound, what: &str) -> Result<usize> {
        self.p.get(base + bound.offset).copied().ok_or_else(|| {
            invalid(format!(
                "geometry \"{}\": short <p> list for {what}",
                self.geometry_id
            ))
        })
    }

    fn source<'a>(&'a self, bound: &Bound, what: &str) -> Result<&'a Source> {
        self.sources.get(&bound.source).ok_or_else(|| {
            invalid(format!(
                "geometry \"{}\": {what} input source \"#{}\" not found",
                self.geometry_id, bound.source
            ))
        })
    }
}

fn parse_primitive(
    node: XNode<'_, '_>,
    kind: PrimitiveKind,
    sources: &HashMap<String, Source>,
    vertices: &[Vertices],
    geometry_id: &str,
) -> Result<Primitive> {
    let material_symbol = node.attribute("material").map(str::to_string);
    let mut position = None;
    let mut normal = None;
    let mut texcoords = Vec::new();
    let mut max_offset = 0usize;

    for input in children(node, "input") {
        let semantic = attr(input, "semantic");
        let offset = input
            .attribute("offset")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0);
        max_offset = max_offset.max(offset);

        match semantic {
            "VERTEX" => {
                let source = strip_hash(attr(input, "source"));
                let vertex_set = vertices
                    .iter()
                    .find(|v| v.id == source)
                    .or_else(|| vertices.first());
                let Some(vertex_set) = vertex_set else {
                    continue;
                };
                for vert in &vertex_set.inputs {
                    let bound = Bound {
                        source: vert.source.clone(),
                        offset,
                        set: vert.set,
                    };
                    match vert.semantic.as_str() {
                        "POSITION" if position.is_none() => position = Some(bound),
                        "NORMAL" if normal.is_none() => normal = Some(bound),
                        "TEXCOORD" => texcoords.push(bound),
                        _ => {}
                    }
                }
            }
            "NORMAL" if normal.is_none() => {
                normal = Some(Bound {
                    source: strip_hash(attr(input, "source")).to_string(),
                    offset,
                    set: input
                        .attribute("set")
                        .and_then(|s| s.trim().parse::<usize>().ok()),
                });
            }
            "TEXCOORD" => texcoords.push(Bound {
                source: strip_hash(attr(input, "source")).to_string(),
                offset,
                set: input
                    .attribute("set")
                    .and_then(|s| s.trim().parse::<usize>().ok()),
            }),
            _ => {}
        }
    }

    let Some(position) = position else {
        return Err(invalid(format!(
            "geometry \"{geometry_id}\": primitive has no position input"
        )));
    };

    let texcoord = if texcoords.iter().any(|b| b.set.is_some()) {
        texcoords
            .iter()
            .filter_map(|b| b.set.map(|set| (set, b)))
            .min_by_key(|(set, _)| *set)
            .map(|(_, b)| b)
    } else {
        texcoords.first()
    };

    let slots = max_offset + 1;
    let p_text = child(node, "p").map(|p| text_of(p)).unwrap_or_default();
    let p = parse_usize_vec(&p_text).map_err(|_| {
        invalid(format!(
            "geometry \"{geometry_id}\": invalid <p> index list"
        ))
    })?;
    if !p.len().is_multiple_of(slots) {
        return Err(invalid(format!(
            "geometry \"{geometry_id}\": <p> length {} is not a whole number of primitives",
            p.len()
        )));
    }
    let groups = p.len() / slots;

    let mut reindexer = Reindexer {
        geometry_id,
        sources,
        p: &p,
        slots,
        position: Some(&position),
        normal: normal.as_ref(),
        texcoord,
        primitive: Primitive {
            kind,
            material_symbol,
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            indices: Vec::new(),
        },
        seen: HashMap::new(),
    };

    match kind {
        PrimitiveKind::Triangles => {
            if !groups.is_multiple_of(3) {
                return Err(invalid(format!(
                    "geometry \"{geometry_id}\": <p> length {} is not a whole number of triangles",
                    p.len()
                )));
            }
            let mut group = 0;
            while group < groups {
                for offset in 0..3 {
                    let index = reindexer.vertex(group + offset)?;
                    reindexer.primitive.indices.push(index);
                }
                group += 3;
            }
        }
        PrimitiveKind::Lines => {
            if !groups.is_multiple_of(2) {
                return Err(invalid(format!(
                    "geometry \"{geometry_id}\": <p> length {} is not a whole number of lines",
                    p.len()
                )));
            }
            let mut group = 0;
            while group < groups {
                for offset in 0..2 {
                    let index = reindexer.vertex(group + offset)?;
                    reindexer.primitive.indices.push(index);
                }
                group += 2;
            }
        }
    }

    Ok(reindexer.primitive)
}

fn parse_polylist(
    node: XNode<'_, '_>,
    sources: &HashMap<String, Source>,
    vertices: &[Vertices],
    geometry_id: &str,
) -> Result<Primitive> {
    let material_symbol = node.attribute("material").map(str::to_string);
    let mut position = None;
    let mut normal = None;
    let mut texcoords = Vec::new();
    let mut max_offset = 0usize;

    for input in children(node, "input") {
        let semantic = attr(input, "semantic");
        let offset = input
            .attribute("offset")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0);
        max_offset = max_offset.max(offset);

        match semantic {
            "VERTEX" => {
                let source = strip_hash(attr(input, "source"));
                let vertex_set = vertices
                    .iter()
                    .find(|v| v.id == source)
                    .or_else(|| vertices.first());
                let Some(vertex_set) = vertex_set else {
                    continue;
                };
                for vert in &vertex_set.inputs {
                    let bound = Bound {
                        source: vert.source.clone(),
                        offset,
                        set: vert.set,
                    };
                    match vert.semantic.as_str() {
                        "POSITION" if position.is_none() => position = Some(bound),
                        "NORMAL" if normal.is_none() => normal = Some(bound),
                        "TEXCOORD" => texcoords.push(bound),
                        _ => {}
                    }
                }
            }
            "NORMAL" if normal.is_none() => {
                normal = Some(Bound {
                    source: strip_hash(attr(input, "source")).to_string(),
                    offset,
                    set: input
                        .attribute("set")
                        .and_then(|s| s.trim().parse::<usize>().ok()),
                });
            }
            "TEXCOORD" => texcoords.push(Bound {
                source: strip_hash(attr(input, "source")).to_string(),
                offset,
                set: input
                    .attribute("set")
                    .and_then(|s| s.trim().parse::<usize>().ok()),
            }),
            _ => {}
        }
    }

    let Some(position) = position else {
        return Err(invalid(format!(
            "geometry \"{geometry_id}\": primitive has no position input"
        )));
    };

    let texcoord = if texcoords.iter().any(|b| b.set.is_some()) {
        texcoords
            .iter()
            .filter_map(|b| b.set.map(|set| (set, b)))
            .min_by_key(|(set, _)| *set)
            .map(|(_, b)| b)
    } else {
        texcoords.first()
    };

    let slots = max_offset + 1;
    let vcount_text = child(node, "vcount")
        .map(|v| text_of(v))
        .unwrap_or_default();
    let vcount = parse_usize_vec(&vcount_text)
        .map_err(|_| invalid(format!("geometry \"{geometry_id}\": invalid <vcount> list")))?;
    let p_text = child(node, "p").map(|p| text_of(p)).unwrap_or_default();
    let p = parse_usize_vec(&p_text).map_err(|_| {
        invalid(format!(
            "geometry \"{geometry_id}\": invalid <p> index list"
        ))
    })?;
    if !p.len().is_multiple_of(slots) {
        return Err(invalid(format!(
            "geometry \"{geometry_id}\": <p> length {} is not a whole number of primitives",
            p.len()
        )));
    }
    let groups = p.len() / slots;
    let expected: usize = vcount.iter().sum();
    if groups != expected {
        return Err(invalid(format!(
            "geometry \"{geometry_id}\": <p> has {groups} vertices but <vcount> expects {expected}"
        )));
    }

    let mut reindexer = Reindexer {
        geometry_id,
        sources,
        p: &p,
        slots,
        position: Some(&position),
        normal: normal.as_ref(),
        texcoord,
        primitive: Primitive {
            kind: PrimitiveKind::Triangles,
            material_symbol,
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            indices: Vec::new(),
        },
        seen: HashMap::new(),
    };

    let mut group = 0;
    for count in vcount {
        let mut polygon = Vec::with_capacity(count);
        for _ in 0..count {
            polygon.push(reindexer.vertex(group)?);
            group += 1;
        }
        if count >= 3 {
            let first = polygon[0];
            for i in 1..polygon.len() - 1 {
                reindexer.primitive.indices.push(first);
                reindexer.primitive.indices.push(polygon[i]);
                reindexer.primitive.indices.push(polygon[i + 1]);
            }
        }
    }

    Ok(reindexer.primitive)
}

fn parse_mesh(node: XNode<'_, '_>, geometry_id: &str) -> Result<Vec<Primitive>> {
    let vertices: Vec<Vertices> = children(node, "vertices")
        .map(parse_vertices)
        .collect::<Result<_>>()?;
    if vertices.is_empty() {
        return Ok(Vec::new());
    }

    let mut sources = HashMap::new();
    for source in children(node, "source") {
        let id = attr(source, "id").to_string();
        if !id.is_empty() {
            sources.insert(id, parse_source(source)?);
        }
    }

    let mut primitives = Vec::new();
    for element in elements(node) {
        match element.tag_name().name() {
            "triangles" => primitives.push(parse_primitive(
                element,
                PrimitiveKind::Triangles,
                &sources,
                &vertices,
                geometry_id,
            )?),
            "lines" => primitives.push(parse_primitive(
                element,
                PrimitiveKind::Lines,
                &sources,
                &vertices,
                geometry_id,
            )?),
            "polylist" => {
                primitives.push(parse_polylist(element, &sources, &vertices, geometry_id)?)
            }
            _ => {}
        }
    }
    Ok(primitives)
}

fn parse_geometry(node: XNode<'_, '_>) -> Result<Mesh> {
    let id = attr(node, "id").to_string();
    let name = attr(node, "name").to_string();
    let primitives = match child(node, "mesh") {
        Some(mesh) => parse_mesh(mesh, &id)?,
        None => Vec::new(),
    };
    Ok(Mesh {
        id,
        name,
        primitives,
    })
}

fn newparams<'a, 'input>(profile: XNode<'a, 'input>) -> Vec<(String, XNode<'a, 'input>)> {
    children(profile, "newparam")
        .filter_map(|node| node.attribute("sid").map(|sid| (sid.to_string(), node)))
        .collect()
}

fn text_float(node: Option<XNode<'_, '_>>) -> Option<f64> {
    node.and_then(|n| text_trimmed(n).parse::<f64>().ok())
}

fn color_of(value: &str) -> [f32; 4] {
    let values: Vec<f32> = value
        .split_whitespace()
        .filter_map(|token| token.parse::<f32>().ok())
        .collect();
    [
        values.first().copied().unwrap_or(0.8),
        values.get(1).copied().unwrap_or(0.8),
        values.get(2).copied().unwrap_or(0.8),
        values.get(3).copied().unwrap_or(1.0),
    ]
}

fn decode_percent(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(&[high, low]) = bytes.get(i + 1..i + 3)
            && let (Some(high), Some(low)) = (hex_digit(high), hex_digit(low))
        {
            out.push(high << 4 | low);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|digit| digit as u8)
}

fn resolve_path(value: &str, base_dir: &Path) -> PathBuf {
    let decoded = decode_percent(value.trim());
    let path = decoded.strip_prefix("file://").unwrap_or(&decoded);
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

fn resolve_texture(
    name: &str,
    profile: XNode<'_, '_>,
    images: &HashMap<String, XNode<'_, '_>>,
    base_dir: &Path,
) -> Option<PathBuf> {
    let name = name.trim();
    let params = newparams(profile);

    let sampler = params
        .iter()
        .find(|(sid, _)| sid == name)
        .and_then(|(_, node)| child(*node, "sampler2D"));
    let mut image_id = sampler
        .and_then(|sampler| child(sampler, "source"))
        .map(text_trimmed)
        .map(|source| strip_hash(source.as_str()).to_string());

    if let Some(id) = &image_id {
        let surface = params
            .iter()
            .find(|(sid, _)| sid == id)
            .and_then(|(_, node)| child(*node, "surface"))
            .and_then(|surface| child(surface, "init_from"))
            .map(text_trimmed)
            .map(|value| strip_hash(value.as_str()).to_string());
        if surface.is_some() {
            image_id = surface;
        }
    }

    if image_id.as_ref().is_none_or(|id| !images.contains_key(id)) && images.contains_key(name) {
        image_id = Some(name.to_string());
    }

    let image = images.get(image_id.as_ref()?)?;
    let init_from = child(*image, "init_from").map(text_trimmed)?;
    Some(resolve_path(&init_from, base_dir))
}

fn parse_color_texture(
    shading: XNode<'_, '_>,
    profile: XNode<'_, '_>,
    images: &HashMap<String, XNode<'_, '_>>,
    base_dir: &Path,
    slot: &str,
    is_constant: bool,
) -> ([f32; 4], Option<PathBuf>) {
    let Some(container) = child(shading, slot) else {
        return ([0.8, 0.8, 0.8, 1.0], None);
    };
    if let Some(texture) = child(container, "texture") {
        let path = resolve_texture(attr(texture, "texture"), profile, images, base_dir);
        return ([1.0, 1.0, 1.0, 1.0], path);
    }
    if let Some(color) = child(container, "color") {
        let mut rgba = color_of(&text_of(color));
        if is_constant && rgba == [0.8, 0.8, 0.8, 1.0] {
            rgba = [0.8, 0.8, 0.8, 1.0];
        }
        return (rgba, None);
    }
    ([0.8, 0.8, 0.8, 1.0], None)
}

fn parse_material(
    node: XNode<'_, '_>,
    effects: &HashMap<String, XNode<'_, '_>>,
    images: &HashMap<String, XNode<'_, '_>>,
    base_dir: &Path,
) -> Material {
    let id = attr(node, "id").to_string();
    let name = attr(node, "name").to_string();
    let mut material = Material {
        id,
        name,
        color: [0.8, 0.8, 0.8, 1.0],
        texture: None,
        opacity: 1.0,
        unlit: false,
    };

    let Some(effect_id) = child(node, "instance_effect")
        .map(|effect| strip_hash(attr(effect, "url")))
        .filter(|id| !id.is_empty())
    else {
        return material;
    };
    let Some(effect) = effects.get(effect_id) else {
        return material;
    };
    let Some(profile) = child(*effect, "profile_COMMON") else {
        return material;
    };
    let Some(technique) = child(profile, "technique") else {
        return material;
    };

    let shading = elements(technique).find(|element| {
        matches!(
            element.tag_name().name(),
            "lambert" | "phong" | "blinn" | "constant"
        )
    });
    let Some(shading) = shading else {
        return material;
    };

    let is_constant = shading.tag_name().name() == "constant";
    let slot = if is_constant { "emission" } else { "diffuse" };
    let (color, texture) =
        parse_color_texture(shading, profile, images, base_dir, slot, is_constant);
    material.color = color;
    material.texture = texture;
    material.unlit = is_constant;

    if let Some(transparent) = child(shading, "transparent")
        && child(transparent, "texture").is_none()
        && let Some(color) = child(transparent, "color")
    {
        let rgba = color_of(&text_of(color));
        let transparency =
            text_float(child(shading, "transparency").and_then(|t| child(t, "float")))
                .unwrap_or(1.0);
        let mode = attr(transparent, "opaque");
        let value = if mode == "RGB_ZERO" {
            let luminance =
                0.212671 * rgba[0] as f64 + 0.715160 * rgba[1] as f64 + 0.072169 * rgba[2] as f64;
            1.0 - luminance * transparency
        } else {
            rgba[3] as f64 * transparency
        };
        material.opacity = value.clamp(0.0, 1.0) as f32;
    }

    material
}

fn parse_projection(camera: XNode<'_, '_>) -> Projection {
    let technique = child(camera, "optics").and_then(|optics| child(optics, "technique_common"));
    if let Some(perspective) = technique.and_then(|t| child(t, "perspective")) {
        let yfov = text_float(child(perspective, "yfov"));
        let xfov = text_float(child(perspective, "xfov"));
        return Projection::Perspective {
            yfov_degrees: yfov.or(xfov).unwrap_or(0.0),
        };
    }
    if let Some(orthographic) = technique.and_then(|t| child(t, "orthographic")) {
        let xmag = text_float(child(orthographic, "xmag"));
        let ymag = text_float(child(orthographic, "ymag"));
        return Projection::Orthographic {
            xmag: xmag.or(ymag).unwrap_or(0.0),
        };
    }
    Projection::Perspective { yfov_degrees: 0.0 }
}

fn parse_matrix(node: XNode<'_, '_>) -> Result<DMat4> {
    let values = parse_f64_vec(&text_of(node))?;
    if values.len() < 16 {
        return Err(invalid("matrix needs 16 numbers"));
    }
    let array: [f64; 16] = std::array::from_fn(|i| values[i]);
    Ok(DMat4::from_cols_array(&array).transpose())
}

fn parse_translate(node: XNode<'_, '_>) -> Result<DMat4> {
    let values = parse_f64_vec(&text_of(node))?;
    let v = f64_array::<3>(&values, 0.0);
    Ok(DMat4::from_translation(DVec3::from_array(v)))
}

fn parse_scale(node: XNode<'_, '_>) -> Result<DMat4> {
    let values = parse_f64_vec(&text_of(node))?;
    let v = f64_array::<3>(&values, 1.0);
    Ok(DMat4::from_scale(DVec3::from_array(v)))
}

fn parse_rotate(node: XNode<'_, '_>) -> Result<DMat4> {
    let values = parse_f64_vec(&text_of(node))?;
    let axis = DVec3::from_array(f64_array::<3>(&values, 0.0));
    let degrees = values.get(3).copied().unwrap_or(0.0);
    if axis.length_squared() == 0.0 {
        return Ok(DMat4::IDENTITY);
    }
    Ok(DMat4::from_axis_angle(
        axis.normalize(),
        degrees.to_radians(),
    ))
}

enum PItem {
    Camera(usize),
    Child(usize),
    Instance(usize),
}

struct PNode {
    name: String,
    transform: DMat4,
    children: Vec<PNode>,
    geometry: Vec<GeometryInstance>,
    instances: Vec<usize>,
    items: Vec<PItem>,
}

impl PNode {
    fn new(name: String) -> Self {
        Self {
            name,
            transform: DMat4::IDENTITY,
            children: Vec::new(),
            geometry: Vec::new(),
            instances: Vec::new(),
            items: Vec::new(),
        }
    }

    fn to_node(&self) -> Node {
        Node {
            name: self.name.clone(),
            transform: self.transform,
            children: self.children.iter().map(PNode::to_node).collect(),
            geometry: self.geometry.clone(),
            instances: self.instances.clone(),
        }
    }
}

struct NodeCtx<'a> {
    meshes: &'a HashMap<String, usize>,
    materials: &'a HashMap<String, usize>,
    definitions: &'a HashMap<String, usize>,
    cameras: &'a HashMap<String, usize>,
}

fn parse_instance_geometry(node: XNode<'_, '_>, ctx: &NodeCtx<'_>) -> Result<GeometryInstance> {
    let url = attr(node, "url");
    let id = strip_hash(url);
    let mesh = *ctx.meshes.get(id).ok_or_else(|| {
        invalid(format!(
            "instance_geometry url \"{url}\" names an unknown geometry"
        ))
    })?;

    let mut bindings = Vec::new();
    if let Some(bind_material) = child(node, "bind_material")
        && let Some(technique) = child(bind_material, "technique_common")
    {
        for instance in children(technique, "instance_material") {
            let symbol = attr(instance, "symbol").to_string();
            let target = attr(instance, "target");
            if target.is_empty() {
                continue;
            }
            let material_id = strip_hash(target);
            let material = *ctx.materials.get(material_id).ok_or_else(|| {
                invalid(format!(
                    "instance_material target \"{target}\" is not a known material"
                ))
            })?;
            bindings.push((symbol, material));
        }
    }

    Ok(GeometryInstance { mesh, bindings })
}

fn parse_pnode(node: XNode<'_, '_>, ctx: &NodeCtx<'_>) -> Result<PNode> {
    let id = attr(node, "id");
    let name_attr = attr(node, "name");
    let name = if !name_attr.is_empty() {
        name_attr.to_string()
    } else {
        id.to_string()
    };
    let mut parsed = PNode::new(name);
    let mut transform = DMat4::IDENTITY;

    for element in elements(node) {
        match element.tag_name().name() {
            "matrix" => transform *= parse_matrix(element)?,
            "translate" => transform *= parse_translate(element)?,
            "rotate" => transform *= parse_rotate(element)?,
            "scale" => transform *= parse_scale(element)?,
            "node" => {
                let index = parsed.children.len();
                parsed.children.push(parse_pnode(element, ctx)?);
                parsed.items.push(PItem::Child(index));
            }
            "instance_geometry" => {
                parsed.geometry.push(parse_instance_geometry(element, ctx)?);
            }
            "instance_node" => {
                let url = attr(element, "url");
                let index = *ctx.definitions.get(strip_hash(url)).ok_or_else(|| {
                    invalid(format!(
                        "instance_node url \"{url}\" names an unknown definition"
                    ))
                })?;
                parsed.instances.push(index);
                parsed.items.push(PItem::Instance(index));
            }
            "instance_camera" => {
                let url = attr(element, "url");
                let index = *ctx.cameras.get(strip_hash(url)).ok_or_else(|| {
                    invalid(format!(
                        "instance_camera url \"{url}\" names an unknown camera"
                    ))
                })?;
                parsed.items.push(PItem::Camera(index));
            }
            _ => {}
        }
    }

    parsed.transform = transform;
    Ok(parsed)
}

fn collect_instance_refs(node: &PNode, out: &mut Vec<usize>) {
    out.extend(node.instances.iter().copied());
    for child in &node.children {
        collect_instance_refs(child, out);
    }
}

fn check_cycles(definitions: &[PNode], ids: &[String]) -> Result<()> {
    let mut adjacency = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let mut refs = Vec::new();
        collect_instance_refs(definition, &mut refs);
        refs.sort_unstable();
        refs.dedup();
        adjacency.push(refs);
    }

    fn visit(index: usize, adjacency: &[Vec<usize>], state: &mut [u8]) -> bool {
        state[index] = 1;
        for &next in &adjacency[index] {
            if state[next] == 1 {
                return true;
            }
            if state[next] == 0 && visit(next, adjacency, state) {
                return true;
            }
        }
        state[index] = 2;
        false
    }

    let mut state = vec![0u8; definitions.len()];
    for index in 0..definitions.len() {
        if state[index] == 0 && visit(index, &adjacency, &mut state) {
            let name = ids.get(index).map(String::as_str).unwrap_or("");
            return Err(invalid(format!(
                "definition \"{name}\" instances itself through library_nodes"
            )));
        }
    }
    Ok(())
}

fn collect_cameras(
    node: &PNode,
    parent: DMat4,
    definitions: &[PNode],
    projections: &[Projection],
    cameras: &mut Vec<ViewCamera>,
) {
    let world = parent * node.transform;
    for item in &node.items {
        match item {
            PItem::Camera(index) => cameras.push(ViewCamera {
                name: node.name.clone(),
                transform: world,
                projection: projections[*index],
            }),
            PItem::Child(index) => {
                collect_cameras(
                    &node.children[*index],
                    world,
                    definitions,
                    projections,
                    cameras,
                );
            }
            PItem::Instance(index) => {
                collect_cameras(
                    &definitions[*index],
                    world,
                    definitions,
                    projections,
                    cameras,
                );
            }
        }
    }
}

pub(crate) fn read(xml: &str, base_dir: &Path) -> Result<Model> {
    let document = Document::parse(xml).map_err(|error| ImportError::Xml(error.to_string()))?;
    let root = document.root_element();
    if root.tag_name().name() != "COLLADA" {
        return Err(invalid(format!(
            "expected a <COLLADA> root element, found <{}>",
            root.tag_name().name()
        )));
    }

    let (unit_meters, up_axis) = parse_asset(root);

    let mut images = HashMap::new();
    for library in libraries(root, "library_images") {
        for image in children(library, "image") {
            let id = attr(image, "id");
            if !id.is_empty() {
                images.entry(id.to_string()).or_insert(image);
            }
        }
    }

    let mut effects = HashMap::new();
    for library in libraries(root, "library_effects") {
        for effect in children(library, "effect") {
            let id = attr(effect, "id");
            if !id.is_empty() {
                effects.entry(id.to_string()).or_insert(effect);
            }
        }
    }

    let mut meshes = Vec::new();
    let mut mesh_ids = HashMap::new();
    for library in libraries(root, "library_geometries") {
        for geometry in children(library, "geometry") {
            let mesh = parse_geometry(geometry)?;
            let index = meshes.len();
            if !mesh.id.is_empty() {
                mesh_ids.entry(mesh.id.clone()).or_insert(index);
            }
            meshes.push(mesh);
        }
    }

    let mut materials = Vec::new();
    let mut material_ids = HashMap::new();
    for library in libraries(root, "library_materials") {
        for material in children(library, "material") {
            let parsed = parse_material(material, &effects, &images, base_dir);
            let index = materials.len();
            if !parsed.id.is_empty() {
                material_ids.entry(parsed.id.clone()).or_insert(index);
            }
            materials.push(parsed);
        }
    }

    let mut camera_projections = Vec::new();
    let mut camera_ids = HashMap::new();
    for library in libraries(root, "library_cameras") {
        for camera in children(library, "camera") {
            let index = camera_projections.len();
            camera_projections.push(parse_projection(camera));
            let id = attr(camera, "id");
            if !id.is_empty() {
                camera_ids.entry(id.to_string()).or_insert(index);
            }
        }
    }

    let mut definition_nodes = Vec::new();
    for library in libraries(root, "library_nodes") {
        definition_nodes.extend(children(library, "node"));
    }
    let mut definition_ids = HashMap::new();
    for (index, node) in definition_nodes.iter().enumerate() {
        let id = attr(*node, "id");
        if !id.is_empty() {
            definition_ids.entry(id.to_string()).or_insert(index);
        }
    }

    let ctx = NodeCtx {
        meshes: &mesh_ids,
        materials: &material_ids,
        definitions: &definition_ids,
        cameras: &camera_ids,
    };

    let parsed_definitions: Vec<PNode> = definition_nodes
        .iter()
        .map(|node| parse_pnode(*node, &ctx))
        .collect::<Result<_>>()?;
    let definition_id_list: Vec<String> = definition_nodes
        .iter()
        .map(|node| attr(*node, "id").to_string())
        .collect();
    check_cycles(&parsed_definitions, &definition_id_list)?;

    let definitions: Vec<Definition> = definition_nodes
        .iter()
        .zip(&parsed_definitions)
        .map(|(raw, parsed)| Definition {
            id: attr(*raw, "id").to_string(),
            name: parsed.name.clone(),
            node: parsed.to_node(),
        })
        .collect();

    let mut visual_scenes = Vec::new();
    for library in libraries(root, "library_visual_scenes") {
        visual_scenes.extend(children(library, "visual_scene"));
    }

    let selected_scene = if let Some(scene) = child(root, "scene") {
        match child(scene, "instance_visual_scene").map(|instance| attr(instance, "url")) {
            Some(url) if !url.is_empty() => {
                let id = strip_hash(url);
                Some(
                    visual_scenes
                        .iter()
                        .find(|candidate| attr(**candidate, "id") == id)
                        .copied()
                        .ok_or_else(|| {
                            invalid(format!(
                                "instance_visual_scene url \"{url}\" names an unknown visual scene"
                            ))
                        })?,
                )
            }
            _ => visual_scenes.first().copied(),
        }
    } else {
        visual_scenes.first().copied()
    };

    let mut root_pnode = PNode::new(
        selected_scene
            .map(|scene| {
                let name = attr(scene, "name");
                if !name.is_empty() {
                    name.to_string()
                } else {
                    attr(scene, "id").to_string()
                }
            })
            .unwrap_or_default(),
    );
    if let Some(scene) = selected_scene {
        for node in children(scene, "node") {
            let index = root_pnode.children.len();
            root_pnode.children.push(parse_pnode(node, &ctx)?);
            root_pnode.items.push(PItem::Child(index));
        }
    }

    let mut cameras = Vec::new();
    collect_cameras(
        &root_pnode,
        DMat4::IDENTITY,
        &parsed_definitions,
        &camera_projections,
        &mut cameras,
    );

    Ok(Model {
        unit_meters,
        up_axis,
        meshes,
        materials,
        definitions,
        scene: root_pnode.to_node(),
        cameras,
    })
}

#[cfg(test)]
mod tests {
    use super::decode_percent;

    #[test]
    fn percent_decoding() {
        assert_eq!(decode_percent("a%20b"), "a b");
        assert_eq!(decode_percent("end%20"), "end ");
        assert_eq!(decode_percent("caf%C3%A9"), "café");
        assert_eq!(decode_percent("50%"), "50%");
        assert_eq!(decode_percent("%zz"), "%zz");
        assert_eq!(decode_percent("%é1"), "%é1");
    }
}
