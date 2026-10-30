// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fixtures shared by the grow/shrink tests (`docs/specs/m7-clearance-offset.md`).

#![allow(dead_code)]

use std::collections::BTreeMap;

use glam::{DMat4, DVec2, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::solid::check_body;
use modeler_kernel::topology::*;

pub fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Millimeters to meters.
pub fn mm(x: f64) -> f64 {
    x * 0.001
}

/// One face: its outer loop and its holes, as indices into the point list.
pub type FaceSpec = (Vec<usize>, Vec<Vec<usize>>);

pub fn plain(faces: &[Vec<usize>]) -> Vec<FaceSpec> {
    faces.iter().map(|f| (f.clone(), vec![])).collect()
}

/// Builds the faces in `body`; returns the face ids in order. Edges are shared by index pair.
pub fn build(model: &mut Model, body: BodyId, points: &[DVec3], faces: &[FaceSpec]) -> Vec<FaceId> {
    let mut edit = ModelEdit::new();
    let ids = model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut lp = |ctx: &mut Context, log: &mut Edit, idx: &[usize]| {
                let mut uses = Vec::new();
                for i in 0..idx.len() {
                    let (a, b) = (idx[i], idx[(i + 1) % idx.len()]);
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
                Loop { uses }
            };
            let mut ids = Vec::new();
            for (outer, holes) in faces {
                let outer = lp(ctx, log, outer);
                let inner = holes.iter().map(|h| lp(ctx, log, h)).collect();
                ids.push(ctx.insert_face(Face { outer, inner }, log).unwrap());
            }
            ids
        })
        .unwrap();
    let violations = model.body(body).unwrap().geometry().validate();
    assert!(violations.is_empty(), "{violations:?}");
    ids
}

/// A new group definition placed once at the root; returns its body.
pub fn add_group(model: &mut Model) -> BodyId {
    let mut edit = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Part".into(),
            group: true,
        },
        &mut edit,
    );
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    BodyId::Definition(def)
}

/// A group holding the faces; returns its body.
pub fn part(model: &mut Model, points: &[DVec3], faces: &[FaceSpec]) -> BodyId {
    let body = add_group(model);
    build(model, body, points, faces);
    body
}

/// The 8 corners of the box `lo..hi`.
pub fn box_points(lo: DVec3, hi: DVec3) -> Vec<DVec3> {
    vec![
        v(lo.x, lo.y, lo.z),
        v(hi.x, lo.y, lo.z),
        v(hi.x, hi.y, lo.z),
        v(lo.x, hi.y, lo.z),
        v(lo.x, lo.y, hi.z),
        v(hi.x, lo.y, hi.z),
        v(hi.x, hi.y, hi.z),
        v(lo.x, hi.y, hi.z),
    ]
}

/// Box faces with outward fronts (bottom, top, front, back, right, left), offset by `base`.
pub fn box_faces(base: usize) -> Vec<Vec<usize>> {
    let f: [[usize; 4]; 6] = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [1, 2, 6, 5],
        [3, 0, 4, 7],
    ];
    f.iter()
        .map(|q| q.iter().map(|i| i + base).collect())
        .collect()
}

pub fn reversed(face: &[usize]) -> Vec<usize> {
    face.iter().rev().copied().collect()
}

/// A group holding the box `lo..hi`.
pub fn box_part(model: &mut Model, lo: DVec3, hi: DVec3) -> BodyId {
    part(model, &box_points(lo, hi), &plain(&box_faces(0)))
}

/// Points and faces of the prism over the counter-clockwise polygon `base` from `z0` to
/// `z1`: bottom (points 0..n), top (n..2n), then one side quad per polygon edge.
pub fn prism(base: &[DVec2], z0: f64, z1: f64) -> (Vec<DVec3>, Vec<Vec<usize>>) {
    let n = base.len();
    let mut points: Vec<DVec3> = base.iter().map(|p| v(p.x, p.y, z0)).collect();
    points.extend(base.iter().map(|p| v(p.x, p.y, z1)));
    let mut faces = vec![reversed(&(0..n).collect::<Vec<_>>()), (n..2 * n).collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i, j, n + j, n + i]);
    }
    (points, faces)
}

pub fn geometry(model: &Model, body: BodyId) -> &Context {
    model.body(body).unwrap().geometry()
}

pub fn positions(model: &Model, body: BodyId) -> BTreeMap<VertexId, DVec3> {
    geometry(model, body)
        .vertices()
        .map(|(id, p)| (id, p.position))
        .collect()
}

/// Bounding box of the body's vertices.
pub fn bounds(model: &Model, body: BodyId) -> (DVec3, DVec3) {
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for (_, p) in geometry(model, body).vertices() {
        lo = lo.min(p.position);
        hi = hi.max(p.position);
    }
    (lo, hi)
}

pub fn volume(model: &Model, body: BodyId) -> f64 {
    check_body(model, body).unwrap().volume.unwrap()
}

/// Relative closeness for volumes.
pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + b.abs()) + 1e-15
}

pub fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) <= 1e-9
}

/// Every moved vertex of each face lies `d` along the face's old outward normal from its
/// old plane (`before` holds the old positions; fronts are outward).
pub fn planes_moved_by(before: &Context, after: &Context, d: f64) -> Result<(), String> {
    for (f, face) in before.faces() {
        let plane = before.face_plane(f).ok_or(format!("{f:?} has no plane"))?;
        for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
            for vid in before.loop_vertices(l) {
                let p = after.vertex(vid).ok_or(format!("{vid:?} gone"))?.position;
                let got = plane.signed_distance(p);
                if (got - d).abs() > 1e-9 {
                    return Err(format!("{f:?} {vid:?}: {got} from the old plane, want {d}"));
                }
            }
        }
    }
    Ok(())
}

/// Vertex ids, edges and faces of a body, in id order.
pub type Topology = (Vec<VertexId>, Vec<(EdgeId, Edge)>, Vec<(FaceId, Face)>);

/// The body's vertex, edge and face ids and the loops of every face.
pub fn topology_of(model: &Model, body: BodyId) -> Topology {
    let c = geometry(model, body);
    (
        c.vertices().map(|(id, _)| id).collect(),
        c.edges().map(|(id, e)| (id, *e)).collect(),
        c.faces().map(|(id, f)| (id, f.clone())).collect(),
    )
}
