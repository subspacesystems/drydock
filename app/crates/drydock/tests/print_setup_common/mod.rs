// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Boxes for the Print Setup tests. Lengths are meters, as in the model.
#![allow(dead_code)]

use std::collections::BTreeMap;

use drydock_kernel::model::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};

pub fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

/// A group in the root: a box of `size` (x, y, z) from the origin, without the faces in
/// `skip` (indices into the six sides; a skipped side makes the box open), placed by
/// `transform`.
pub fn add_box(model: &mut Model, size: DVec3, skip: &[usize], transform: DMat4) -> InstanceId {
    let mut log = ModelEdit::new();
    let definition = model.add_definition(
        DefinitionInfo {
            name: "Box".into(),
            group: true,
        },
        &mut log,
    );
    let (x, y, z) = (size.x, size.y, size.z);
    let points = [
        v(0.0, 0.0, 0.0),
        v(x, 0.0, 0.0),
        v(x, y, 0.0),
        v(0.0, y, 0.0),
        v(0.0, 0.0, z),
        v(x, 0.0, z),
        v(x, y, z),
        v(0.0, y, z),
    ];
    model
        .edit_geometry(BodyId::Definition(definition), &mut log, |ctx, edit| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, edit)).collect();
            let mut edges = BTreeMap::new();
            for (n, quad) in FACES.iter().enumerate() {
                if skip.contains(&n) {
                    continue;
                }
                let mut uses = Vec::new();
                for i in 0..4 {
                    let (a, b) = (quad[i], quad[(i + 1) % 4]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], edit).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: vec![],
                    },
                    edit,
                )
                .unwrap();
            }
        })
        .unwrap();
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition,
                transform,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap()
}

/// A solid box of `size` millimeters whose corner is at (`x`, `y`, `z`) meters.
pub fn box_mm(model: &mut Model, size_mm: [f64; 3], at: DVec3) -> InstanceId {
    let size = v(size_mm[0], size_mm[1], size_mm[2]) / 1000.0;
    add_box(model, size, &[], DMat4::from_translation(at))
}

/// The lowest world z of `id`, in meters.
pub fn lowest_z(model: &Model, id: InstanceId) -> f64 {
    drydock_dae::instance_triangles(model, BodyId::Root, id, DMat4::IDENTITY)
        .iter()
        .flat_map(|t| t.iter())
        .map(|p| p.z)
        .fold(f64::INFINITY, f64::min)
}
