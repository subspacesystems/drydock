// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid check for 3D printing: open, loose and shared edges, internal and reversed faces,
//! shells and volume of one body.
//!
//! Behaviour: `docs/specs/m7-solid-check.md`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::{DVec2, DVec3};

use crate::geom;
use crate::model::{BodyId, Model};
use crate::tess;
use crate::topology::{Context, EdgeId, Face, FaceId};

/// What `check_geometry` found in one context. Every list is in ascending id order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidReport {
    /// Groups of faces joined across edges that exactly two faces use.
    pub shells: usize,
    /// Edges exactly one face uses (borders of holes in the surface).
    pub open_edges: Vec<EdgeId>,
    /// Edges no face uses.
    pub loose_edges: Vec<EdgeId>,
    /// Edges three or more faces use.
    pub shared_edges: Vec<EdgeId>,
    /// Faces whose every edge is a shared edge (walls inside the solid).
    pub internal_faces: Vec<FaceId>,
    /// Faces whose front points into the material instead of out of it.
    pub reversed_faces: Vec<FaceId>,
    /// Groups and components placed inside the body (`check_body` only).
    pub nested_instances: usize,
    /// Enclosed volume in cubic meters, with reversed faces counted the right way round;
    /// `Some` exactly when `is_solid`.
    pub volume: Option<f64>,
}

impl SolidReport {
    /// At least one shell, no open, loose or shared edges, no internal faces and no nested
    /// instances. Reversed faces don't count: see `is_clean`.
    pub fn is_solid(&self) -> bool {
        self.shells >= 1
            && self.open_edges.is_empty()
            && self.loose_edges.is_empty()
            && self.shared_edges.is_empty()
            && self.internal_faces.is_empty()
            && self.nested_instances == 0
    }

    /// Solid and no reversed faces: ready to export as it is.
    pub fn is_clean(&self) -> bool {
        self.is_solid() && self.reversed_faces.is_empty()
    }
}

fn loops(face: &Face) -> impl Iterator<Item = &crate::topology::Loop> {
    std::iter::once(&face.outer).chain(face.inner.iter())
}

fn face_area_vector(ctx: &Context, face: &Face) -> DVec3 {
    let mut sum = DVec3::ZERO;
    for l in loops(face) {
        sum += geom::area_vector(&ctx.loop_points(l));
    }
    sum
}

fn face_area(ctx: &Context, face: &Face) -> f64 {
    face_area_vector(ctx, face).length()
}

/// Signed volume of one face with its stored front as the front.
fn face_volume(ctx: &Context, face: &Face) -> f64 {
    let mut volume = 0.0;
    for l in loops(face) {
        let pts = ctx.loop_points(l);
        if pts.len() < 3 {
            continue;
        }
        let p0 = pts[0];
        for i in 1..pts.len() - 1 {
            volume += p0.dot(pts[i].cross(pts[i + 1])) / 6.0;
        }
    }
    volume
}

fn triangle_solid_angle(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    let numerator = a.dot(b.cross(c));
    let denominator = a.length() * b.length() * c.length()
        + a.dot(b) * c.length()
        + a.dot(c) * b.length()
        + b.dot(c) * a.length();
    2.0 * numerator.atan2(denominator)
}

/// Winding number of one face with its stored front as the front.
fn face_winding(ctx: &Context, face: &Face, point: DVec3) -> f64 {
    let mut sum = 0.0;
    for l in loops(face) {
        let pts = ctx.loop_points(l);
        if pts.len() < 3 {
            continue;
        }
        let a = pts[0] - point;
        for i in 1..pts.len() - 1 {
            let b = pts[i] - point;
            let c = pts[i + 1] - point;
            sum += triangle_solid_angle(a, b, c);
        }
    }
    sum / (4.0 * std::f64::consts::PI)
}

fn winding_sum(
    ctx: &Context,
    faces: &[FaceId],
    point: DVec3,
    turned: Option<&BTreeMap<FaceId, bool>>,
) -> f64 {
    let mut sum = 0.0;
    for &f in faces {
        let Some(face) = ctx.face(f) else {
            continue;
        };
        let w = face_winding(ctx, face, point);
        let is_turned = turned.and_then(|m| m.get(&f)).copied().unwrap_or(false);
        sum += if is_turned { -w } else { w };
    }
    sum
}

/// A point inside the shell, from the largest triangle of its lowest-id face.
fn shell_test_point(ctx: &Context, faces: &[FaceId]) -> DVec3 {
    let Some(f) = faces.iter().copied().min() else {
        return DVec3::ZERO;
    };
    let Some(face) = ctx.face(f) else {
        return DVec3::ZERO;
    };
    let area_vector = face_area_vector(ctx, face);
    let n = if area_vector.length() > 0.0 {
        area_vector.normalize()
    } else {
        ctx.face_plane(f).map(|p| p.normal).unwrap_or(DVec3::Z)
    };

    let outer = ctx.loop_points(&face.outer);
    let outer_2d = geom::to_plane_2d(&outer, n);
    let holes_2d: Vec<Vec<DVec2>> = face
        .inner
        .iter()
        .map(|l| geom::to_plane_2d(&ctx.loop_points(l), n))
        .collect();
    let mut points = outer_2d.clone();
    for hole in &holes_2d {
        points.extend_from_slice(hole);
    }

    let triangles = tess::triangulate(&outer_2d, &holes_2d);
    let mut best_area = -1.0;
    let mut best = DVec2::ZERO;
    for [a, b, c] in triangles {
        let (a, b, c) = (points[a], points[b], points[c]);
        let area = (b - a).perp_dot(c - a).abs() * 0.5;
        if area > best_area {
            best_area = area;
            best = (a + b + c) / 3.0;
        }
    }
    if best_area < 0.0 {
        best = outer_2d.iter().copied().sum::<DVec2>() / outer_2d.len().max(1) as f64;
    }

    let (u, v) = geom::plane_basis(n);
    let p = outer.first().copied().unwrap_or(DVec3::ZERO);
    best.x * u + best.y * v + n.dot(p) * n
}

fn orient_shell(
    faces: &[FaceId],
    neighbours: &BTreeMap<FaceId, Vec<(FaceId, EdgeId)>>,
    face_uses: &BTreeMap<FaceId, BTreeMap<EdgeId, bool>>,
) -> BTreeMap<FaceId, bool> {
    let mut turned = BTreeMap::new();
    let Some(&start) = faces.first() else {
        return turned;
    };
    turned.insert(start, false);
    let mut queue = VecDeque::from([start]);
    while let Some(f) = queue.pop_front() {
        let turned_f = turned[&f];
        let mut next = neighbours.get(&f).cloned().unwrap_or_default();
        next.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (g, e) in next {
            if turned.contains_key(&g) {
                continue;
            }
            let f_reversed = face_uses[&f][&e];
            let g_reversed = face_uses[&g][&e];
            turned.insert(g, (f_reversed == g_reversed) ^ turned_f);
            queue.push_back(g);
        }
    }
    turned
}

/// Checks the geometry of one context; `nested_instances` is 0.
pub fn check_geometry(ctx: &Context) -> SolidReport {
    let mut report = SolidReport::default();

    let mut edge_faces: BTreeMap<EdgeId, Vec<FaceId>> = BTreeMap::new();
    let mut two_use_edges = Vec::new();
    let mut shared_edges = BTreeSet::new();
    for (e, _) in ctx.edges() {
        let uses = ctx.faces_of_edge(e);
        match uses.len() {
            0 => report.loose_edges.push(e),
            1 => report.open_edges.push(e),
            2 => two_use_edges.push(e),
            _ => {
                report.shared_edges.push(e);
                shared_edges.insert(e);
            }
        }
        edge_faces.insert(e, uses);
    }

    // The direction of each face's use of each edge.
    let mut face_uses: BTreeMap<FaceId, BTreeMap<EdgeId, bool>> = BTreeMap::new();
    for (f, face) in ctx.faces() {
        let mut uses = BTreeMap::new();
        for l in loops(face) {
            for u in &l.uses {
                uses.insert(u.edge, u.reversed);
            }
        }
        face_uses.insert(f, uses);
    }

    for (f, face) in ctx.faces() {
        let internal = loops(face)
            .flat_map(|l| l.uses.iter())
            .all(|u| shared_edges.contains(&u.edge));
        if internal {
            report.internal_faces.push(f);
        }
    }

    let mut neighbours: BTreeMap<FaceId, Vec<(FaceId, EdgeId)>> = BTreeMap::new();
    for e in two_use_edges {
        if let [a, b] = edge_faces[&e].as_slice() {
            neighbours.entry(*a).or_default().push((*b, e));
            neighbours.entry(*b).or_default().push((*a, e));
        }
    }

    // Connected groups of faces joined across two-use edges.
    let mut visited: BTreeSet<FaceId> = BTreeSet::new();
    let mut shells: Vec<Vec<FaceId>> = Vec::new();
    for (f, _) in ctx.faces() {
        if !visited.insert(f) {
            continue;
        }
        let mut group = Vec::new();
        let mut stack = vec![f];
        while let Some(x) = stack.pop() {
            group.push(x);
            if let Some(next) = neighbours.get(&x) {
                for &(g, _) in next {
                    if visited.insert(g) {
                        stack.push(g);
                    }
                }
            }
        }
        group.sort_unstable();
        shells.push(group);
    }
    report.shells = shells.len();

    let mut reversed: BTreeSet<FaceId> = BTreeSet::new();
    let mut closed_shells: Vec<(Vec<FaceId>, BTreeMap<FaceId, bool>)> = Vec::new();

    for group in &shells {
        let turned = orient_shell(group, &neighbours, &face_uses);
        let members: BTreeSet<FaceId> = group.iter().copied().collect();
        let closed = group.iter().all(|f| {
            ctx.face(*f).is_some_and(|face| {
                loops(face).flat_map(|l| l.uses.iter()).all(|u| {
                    edge_faces
                        .get(&u.edge)
                        .is_some_and(|fs| fs.len() == 2 && fs.iter().all(|x| members.contains(x)))
                })
            })
        });

        if closed {
            closed_shells.push((group.clone(), turned));
        } else {
            let mut turned_area = 0.0;
            let mut unturned_area = 0.0;
            for &f in group {
                let Some(face) = ctx.face(f) else {
                    continue;
                };
                let area = face_area(ctx, face);
                if *turned.get(&f).unwrap_or(&false) {
                    turned_area += area;
                } else {
                    unturned_area += area;
                }
            }
            let keep_turned = turned_area > unturned_area + 1e-9 * (turned_area + unturned_area);
            for &f in group {
                let is_turned = *turned.get(&f).unwrap_or(&false);
                if is_turned != keep_turned {
                    reversed.insert(f);
                }
            }
        }
    }

    // Depth of each closed shell: how many other closed shells contain its test point.
    let test_points: Vec<DVec3> = closed_shells
        .iter()
        .map(|(group, _)| shell_test_point(ctx, group))
        .collect();
    let depths: Vec<usize> = test_points
        .iter()
        .enumerate()
        .map(|(i, &q)| {
            closed_shells
                .iter()
                .enumerate()
                .filter(|(j, (group, turned))| {
                    *j != i && winding_sum(ctx, group, q, Some(turned)).abs() > 0.5
                })
                .count()
        })
        .collect();

    let mut volume = 0.0;
    for (i, (group, turned)) in closed_shells.iter().enumerate() {
        let mut walked_volume = 0.0;
        for &f in group {
            let Some(face) = ctx.face(f) else {
                continue;
            };
            let v = face_volume(ctx, face);
            walked_volume += if *turned.get(&f).unwrap_or(&false) {
                -v
            } else {
                v
            };
        }
        let wants_positive = depths[i].is_multiple_of(2);
        let flip =
            (wants_positive && walked_volume < 0.0) || (!wants_positive && walked_volume > 0.0);
        volume += if flip { -walked_volume } else { walked_volume };
        for &f in group {
            let is_turned = *turned.get(&f).unwrap_or(&false);
            if is_turned ^ flip {
                reversed.insert(f);
            }
        }
    }

    report.reversed_faces = reversed.into_iter().collect();
    report.volume = if report.is_solid() {
        Some(volume)
    } else {
        None
    };
    report
}

/// Checks the body's geometry and counts its instances; `None` when the body doesn't exist.
pub fn check_body(model: &Model, body: BodyId) -> Option<SolidReport> {
    let body = model.body(body)?;
    let mut report = check_geometry(body.geometry());
    report.nested_instances = body.instances().count();
    if report.nested_instances != 0 {
        report.volume = None;
    }
    Some(report)
}

/// Winding number of `faces` (each counted with its front as stored) around `point`: about 1
/// inside a closed surface whose fronts point out, -1 inside one whose fronts point in, 0
/// outside.
pub fn winding_number(ctx: &Context, faces: &[FaceId], point: DVec3) -> f64 {
    winding_sum(ctx, faces, point, None)
}
