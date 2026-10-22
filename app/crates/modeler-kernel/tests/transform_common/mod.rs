// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fixtures shared by the transform tests.

#![allow(dead_code)]

use std::collections::BTreeMap;

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::topology::*;

pub fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

pub fn mesh(model: &mut Model, points: &[DVec3], faces: &[&[usize]]) -> Vec<FaceId> {
    let mut edit = ModelEdit::new();
    let ids = model
        .edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut out = Vec::new();
            for face in faces {
                let mut uses = Vec::new();
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], log).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                let face = Face {
                    outer: Loop { uses },
                    inner: vec![],
                };
                out.push(ctx.insert_face(face, log).unwrap());
            }
            out
        })
        .unwrap();
    assert!(model.root().geometry().validate().is_empty());
    ids
}

const CUBE_POINTS: [DVec3; 8] = [
    DVec3::new(0.0, 0.0, 0.0),
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
    DVec3::new(0.0, 1.0, 0.0),
    DVec3::new(0.0, 0.0, 1.0),
    DVec3::new(1.0, 0.0, 1.0),
    DVec3::new(1.0, 1.0, 1.0),
    DVec3::new(0.0, 1.0, 1.0),
];

const CUBE_FACES: [&[usize]; 6] = [
    &[0, 3, 2, 1], // bottom, normal -z
    &[4, 5, 6, 7], // top, normal +z
    &[0, 1, 5, 4], // front, normal -y
    &[2, 3, 7, 6], // back, normal +y
    &[1, 2, 6, 5], // right, normal +x
    &[3, 0, 4, 7], // left, normal -x
];

pub fn cube(model: &mut Model) -> Vec<FaceId> {
    mesh(model, &CUBE_POINTS, &CUBE_FACES)
}

pub fn positions(model: &Model) -> Vec<DVec3> {
    model
        .root()
        .geometry()
        .vertices()
        .map(|(_, p)| p.position)
        .collect()
}

/// Every face's normal points away from the centroid of all vertices (for convex solids).
pub fn normals_outward(model: &Model) -> bool {
    let c = model.root().geometry();
    let pts = positions(model);
    let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
    c.faces().all(|(f, face)| {
        let plane = c.face_plane(f).unwrap();
        let pts = c.loop_points(&face.outer);
        let mid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        plane.normal.dot(mid - centre) > 0.0
    })
}
