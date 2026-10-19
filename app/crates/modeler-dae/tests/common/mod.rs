// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Helpers shared by the m1-dae-to-kernel tests: documents built in code, and area per
//! material measured on both sides of the conversion.

#![allow(dead_code)]

use std::collections::BTreeMap;

use glam::{DMat4, DVec3};
use modeler_dae::{
    Definition, FlatScene, GeometryInstance, Material, Mesh, Model, Node, Primitive, PrimitiveKind,
    UpAxis,
};
use modeler_kernel::geom::area_vector;
use modeler_kernel::model::{self as km, BodyId};

pub fn node(name: &str) -> Node {
    Node {
        name: name.to_string(),
        transform: DMat4::IDENTITY,
        children: Vec::new(),
        geometry: Vec::new(),
        instances: Vec::new(),
    }
}

pub fn doc() -> Model {
    Model {
        unit_meters: 1.0,
        up_axis: UpAxis::Z,
        meshes: Vec::new(),
        materials: Vec::new(),
        definitions: Vec::new(),
        scene: node(""),
        cameras: Vec::new(),
    }
}

pub fn material(id: &str, name: &str, color: [f32; 4]) -> Material {
    Material {
        id: id.to_string(),
        name: name.to_string(),
        color,
        texture: None,
        opacity: 1.0,
        unlit: false,
    }
}

/// A primitive whose points are listed in index order (indices 0, 1, 2, ...).
pub fn prim(kind: PrimitiveKind, symbol: Option<&str>, points: &[[f32; 3]]) -> Primitive {
    Primitive {
        kind,
        material_symbol: symbol.map(str::to_string),
        positions: points.to_vec(),
        normals: Vec::new(),
        texcoords: Vec::new(),
        indices: (0..points.len() as u32).collect(),
    }
}

/// Triangles, three points each.
pub fn tris(symbol: Option<&str>, triangles: &[[[f32; 3]; 3]]) -> Primitive {
    let points: Vec<[f32; 3]> = triangles.iter().flatten().copied().collect();
    prim(PrimitiveKind::Triangles, symbol, &points)
}

/// Segments, two points each.
pub fn segs(segments: &[[[f32; 3]; 2]]) -> Primitive {
    let points: Vec<[f32; 3]> = segments.iter().flatten().copied().collect();
    prim(PrimitiveKind::Lines, None, &points)
}

/// Add a mesh and return a geometry instance of it with the given `(symbol, material)`
/// bindings.
pub fn add_mesh(
    doc: &mut Model,
    primitives: Vec<Primitive>,
    bindings: &[(&str, usize)],
) -> GeometryInstance {
    doc.meshes.push(Mesh {
        id: format!("mesh{}", doc.meshes.len()),
        name: String::new(),
        primitives,
    });
    GeometryInstance {
        mesh: doc.meshes.len() - 1,
        bindings: bindings.iter().map(|&(s, m)| (s.to_string(), m)).collect(),
    }
}

pub fn definition(id: &str, name: &str, node: Node) -> Definition {
    Definition {
        id: id.to_string(),
        name: name.to_string(),
        node,
    }
}

/// Area of a face's region under `world`: outer loop minus holes.
pub fn face_area(
    ctx: &modeler_kernel::topology::Context,
    face: &modeler_kernel::topology::Face,
    world: DMat4,
) -> f64 {
    let area = |l: &modeler_kernel::topology::Loop| {
        let pts: Vec<DVec3> = ctx
            .loop_points(l)
            .into_iter()
            .map(|p| world.transform_point3(p))
            .collect();
        area_vector(&pts).length()
    };
    area(&face.outer) - face.inner.iter().map(area).sum::<f64>()
}

/// Material key used for comparisons: the index of the material among the model's
/// materials in id order, or `None` for the default material.
pub type Key = Option<usize>;

/// World area per (front material, back material) over the whole model, walking
/// instances from the root.
pub fn kernel_areas(model: &km::Model) -> (BTreeMap<Key, f64>, BTreeMap<Key, f64>) {
    let index: BTreeMap<km::MaterialId, usize> = model
        .materials()
        .enumerate()
        .map(|(i, (id, _))| (id, i))
        .collect();
    let mut front = BTreeMap::new();
    let mut back = BTreeMap::new();
    let mut stack = vec![(BodyId::Root, DMat4::IDENTITY, 0usize)];
    while let Some((body_id, world, depth)) = stack.pop() {
        assert!(depth < 64, "instance nesting too deep");
        let body = model.body(body_id).expect("body exists");
        for (fid, face) in body.geometry().faces() {
            let area = face_area(body.geometry(), face, world);
            let fm = body.face_materials(fid);
            *front.entry(fm.front.map(|m| index[&m])).or_insert(0.0) += area;
            *back.entry(fm.back.map(|m| index[&m])).or_insert(0.0) += area;
        }
        for (_, inst) in body.instances() {
            stack.push((
                BodyId::Definition(inst.definition),
                world * inst.transform,
                depth + 1,
            ));
        }
    }
    (front, back)
}

/// World area per material of `flatten`'s triangles.
pub fn flat_areas(scene: &FlatScene) -> BTreeMap<Key, f64> {
    let mut out = BTreeMap::new();
    for batch in &scene.batches {
        let p = |i: u32| {
            let v = batch.positions[i as usize];
            DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
        };
        let mut sum = 0.0;
        for t in batch.indices.as_chunks::<3>().0 {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            sum += (b - a).cross(c - a).length() * 0.5;
        }
        *out.entry(batch.material).or_insert(0.0) += sum;
    }
    out
}

/// Assert two area maps agree within `rel` (relative to the larger value) plus `abs`.
pub fn assert_areas_match(
    what: &str,
    actual: &BTreeMap<Key, f64>,
    expected: &BTreeMap<Key, f64>,
    rel: f64,
    abs: f64,
) {
    let keys: std::collections::BTreeSet<Key> =
        actual.keys().chain(expected.keys()).copied().collect();
    for k in keys {
        let a = actual.get(&k).copied().unwrap_or(0.0);
        let e = expected.get(&k).copied().unwrap_or(0.0);
        assert!(
            (a - e).abs() <= rel * a.abs().max(e.abs()) + abs,
            "{what}: material {k:?}: kernel area {a}, expected {e}"
        );
    }
}
