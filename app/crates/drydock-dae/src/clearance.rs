// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fit check for 3D printing: which triangles of two parts come closer to the other part
//! than the clearance, and a scene copy with those triangles tinted.
//!
//! Behaviour: `docs/specs/m7-clearance.md`.

use std::collections::HashMap;

use drydock_kernel::geom::{polygon_normal, to_plane_2d};
use drydock_kernel::model as km;
use drydock_kernel::tess::triangulate;
use glam::{DMat4, DVec3};

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
    if triangles_pierce(a, b) {
        return 0.0;
    }
    let mut best = point_triangle(a[0], b).min(point_triangle(a[1], b));
    best = best.min(point_triangle(a[2], b));
    best = best.min(point_triangle(b[0], a));
    best = best.min(point_triangle(b[1], a));
    best = best.min(point_triangle(b[2], a));
    best = best.min(segment_segment(a[0], a[1], b[0], b[1]));
    best = best.min(segment_segment(a[0], a[1], b[0], b[2]));
    best = best.min(segment_segment(a[0], a[1], b[1], b[2]));
    best = best.min(segment_segment(a[0], a[2], b[0], b[1]));
    best = best.min(segment_segment(a[0], a[2], b[0], b[2]));
    best = best.min(segment_segment(a[0], a[2], b[1], b[2]));
    best = best.min(segment_segment(a[1], a[2], b[0], b[1]));
    best = best.min(segment_segment(a[1], a[2], b[0], b[2]));
    best = best.min(segment_segment(a[1], a[2], b[1], b[2]));
    best
}

/// True when an edge of one triangle pierces the other triangle.
fn triangles_pierce(a: &Triangle, b: &Triangle) -> bool {
    let edges_a = [[a[0], a[1]], [a[1], a[2]], [a[2], a[0]]];
    let edges_b = [[b[0], b[1]], [b[1], b[2]], [b[2], b[0]]];
    edges_a
        .iter()
        .any(|edge| segment_triangle(edge[0], edge[1], b))
        || edges_b
            .iter()
            .any(|edge| segment_triangle(edge[0], edge[1], a))
}

/// True when the closed segment `p0..p1` meets the closed triangle `t` away from the
/// plane case (coplanar overlap is covered by the point and edge distances).
fn segment_triangle(p0: DVec3, p1: DVec3, t: &Triangle) -> bool {
    let dir = p1 - p0;
    let e1 = t[1] - t[0];
    let e2 = t[2] - t[0];
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() <= 1e-18 {
        return false;
    }
    let inv = 1.0 / det;
    let s = p0 - t[0];
    let u = s.dot(h) * inv;
    let eps = 1e-9;
    if u < -eps || u > 1.0 + eps {
        return false;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -eps || u + v > 1.0 + eps {
        return false;
    }
    let along = e2.dot(q) * inv;
    (-eps..=1.0 + eps).contains(&along)
}

/// The distance from a point `p` to the closed triangle `t`.
fn point_triangle(p: DVec3, t: &Triangle) -> f64 {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (u, v) = (b - a, c - a);
    let u2 = u.length_squared();
    let v2 = v.length_squared();
    let uv = u.dot(v);
    if u2 <= 1e-30 && v2 <= 1e-30 {
        return p.distance(a);
    }
    if u2 <= 1e-30 {
        return p.distance(b).min(point_segment(p, b, c));
    }
    if v2 <= 1e-30 {
        return p.distance(c).min(point_segment(p, a, b));
    }
    let up = u.dot(p - a);
    let vp = v.dot(p - a);
    let det = u2 * v2 - uv * uv;
    if det <= 1e-30 {
        return point_segment(p, a, b)
            .min(point_segment(p, a, c))
            .min(point_segment(p, b, c));
    }
    let s = (v2 * up - uv * vp) / det;
    let tw = (u2 * vp - uv * up) / det;
    if s >= 0.0 && tw >= 0.0 && s + tw <= 1.0 {
        return (p - (a + u * s + v * tw)).length();
    }
    let d_ab = (p - (a + u * s.clamp(0.0, 1.0))).length();
    let d_ac = (p - (a + v * tw.clamp(0.0, 1.0))).length();
    let d_bc = point_segment(p, b, c);
    let d_min_ab = if s < 0.0 {
        (p - a).length()
    } else if s > 1.0 {
        (p - b).length()
    } else {
        d_ab
    };
    let d_min_ac = if tw < 0.0 {
        (p - a).length()
    } else if tw > 1.0 {
        (p - c).length()
    } else {
        d_ac
    };
    d_min_ab.min(d_min_ac).min(d_bc)
}

/// The distance from a point `p` to the segment `a..b`.
fn point_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let v = b - a;
    let t = v.length_squared().max(1e-30);
    let s = ((p - a).dot(v) / t).clamp(0.0, 1.0);
    (p - (a + v * s)).length()
}

/// The distance between the two closed segments `p1..q1` and `p2..q2`, 0 when they cross
/// or share a point.
fn segment_segment(p1: DVec3, q1: DVec3, p2: DVec3, q2: DVec3) -> f64 {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    let (mut s, mut t);
    if a <= 1e-30 && e <= 1e-30 {
        return p1.distance(p2);
    }
    if a <= 1e-30 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= 1e-30 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            s = if denom != 0.0 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
        }
    }
    (p1 + d1 * s).distance(p2 + d2 * t)
}

/// The triangles of `a` and of `b` that lie within `clearance` (exclusive) of some
/// triangle of the other list.
pub fn close_triangles(a: &[Triangle], b: &[Triangle], clearance: f64) -> FitReport {
    if clearance.is_nan() || clearance <= 0.0 {
        return FitReport::default();
    }
    let a_tree = Bvh::new(a);
    let b_tree = Bvh::new(b);

    let mut report = FitReport::default();
    for (i, triangle) in a.iter().enumerate() {
        if !finite(triangle) {
            continue;
        }
        if b_tree.close_to(b, triangle, clearance) {
            report.a.push(i);
        }
    }
    for (j, triangle) in b.iter().enumerate() {
        if !finite(triangle) {
            continue;
        }
        if a_tree.close_to(a, triangle, clearance) {
            report.b.push(j);
        }
    }
    report
}

fn finite(triangle: &Triangle) -> bool {
    triangle.iter().all(|point| point.is_finite())
}

fn triangle_bounds(triangle: &Triangle) -> [DVec3; 2] {
    let mut min = triangle[0];
    let mut max = triangle[0];
    for point in &triangle[1..] {
        min = min.min(*point);
        max = max.max(*point);
    }
    [min, max]
}

/// True when the two axis-aligned bounding boxes are within `d` of each other.
fn aabb_close(a: [DVec3; 2], b: [DVec3; 2], d: f64) -> bool {
    let axes = [
        (a[0].x, a[1].x, b[0].x, b[1].x),
        (a[0].y, a[1].y, b[0].y, b[1].y),
        (a[0].z, a[1].z, b[0].z, b[1].z),
    ];
    for (a_min, a_max, b_min, b_max) in axes {
        if a_max < b_min - d || b_max < a_min - d {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
struct BvhItem {
    index: usize,
    bounds: [DVec3; 2],
}

#[derive(Clone, Copy)]
struct BvhNode {
    bounds: [DVec3; 2],
    left: u32,
    right: u32,
    start: u32,
    end: u32,
}

/// A bounding volume hierarchy over the finite triangles of one list, so that
/// [`close_triangles`] only measures nearby pairs.
struct Bvh {
    nodes: Vec<BvhNode>,
    items: Vec<BvhItem>,
}

impl Bvh {
    fn new(triangles: &[Triangle]) -> Bvh {
        let mut items: Vec<BvhItem> = triangles
            .iter()
            .enumerate()
            .filter(|(_, triangle)| finite(triangle))
            .map(|(index, triangle)| BvhItem {
                index,
                bounds: triangle_bounds(triangle),
            })
            .collect();
        let mut nodes = Vec::new();
        if !items.is_empty() {
            build_bvh(&mut items, 0, &mut nodes);
        }
        Bvh { nodes, items }
    }

    /// True when some triangle of `triangles` is closer to `query` than `clearance`.
    fn close_to(&self, triangles: &[Triangle], query: &Triangle, clearance: f64) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.node_close_to(0, triangles, query, clearance)
    }

    fn node_close_to(
        &self,
        node: usize,
        triangles: &[Triangle],
        query: &Triangle,
        clearance: f64,
    ) -> bool {
        let node = &self.nodes[node];
        // A little slack keeps rounding in the box test from pruning a pair whose exact
        // distance is just under the clearance.
        let reach = clearance + clearance.abs() * 1e-12 + f64::MIN_POSITIVE;
        if !aabb_close(node.bounds, triangle_bounds(query), reach) {
            return false;
        }
        if node.left == u32::MAX {
            let items = &self.items[node.start as usize..node.end as usize];
            return items
                .iter()
                .any(|item| triangle_distance(query, &triangles[item.index]) < clearance);
        }
        self.node_close_to(node.left as usize, triangles, query, clearance)
            || self.node_close_to(node.right as usize, triangles, query, clearance)
    }
}

fn build_bvh(items: &mut [BvhItem], offset: usize, nodes: &mut Vec<BvhNode>) -> usize {
    let node = nodes.len();
    nodes.push(BvhNode {
        bounds: [DVec3::ZERO; 2],
        left: u32::MAX,
        right: u32::MAX,
        start: 0,
        end: 0,
    });
    let bounds = items.iter().skip(1).fold(items[0].bounds, |bounds, item| {
        [bounds[0].min(item.bounds[0]), bounds[1].max(item.bounds[1])]
    });

    if items.len() <= 4 {
        nodes[node].bounds = bounds;
        nodes[node].start = offset as u32;
        nodes[node].end = (offset + items.len()) as u32;
        return node;
    }

    let extent = bounds[1] - bounds[0];
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    items.sort_by(|a, b| {
        let ac = (a.bounds[0] + a.bounds[1]).to_array()[axis];
        let bc = (b.bounds[0] + b.bounds[1]).to_array()[axis];
        ac.total_cmp(&bc)
    });
    let mid = items.len() / 2;
    let (lower, upper) = items.split_at_mut(mid);
    let left = build_bvh(lower, offset, nodes);
    let right = build_bvh(upper, offset + mid, nodes);
    nodes[node] = BvhNode {
        bounds,
        left: left as u32,
        right: right as u32,
        start: 0,
        end: 0,
    };
    node
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
    let mut out = Vec::new();
    collect_instance(
        model,
        instance.definition,
        outer * instance.transform,
        0,
        &mut out,
    );
    out
}

/// How deep instances may nest before the walk stops, as in `kernel_scene`.
const MAX_DEPTH: usize = 64;

fn collect_instance(
    model: &km::Model,
    definition: km::DefinitionId,
    world: DMat4,
    depth: usize,
    out: &mut Vec<Triangle>,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let Some(body) = model.body(km::BodyId::Definition(definition)) else {
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
        let world_outer: Vec<DVec3> = outer
            .iter()
            .map(|&point| world.transform_point3(point))
            .collect();
        let world_holes: Vec<Vec<DVec3>> = holes
            .iter()
            .map(|hole| {
                hole.iter()
                    .map(|&point| world.transform_point3(point))
                    .collect()
            })
            .collect();
        if world_outer
            .iter()
            .chain(world_holes.iter().flatten())
            .any(|point| !point.is_finite())
        {
            continue;
        }
        let outer_2d = to_plane_2d(&outer, normal);
        let holes_2d: Vec<Vec<glam::DVec2>> =
            holes.iter().map(|hole| to_plane_2d(hole, normal)).collect();
        let triangles = triangulate(&outer_2d, &holes_2d);
        let positions: Vec<DVec3> = world_outer
            .iter()
            .chain(world_holes.iter().flatten())
            .copied()
            .collect();
        for [a, b, c] in triangles {
            if let (Some(&a), Some(&b), Some(&c)) =
                (positions.get(a), positions.get(b), positions.get(c))
            {
                out.push([a, b, c]);
            }
        }
    }
    for (_, child) in body.instances() {
        collect_instance(
            model,
            child.definition,
            world * child.transform,
            depth + 1,
            out,
        );
    }
}

/// A copy of `scene` with every triangle that matches one of `flagged` moved into one batch
/// drawn in [`FIT_COLOR`].
pub fn tint_fit(scene: &FlatScene, flagged: &[Triangle]) -> FlatScene {
    let mut grid = FlagGrid::new(flagged);

    let mut flags = Vec::with_capacity(scene.batches.len());
    let mut any = false;
    for batch in &scene.batches {
        let mut batch_flags = Vec::with_capacity(batch.indices.len() / 3);
        for triangle in batch.indices.as_chunks::<3>().0 {
            let matched =
                scene_triangle(batch, triangle).is_some_and(|triangle| grid.matches(&triangle));
            any |= matched;
            batch_flags.push(matched);
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
        for (triangle, matched) in batch.indices.as_chunks::<3>().0.iter().zip(batch_flags) {
            if matched {
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

/// The scene triangle behind three indices, or `None` when an index is out of range or a
/// corner is not finite.
fn scene_triangle(batch: &Batch, indices: &[u32; 3]) -> Option<Triangle> {
    let mut triangle = [DVec3::ZERO; 3];
    for (slot, &index) in triangle.iter_mut().zip(indices) {
        let position = batch.positions.get(index as usize)?;
        *slot = DVec3::new(
            f64::from(position[0]),
            f64::from(position[1]),
            f64::from(position[2]),
        );
    }
    finite(&triangle).then_some(triangle)
}

/// About one millimetre, in meters.
const MATCH_CELL_M: f64 = 0.001;

/// A hash grid of the flagged triangles' corners for fast approximate lookup.
struct FlagGrid {
    triangles: Vec<Triangle>,
    cells: HashMap<(i64, i64, i64), Vec<usize>>,
    seen: Vec<u32>,
    stamp: u32,
}

impl FlagGrid {
    fn new(flagged: &[Triangle]) -> FlagGrid {
        let triangles: Vec<Triangle> = flagged
            .iter()
            .filter(|triangle| finite(triangle))
            .copied()
            .collect();
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (index, triangle) in triangles.iter().enumerate() {
            for point in triangle {
                cells.entry(cell_key(*point)).or_default().push(index);
            }
        }
        let seen = vec![0; triangles.len()];
        FlagGrid {
            triangles,
            cells,
            seen,
            stamp: 1,
        }
    }

    fn matches(&mut self, scene: &Triangle) -> bool {
        let stamp = self.stamp;
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.seen.fill(0);
            self.stamp = 1;
        }
        let anchor = cell_key(scene[0]);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = (
                        anchor.0.saturating_add(dx),
                        anchor.1.saturating_add(dy),
                        anchor.2.saturating_add(dz),
                    );
                    let Some(indices) = self.cells.get(&key) else {
                        continue;
                    };
                    for &index in indices {
                        if self.seen[index] == stamp {
                            continue;
                        }
                        self.seen[index] = stamp;
                        if corners_match(scene, &self.triangles[index]) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

fn cell_key(point: DVec3) -> (i64, i64, i64) {
    (
        (point.x / MATCH_CELL_M).floor() as i64,
        (point.y / MATCH_CELL_M).floor() as i64,
        (point.z / MATCH_CELL_M).floor() as i64,
    )
}

/// Every pairing of the flagged triangle's corners with the scene triangle's corners: the
/// three rotations and both windings.
const CORNER_PAIRINGS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [1, 2, 0],
    [2, 0, 1],
    [0, 2, 1],
    [2, 1, 0],
    [1, 0, 2],
];

fn corners_match(scene: &Triangle, flagged: &Triangle) -> bool {
    let tolerance = MATCH_TOLERANCE_M * MATCH_TOLERANCE_M;
    CORNER_PAIRINGS.iter().any(|pairing| {
        (0..3).all(|corner| scene[corner].distance_squared(flagged[pairing[corner]]) <= tolerance)
    })
}
