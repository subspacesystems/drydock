// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fixtures shared by the print mesh tests: kernel solids built straight from points, and
//! checks on triangle lists.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use glam::{DMat4, DVec3};
use modeler_kernel::curve::Curve;
use modeler_kernel::model::*;
use modeler_kernel::topology::*;
use modeler_mesh::print_mesh::Triangle;

pub fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A face: outer loop and holes, as indices into the point list.
pub type FaceSpec<'a> = (&'a [usize], &'a [&'a [usize]]);

/// Faces of `body` from points; each loop's edges are shared between faces by their end
/// points. `curves` lists chains of point indices (a closed chain repeats its first index)
/// that become kernel curves on the circle through `center`, `normal`, with x axis `x_axis`
/// and the given radius, from angle `start` to `end`.
pub struct CurveSpec<'a> {
    pub chain: &'a [usize],
    pub center: DVec3,
    pub normal: DVec3,
    pub radius: f64,
    pub start: f64,
    pub end: f64,
}

pub fn build(
    model: &mut Model,
    body: BodyId,
    points: &[DVec3],
    faces: &[FaceSpec],
    curves: &[CurveSpec],
) -> Vec<FaceId> {
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
            for c in curves {
                let x_axis = (points[c.chain[0]] - c.center).normalize();
                ctx.insert_curve(
                    Curve {
                        center: c.center,
                        normal: c.normal,
                        x_axis,
                        radius: c.radius,
                        start: c.start,
                        end: c.end,
                        vertices: c.chain.iter().map(|&i| vs[i]).collect(),
                    },
                    log,
                );
            }
            ids
        })
        .unwrap();
    assert_eq!(
        model.body(body).unwrap().geometry().validate(),
        vec![],
        "fixture is valid"
    );
    ids
}

/// A new, empty definition.
pub fn definition(model: &mut Model, name: &str, group: bool) -> (DefinitionId, BodyId) {
    let id = model.add_definition(
        DefinitionInfo {
            name: name.to_string(),
            group,
        },
        &mut ModelEdit::new(),
    );
    (id, BodyId::Definition(id))
}

pub fn place(
    model: &mut Model,
    parent: BodyId,
    definition: DefinitionId,
    transform: DMat4,
    name: &str,
) -> InstanceId {
    model
        .add_instance(
            parent,
            Instance {
                definition,
                transform,
                name: name.to_string(),
                material: None,
            },
            &mut ModelEdit::new(),
        )
        .unwrap()
}

pub fn set_instance_material(
    model: &mut Model,
    parent: BodyId,
    id: InstanceId,
    material: Option<MaterialId>,
) {
    let mut instance = model.body(parent).unwrap().instance(id).unwrap().clone();
    instance.material = material;
    model
        .set_instance(parent, id, instance, &mut ModelEdit::new())
        .unwrap();
}

pub fn add_material(model: &mut Model, name: &str, color: [f32; 4]) -> MaterialId {
    model.add_material(Material::new(name, color), &mut ModelEdit::new())
}

pub fn paint(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    front: Option<MaterialId>,
    back: Option<MaterialId>,
) {
    model
        .set_face_materials(
            body,
            face,
            FaceMaterials {
                front,
                back,
                front_map: None,
                back_map: None,
            },
            &mut ModelEdit::new(),
        )
        .unwrap();
}

/// Corners of the box `lo..hi`: bit 0 = x, bit 1 = y, bit 2 = z of the index.
fn box_points(lo: DVec3, hi: DVec3) -> Vec<DVec3> {
    (0..8)
        .map(|i| {
            v(
                if i & 1 != 0 { hi.x } else { lo.x },
                if i & 2 != 0 { hi.y } else { lo.y },
                if i & 4 != 0 { hi.z } else { lo.z },
            )
        })
        .collect()
}

/// Outward-facing box faces over `box_points`, in the order bottom, top, front, back,
/// left, right.
const BOX_FACES: [&[usize]; 6] = [
    &[0, 2, 3, 1],
    &[4, 5, 7, 6],
    &[0, 1, 5, 4],
    &[2, 6, 7, 3],
    &[0, 4, 6, 2],
    &[1, 3, 7, 5],
];

/// A group named `name` holding the closed box `lo..hi` (meters); returns its definition,
/// body and the six face ids (bottom, top, front, back, left, right).
pub fn box_group(
    model: &mut Model,
    name: &str,
    lo: DVec3,
    hi: DVec3,
) -> (DefinitionId, BodyId, Vec<FaceId>) {
    let (def, body) = definition(model, name, true);
    let faces: Vec<FaceSpec> = BOX_FACES.iter().map(|f| (*f, &[][..])).collect();
    let ids = build(model, body, &box_points(lo, hi), &faces, &[]);
    (def, body, ids)
}

/// The same box with the faces in `turned` wound the other way round.
pub fn box_group_turned(
    model: &mut Model,
    name: &str,
    lo: DVec3,
    hi: DVec3,
    turned: &[usize],
) -> (DefinitionId, BodyId, Vec<FaceId>) {
    let (def, body) = definition(model, name, true);
    let reversed: Vec<Vec<usize>> = BOX_FACES
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut f = f.to_vec();
            if turned.contains(&i) {
                f.reverse();
            }
            f
        })
        .collect();
    let faces: Vec<FaceSpec> = reversed.iter().map(|f| (&f[..], &[][..])).collect();
    let ids = build(model, body, &box_points(lo, hi), &faces, &[]);
    (def, body, ids)
}

/// The box without its top face: open, not a solid.
pub fn open_box_group(model: &mut Model, name: &str) -> DefinitionId {
    let (def, body) = definition(model, name, true);
    let faces: Vec<FaceSpec> = BOX_FACES
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, f)| (*f, &[][..]))
        .collect();
    build(
        model,
        body,
        &box_points(v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0)),
        &faces,
        &[],
    );
    def
}

/// A 4 x 4 x 1 m plate with a 2 x 2 m square hole through it (corner at the origin).
/// Top and bottom have an inner loop.
pub fn plate_with_hole(model: &mut Model, name: &str) -> (DefinitionId, BodyId) {
    let (def, body) = definition(model, name, true);
    let mut points = Vec::new();
    for z in [0.0, 1.0] {
        for (x, y) in [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)] {
            points.push(v(x, y, z));
        }
    }
    for z in [0.0, 1.0] {
        for (x, y) in [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)] {
            points.push(v(x, y, z));
        }
    }
    // o0..o3 bottom outer, o4..o7 top outer, h0..h3 (8..11) bottom hole, h4..h7 (12..15) top.
    let faces: Vec<FaceSpec> = vec![
        (&[0, 3, 2, 1], &[&[8, 9, 10, 11]]),
        (&[4, 5, 6, 7], &[&[12, 15, 14, 13]]),
        (&[0, 1, 5, 4], &[]),
        (&[1, 2, 6, 5], &[]),
        (&[2, 3, 7, 6], &[]),
        (&[3, 0, 4, 7], &[]),
        (&[9, 8, 12, 13], &[]),
        (&[10, 9, 13, 14], &[]),
        (&[11, 10, 14, 15], &[]),
        (&[8, 11, 15, 12], &[]),
    ];
    build(model, body, &points, &faces, &[]);
    (def, body)
}

/// A group holding a prism: a regular `n`-gon of radius `r` (meters) around the Z axis,
/// from z = 0 to z = `h`. With `curves`, both rings are kernel curves (circles).
pub fn cylinder_group(
    model: &mut Model,
    name: &str,
    r: f64,
    h: f64,
    n: usize,
    curves: bool,
) -> DefinitionId {
    let (def, body) = definition(model, name, true);
    let mut points = Vec::new();
    for z in [0.0, h] {
        for i in 0..n {
            let a = TAU * i as f64 / n as f64;
            points.push(v(r * a.cos(), r * a.sin(), z));
        }
    }
    let mut faces: Vec<Vec<usize>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push(vec![i, j, n + j, n + i]);
    }
    let faces: Vec<FaceSpec> = faces.iter().map(|f| (&f[..], &[][..])).collect();
    let bottom: Vec<usize> = (0..n).chain([0]).collect();
    let top: Vec<usize> = (n..2 * n).chain([n]).collect();
    let specs = [
        CurveSpec {
            chain: &bottom,
            center: v(0.0, 0.0, 0.0),
            normal: DVec3::Z,
            radius: r,
            start: 0.0,
            end: TAU,
        },
        CurveSpec {
            chain: &top,
            center: v(0.0, 0.0, h),
            normal: DVec3::Z,
            radius: r,
            start: 0.0,
            end: TAU,
        },
    ];
    build(
        model,
        body,
        &points,
        &faces,
        if curves { &specs } else { &[] },
    );
    def
}

/// A group holding a cone: the regular `n`-gon of radius `r` (meters) in z = 0, the base,
/// with its apex at z = `h`. With `curves`, the base ring is a kernel curve.
pub fn cone_group(
    model: &mut Model,
    name: &str,
    r: f64,
    h: f64,
    n: usize,
    curves: bool,
) -> DefinitionId {
    let (def, body) = definition(model, name, true);
    let mut points = Vec::new();
    for i in 0..n {
        let a = TAU * i as f64 / n as f64;
        points.push(v(r * a.cos(), r * a.sin(), 0.0));
    }
    points.push(v(0.0, 0.0, h));
    let mut faces: Vec<Vec<usize>> = vec![(0..n).rev().collect()];
    for i in 0..n {
        faces.push(vec![i, (i + 1) % n, n]);
    }
    let faces: Vec<FaceSpec> = faces.iter().map(|f| (&f[..], &[][..])).collect();
    let ring: Vec<usize> = (0..n).chain([0]).collect();
    let specs = [CurveSpec {
        chain: &ring,
        center: v(0.0, 0.0, 0.0),
        normal: DVec3::Z,
        radius: r,
        start: 0.0,
        end: TAU,
    }];
    build(
        model,
        body,
        &points,
        &faces,
        if curves { &specs } else { &[] },
    );
    def
}

/// A group holding a half-cylinder: the half disk `y >= 0` of radius `r` (an arc of `n`
/// steps from angle 0 to pi and the straight line back), from z = 0 to z = `h`. Both arcs
/// are open kernel curves.
pub fn half_cylinder_group(
    model: &mut Model,
    name: &str,
    r: f64,
    h: f64,
    n: usize,
) -> DefinitionId {
    let (def, body) = definition(model, name, true);
    let m = n + 1;
    let mut points = Vec::new();
    for z in [0.0, h] {
        for i in 0..m {
            let a = std::f64::consts::PI * i as f64 / n as f64;
            points.push(v(r * a.cos(), r * a.sin(), z));
        }
    }
    let mut faces: Vec<Vec<usize>> = vec![(0..m).rev().collect(), (m..2 * m).collect()];
    for i in 0..n {
        faces.push(vec![i, i + 1, m + i + 1, m + i]);
    }
    faces.push(vec![n, 0, m, m + n]);
    let faces: Vec<FaceSpec> = faces.iter().map(|f| (&f[..], &[][..])).collect();
    let bottom: Vec<usize> = (0..m).collect();
    let top: Vec<usize> = (m..2 * m).collect();
    let specs = [
        CurveSpec {
            chain: &bottom,
            center: v(0.0, 0.0, 0.0),
            normal: DVec3::Z,
            radius: r,
            start: 0.0,
            end: std::f64::consts::PI,
        },
        CurveSpec {
            chain: &top,
            center: v(0.0, 0.0, h),
            normal: DVec3::Z,
            radius: r,
            start: 0.0,
            end: std::f64::consts::PI,
        },
    ];
    build(model, body, &points, &faces, &specs);
    def
}

/// Signed volume of the closed triangle list (positive when it faces outward), in cubic
/// millimeters.
pub fn volume(triangles: &[Triangle]) -> f64 {
    triangles
        .iter()
        .map(|t| t[0].dot(t[1].cross(t[2])) / 6.0)
        .sum()
}

pub fn bounds(triangles: &[Triangle]) -> (DVec3, DVec3) {
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for p in triangles.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    (lo, hi)
}

#[track_caller]
pub fn assert_close(a: DVec3, b: DVec3, tol: f64) {
    assert!(a.distance(b) <= tol, "{a:?} != {b:?}");
}

#[track_caller]
pub fn assert_rel(a: f64, b: f64, rel: f64) {
    assert!(
        (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-12),
        "{a} != {b}"
    );
}

fn key(p: DVec3) -> [u64; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

/// Every undirected edge is used by exactly two triangles, once in each direction (points
/// compared bit for bit).
#[track_caller]
pub fn assert_closed(triangles: &[Triangle]) {
    let mut directed: BTreeMap<([u64; 3], [u64; 3]), usize> = BTreeMap::new();
    for t in triangles {
        for i in 0..3 {
            let (a, b) = (key(t[i]), key(t[(i + 1) % 3]));
            assert_ne!(a, b, "triangle with a repeated corner: {t:?}");
            *directed.entry((a, b)).or_default() += 1;
        }
    }
    for (&(a, b), &count) in &directed {
        assert_eq!(count, 1, "edge used {count} times the same way");
        assert_eq!(
            directed.get(&(b, a)),
            Some(&1),
            "edge has no partner running the other way"
        );
    }
}

/// Distinct points of `triangles` with the given z (within 1e-9), as (x, y).
pub fn ring_at(triangles: &[Triangle], z: f64) -> Vec<(f64, f64)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for p in triangles.iter().flatten() {
        if (p.z - z).abs() < 1e-9 && seen.insert(key(*p)) {
            out.push((p.x, p.y));
        }
    }
    out
}

/// Largest distance from a chord's midpoint to the circle of radius `r` about the Z axis,
/// over the angular gaps of `ring`.
pub fn max_sagitta(ring: &[(f64, f64)], r: f64) -> f64 {
    let mut angles: Vec<f64> = ring.iter().map(|(x, y)| y.atan2(*x)).collect();
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut worst: f64 = 0.0;
    for i in 0..angles.len() {
        let next = if i + 1 < angles.len() {
            angles[i + 1]
        } else {
            angles[0] + TAU
        };
        let gap = next - angles[i];
        worst = worst.max(r * (1.0 - (gap / 2.0).cos()));
    }
    worst
}
