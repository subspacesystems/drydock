// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fixtures shared by the push/pull tests.

#![allow(dead_code)]

use std::collections::BTreeMap;

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::topology::*;

pub fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

pub fn ctx(model: &Model) -> &Context {
    model.root().geometry()
}

/// Add faces to the root. Each face is a list of loops (outer first, then holes), each loop a
/// list of indexes into `points`, counter-clockwise seen from the front for the outer loop and
/// clockwise for holes. Edges are shared between faces. Returns the face ids in order.
pub fn mesh(model: &mut Model, points: &[DVec3], faces: &[&[&[usize]]]) -> Vec<FaceId> {
    let mut edit = ModelEdit::new();
    let ids = model
        .edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut out = Vec::new();
            for face in faces {
                let mut loops = Vec::new();
                for l in face.iter() {
                    let mut uses = Vec::new();
                    for i in 0..l.len() {
                        let (a, b) = (l[i], l[(i + 1) % l.len()]);
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
                    loops.push(Loop { uses });
                }
                let outer = loops.remove(0);
                let face = Face {
                    outer,
                    inner: loops,
                };
                out.push(ctx.insert_face(face, log).unwrap());
            }
            out
        })
        .unwrap();
    assert!(
        ctx(model).validate().is_empty(),
        "{:?}",
        ctx(model).validate()
    );
    ids
}

/// A square on the ground fronting +z.
pub fn square(model: &mut Model, x: f64, y: f64, size: f64) -> FaceId {
    let pts = [
        v(x, y, 0.0),
        v(x + size, y, 0.0),
        v(x + size, y + size, 0.0),
        v(x, y + size, 0.0),
    ];
    mesh(model, &pts, &[&[&[0, 1, 2, 3]]])[0]
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

/// Unit cube, fronts outward: bottom, top, front (-y), back, right (+x), left.
pub fn cube(model: &mut Model) -> Vec<FaceId> {
    mesh(
        model,
        &CUBE_POINTS,
        &[
            &[&[0, 3, 2, 1]],
            &[&[4, 5, 6, 7]],
            &[&[0, 1, 5, 4]],
            &[&[2, 3, 7, 6]],
            &[&[1, 2, 6, 5]],
            &[&[3, 0, 4, 7]],
        ],
    )
}

/// Unit cube whose top is a ring around an inner face (0.25..0.75 square). Returns
/// `[bottom, ring, front, back, right, left, inner]`.
pub fn cube_with_inset(model: &mut Model) -> Vec<FaceId> {
    let mut pts = CUBE_POINTS.to_vec();
    pts.extend([
        v(0.25, 0.25, 1.0),
        v(0.75, 0.25, 1.0),
        v(0.75, 0.75, 1.0),
        v(0.25, 0.75, 1.0),
    ]);
    mesh(
        model,
        &pts,
        &[
            &[&[0, 3, 2, 1]],
            &[&[4, 5, 6, 7], &[8, 11, 10, 9]],
            &[&[0, 1, 5, 4]],
            &[&[2, 3, 7, 6]],
            &[&[1, 2, 6, 5]],
            &[&[3, 0, 4, 7]],
            &[&[8, 9, 10, 11]],
        ],
    )
}

pub fn face_points(c: &Context, f: FaceId) -> Vec<DVec3> {
    c.loop_points(&c.face(f).unwrap().outer)
}

pub fn centroid(pts: &[DVec3]) -> DVec3 {
    pts.iter().copied().sum::<DVec3>() / pts.len() as f64
}

pub fn normal(c: &Context, f: FaceId) -> DVec3 {
    c.face_plane(f).unwrap().normal
}

/// Every edge borders exactly two faces.
pub fn closed(c: &Context) -> bool {
    c.edges().all(|(e, _)| c.faces_of_edge(e).len() == 2)
}

/// Signed volume enclosed by the faces (positive when fronts face out).
pub fn volume(c: &Context) -> f64 {
    let mut sum = 0.0;
    for (_, face) in c.faces() {
        for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
            let p = c.loop_points(l);
            for i in 1..p.len().saturating_sub(1) {
                sum += p[0].dot(p[i].cross(p[i + 1]));
            }
        }
    }
    sum / 6.0
}

/// For convex solids: every face's normal points away from the centroid of all vertices.
pub fn normals_outward(c: &Context) -> bool {
    let all: Vec<DVec3> = c.vertices().map(|(_, p)| p.position).collect();
    let centre = centroid(&all);
    c.faces()
        .all(|(f, _)| normal(c, f).dot(centroid(&face_points(c, f)) - centre) > 0.0)
}
