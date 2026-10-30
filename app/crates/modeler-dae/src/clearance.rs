// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fit check for 3D printing: which triangles of two parts come closer to the other part
//! than the clearance, and a scene copy with those triangles tinted.
//!
//! Behaviour: `docs/specs/m7-clearance.md`.

use std::collections::HashMap;

use glam::{DMat4, DVec3};
use modeler_kernel::geom::{polygon_normal, to_plane_2d};
use modeler_kernel::model as km;
use modeler_kernel::tess::triangulate;

use crate::{Batch, FlatScene, Material};

/// A triangle's corners, in meters.
pub type Triangle = [DVec3; 3];

/// The clearance the app starts with: 0.2 mm, in meters.
pub const DEFAULT_CLEARANCE_M: f64 = 0.0002;

/// Color of the material [`tint_fit`] adds.
pub const FIT_COLOR: [f32; 4] = [0.95, 0.75, 0.1, 1.0];

/// How close a scene triangle's corners must be to a flagged triangle's corners for
/// [`tint_fit`] to tint it, in meters.
pub const MATCH_TOLERANCE_M: f64 = 1e-5;

/// How deep instances may nest before the walk stops, as in `kernel_scene`.
const MAX_DEPTH: usize = 64;

/// Which triangles of two parts are closer to the other part than the clearance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FitReport {
    /// Indices into the first list, ascending.
    pub a: Vec<usize>,
    /// Indices into the second list, ascending.
    pub b: Vec<usize>,
}

/// The shortest distance between the two closed triangles (their points, not just their
/// corners): 0 when they touch, share a point or cross.
pub fn triangle_distance(a: &Triangle, b: &Triangle) -> f64 {
    let mut best = f64::INFINITY;
    for &p in a {
        best = best.min(point_triangle_distance(p, b));
        if best == 0.0 {
            return 0.0;
        }
    }
    for &q in b {
        best = best.min(point_triangle_distance(q, a));
        if best == 0.0 {
            return 0.0;
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            best = best.min(segment_segment_distance(
                a[i],
                a[(i + 1) % 3],
                b[j],
                b[(j + 1) % 3],
            ));
            if best == 0.0 {
                return 0.0;
            }
        }
    }
    for i in 0..3 {
        if segment_triangle_hit(a[i], a[(i + 1) % 3], b)
            || segment_triangle_hit(b[i], b[(i + 1) % 3], a)
        {
            return 0.0;
        }
    }
    best
}

fn point_segment_distance(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 == 0.0 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    (p - (a + ab * t)).length()
}

fn point_triangle_distance(p: DVec3, t: &Triangle) -> f64 {
    let [a, b, c] = *t;
    let ab = b - a;
    let ac = c - a;
    let normal = ab.cross(ac);
    let normal2 = normal.length_squared();
    if normal2 == 0.0 {
        return point_segment_distance(p, a, b)
            .min(point_segment_distance(p, b, c))
            .min(point_segment_distance(p, c, a));
    }
    let signed = (p - a).dot(normal) / normal2;
    let projection = p - normal * signed;
    let c1 = ab.cross(projection - a).dot(normal);
    let c2 = (c - b).cross(projection - b).dot(normal);
    let c3 = (a - c).cross(projection - c).dot(normal);
    let inside = (c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0) || (c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0);
    if inside {
        return (p - projection).length();
    }
    point_segment_distance(p, a, b)
        .min(point_segment_distance(p, b, c))
        .min(point_segment_distance(p, c, a))
}

fn segment_segment_distance(p1: DVec3, q1: DVec3, p2: DVec3, q2: DVec3) -> f64 {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    let (s, t);
    if a == 0.0 && e == 0.0 {
        return r.length();
    }
    if a == 0.0 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e == 0.0 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denominator = a * e - b * b;
            let mut s0 = if denominator > 1e-30 {
                ((b * f - c * e) / denominator).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s).distance(p2 + d2 * t)
}

fn segment_triangle_hit(p: DVec3, q: DVec3, t: &Triangle) -> bool {
    let [a, b, c] = *t;
    let direction = q - p;
    let edge1 = b - a;
    let edge2 = c - a;
    let h = direction.cross(edge2);
    let determinant = edge1.dot(h);
    if determinant.abs() <= 1e-30 {
        return false;
    }
    let inverse = 1.0 / determinant;
    let s = p - a;
    let u = inverse * s.dot(h);
    if u < -1e-9 || u > 1.0 + 1e-9 {
        return false;
    }
    let qv = s.cross(edge1);
    let v = inverse * direction.dot(qv);
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return false;
    }
    let distance = inverse * edge2.dot(qv);
    (-1e-9..=1.0 + 1e-9).contains(&distance)
}

#[derive(Clone)]
struct Item {
    index: usize,
    min: DVec3,
    max: DVec3,
    center: DVec3,
}

struct Node {
    min: DVec3,
    max: DVec3,
    kind: Kind,
}

enum Kind {
    Leaf(Vec<Item>),
    Branch(Box<Node>, Box<Node>),
}

struct Bvh {
    root: Option<Node>,
}

impl Bvh {
    fn new(triangles: &[Triangle]) -> Bvh {
        let items = triangles
            .iter()
            .enumerate()
            .filter(|(_, triangle)| triangle.iter().all(|q| q.is_finite()))
            .map(|(index, triangle)| {
                let mut min = DVec3::splat(f64::INFINITY);
                let mut max = DVec3::splat(f64::NEG_INFINITY);
                for q in triangle {
                    min = min.min(*q);
                    max = max.max(*q);
                }
                Item {
                    index,
                    min,
                    max,
                    center: (min + max) * 0.5,
                }
            })
            .collect();
        Bvh {
            root: build_node(items),
        }
    }
}

fn triangle_bounds(triangle: &Triangle) -> (DVec3, DVec3) {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    for q in triangle {
        min = min.min(*q);
        max = max.max(*q);
    }
    (min, max)
}

fn build_node(mut items: Vec<Item>) -> Option<Node> {
    if items.is_empty() {
        return None;
    }
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    for item in &items {
        min = min.min(item.min);
        max = max.max(item.max);
    }
    if items.len() <= 8 {
        return Some(Node {
            min,
            max,
            kind: Kind::Leaf(items),
        });
    }
    let extent = max - min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    items.sort_by(|a, b| {
        let a = match axis {
            0 => a.center.x,
            1 => a.center.y,
            _ => a.center.z,
        };
        let b = match axis {
            0 => b.center.x,
            1 => b.center.y,
            _ => b.center.z,
        };
        a.total_cmp(&b)
    });
    let right = items.split_off(items.len() / 2);
    Some(Node {
        min,
        max,
        kind: Kind::Branch(
            Box::new(build_node(items).expect("left half")),
            Box::new(build_node(right).expect("right half")),
        ),
    })
}

fn visit(node: &Node, lo: DVec3, hi: DVec3, found: &mut impl FnMut(usize)) {
    if node.max.x < lo.x
        || node.min.x > hi.x
        || node.max.y < lo.y
        || node.min.y > hi.y
        || node.max.z < lo.z
        || node.min.z > hi.z
    {
        return;
    }
    match &node.kind {
        Kind::Leaf(items) => {
            for item in items {
                if item.max.x >= lo.x
                    && item.min.x <= hi.x
                    && item.max.y >= lo.y
                    && item.min.y <= hi.y
                    && item.max.z >= lo.z
                    && item.min.z <= hi.z
                {
                    found(item.index);
                }
            }
        }
        Kind::Branch(left, right) => {
            visit(left, lo, hi, found);
            visit(right, lo, hi, found);
        }
    }
}

fn any_close(bvh: &Bvh, triangle: &Triangle, triangles: &[Triangle], clearance: f64) -> bool {
    let Some(root) = &bvh.root else {
        return false;
    };
    let (min, max) = triangle_bounds(triangle);
    let lo = min - DVec3::splat(clearance);
    let hi = max + DVec3::splat(clearance);
    let mut found = false;
    visit(root, lo, hi, &mut |index| {
        if triangle_distance(triangle, &triangles[index]) < clearance {
            found = true;
        }
    });
    found
}

/// The triangles of `a` and of `b` that lie within `clearance` (exclusive) of some
/// triangle of the other list.
pub fn close_triangles(a: &[Triangle], b: &[Triangle], clearance: f64) -> FitReport {
    if clearance <= 0.0 || !clearance.is_finite() || a.is_empty() || b.is_empty() {
        return FitReport::default();
    }
    let tree_a = Bvh::new(a);
    let tree_b = Bvh::new(b);
    let mut report = FitReport::default();
    for (index, triangle) in a.iter().enumerate() {
        if !triangle.iter().all(|q| q.is_finite()) {
            continue;
        }
        if any_close(&tree_b, triangle, b, clearance) {
            report.a.push(index);
        }
    }
    for (index, triangle) in b.iter().enumerate() {
        if !triangle.iter().all(|q| q.is_finite()) {
            continue;
        }
        if any_close(&tree_a, triangle, a, clearance) {
            report.b.push(index);
        }
    }
    report
}

/// Every triangle of the faces of `instance` (an instance in `body`) and of the instances
/// nested in it, placed by the instance's transform and then by `outer`: the triangles
/// `kernel_scene` draws for it.
pub fn instance_triangles(
    model: &km::Model,
    body: km::BodyId,
    instance: km::InstanceId,
    outer: DMat4,
) -> Vec<Triangle> {
    let Some(instance) = model.body(body).and_then(|body| body.instance(instance)) else {
        return Vec::new();
    };
    let mut triangles = Vec::new();
    collect_triangles(
        model,
        km::BodyId::Definition(instance.definition),
        outer * instance.transform,
        0,
        &mut triangles,
    );
    triangles
}

fn collect_triangles(
    model: &km::Model,
    body_id: km::BodyId,
    world: DMat4,
    depth: usize,
    triangles: &mut Vec<Triangle>,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let Some(body) = model.body(body_id) else {
        return;
    };
    let geometry = body.geometry();
    for (_, face) in geometry.faces() {
        let outer = geometry.loop_points(&face.outer);
        let normal = polygon_normal(&outer);
        if normal == DVec3::ZERO {
            continue;
        }
        let holes: Vec<Vec<DVec3>> = face
            .inner
            .iter()
            .map(|hole| geometry.loop_points(hole))
            .collect();
        if outer
            .iter()
            .chain(holes.iter().flatten())
            .any(|point| !point.is_finite())
        {
            continue;
        }
        let outer_2d = to_plane_2d(&outer, normal);
        let holes_2d: Vec<Vec<glam::DVec2>> =
            holes.iter().map(|hole| to_plane_2d(hole, normal)).collect();
        let faces = triangulate(&outer_2d, &holes_2d);
        let points: Vec<DVec3> = outer
            .iter()
            .chain(holes.iter().flatten())
            .copied()
            .collect();
        for [a, b, c] in faces {
            let triangle = [points[a], points[b], points[c]].map(|q| world.transform_point3(q));
            if triangle.iter().all(|q| q.is_finite()) {
                triangles.push(triangle);
            }
        }
    }
    for (_, child) in body.instances() {
        collect_triangles(
            model,
            km::BodyId::Definition(child.definition),
            world * child.transform,
            depth + 1,
            triangles,
        );
    }
}

fn as_dvec3(value: &[f32; 3]) -> DVec3 {
    DVec3::new(
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
    )
}

fn triangle_matches(scene: &Triangle, flagged: &Triangle) -> bool {
    const PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    PERMUTATIONS
        .iter()
        .any(|order| (0..3).all(|i| scene[i].distance(flagged[order[i]]) <= MATCH_TOLERANCE_M))
}

const MATCH_CELL: f64 = 2.0 * MATCH_TOLERANCE_M;

fn cell(value: f64) -> i64 {
    let cell = (value / MATCH_CELL).floor();
    if !cell.is_finite() {
        return 0;
    }
    cell.clamp(i64::MIN as f64, i64::MAX as f64) as i64
}

/// A copy of `scene` with every triangle that matches one of `flagged` moved into one batch
/// drawn in [`FIT_COLOR`].
pub fn tint_fit(scene: &FlatScene, flagged: &[Triangle]) -> FlatScene {
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (index, triangle) in flagged.iter().enumerate() {
        if !triangle.iter().all(|q| q.is_finite()) {
            continue;
        }
        for q in triangle {
            grid.entry((cell(q.x), cell(q.y), cell(q.z)))
                .or_default()
                .push(index);
        }
    }

    let matches = |scene: &Triangle| -> bool {
        if !scene.iter().all(|q| q.is_finite()) {
            return false;
        }
        let mut candidates = Vec::new();
        for q in scene {
            let (x, y, z) = (cell(q.x), cell(q.y), cell(q.z));
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(indices) = grid.get(&(
                            x.saturating_add(dx),
                            y.saturating_add(dy),
                            z.saturating_add(dz),
                        )) {
                            candidates.extend_from_slice(indices);
                        }
                    }
                }
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        candidates
            .into_iter()
            .any(|index| triangle_matches(scene, &flagged[index]))
    };

    let mut flags = Vec::with_capacity(scene.batches.len());
    let mut any = false;
    for batch in &scene.batches {
        let mut batch_flags = Vec::with_capacity(batch.indices.len() / 3);
        for triangle in batch.indices.as_chunks::<3>().0 {
            let (Some(a), Some(b), Some(c)) = (
                batch.positions.get(triangle[0] as usize),
                batch.positions.get(triangle[1] as usize),
                batch.positions.get(triangle[2] as usize),
            ) else {
                batch_flags.push(false);
                continue;
            };
            let triangle = [as_dvec3(a), as_dvec3(b), as_dvec3(c)];
            let flag = matches(&triangle);
            any |= flag;
            batch_flags.push(flag);
        }
        flags.push(batch_flags);
    }

    if !any {
        return scene.clone();
    }

    let mut materials = scene.materials.clone();
    materials.push(Material {
        id: "fit".to_owned(),
        name: "Fit".to_owned(),
        color: FIT_COLOR,
        texture: None,
        opacity: 1.0,
        unlit: false,
    });
    let material = Some(scene.materials.len());

    let mut batches = Vec::with_capacity(scene.batches.len() + 1);
    let mut fit = Batch {
        material,
        ..Batch::default()
    };
    for (batch, batch_flags) in scene.batches.iter().zip(flags) {
        let mut kept = Batch {
            material: batch.material,
            positions: batch.positions.clone(),
            normals: batch.normals.clone(),
            texcoords: batch.texcoords.clone(),
            indices: Vec::new(),
        };
        for (triangle, flagged) in batch.indices.as_chunks::<3>().0.iter().zip(batch_flags) {
            if flagged {
                for &index in triangle {
                    let index = index as usize;
                    let first = fit.positions.len() as u32;
                    fit.positions.push(batch.positions[index]);
                    fit.normals
                        .push(batch.normals.get(index).copied().unwrap_or([0.0; 3]));
                    fit.indices.push(first);
                }
            } else {
                kept.indices.extend_from_slice(triangle);
            }
        }
        if !kept.indices.is_empty() {
            batches.push(kept);
        }
    }
    batches.push(fit);

    FlatScene {
        materials,
        batches,
        lines: scene.lines.clone(),
        bounds: scene.bounds,
    }
}
