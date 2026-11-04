// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fixtures shared by the sweep tests (`docs/specs/m9-sweep-face.md`).

#![allow(dead_code)]

use drydock_kernel::model::*;
use drydock_kernel::solid::{SolidReport, check_geometry};
use drydock_kernel::sweep::SweepPath;
use drydock_kernel::sweepface::*;
use drydock_kernel::topology::*;
use glam::DVec3;

pub const LABEL: &str = "Follow Me";

/// A path from bare points. Vertex ids are `1000 + i` and edge ids `2000 + i`: the sweep
/// reads only the points (and the vertex ids, to name a vertex in an error), so they need
/// not be in the body.
pub fn path(points: &[DVec3], closed: bool) -> SweepPath {
    let n = points.len();
    let edges = if closed { n } else { n.saturating_sub(1) };
    SweepPath {
        vertices: (0..n as u64).map(|i| VertexId(1000 + i)).collect(),
        points: points.to_vec(),
        edges: (0..edges as u64).map(|i| EdgeId(2000 + i)).collect(),
        closed,
    }
}

/// The same path run the other way (open: all reversed; closed: first point kept).
pub fn reversed(p: &SweepPath) -> SweepPath {
    let mut points: Vec<DVec3> = p.points.clone();
    let mut vertices: Vec<VertexId> = p.vertices.clone();
    if p.closed {
        points[1..].reverse();
        vertices[1..].reverse();
    } else {
        points.reverse();
        vertices.reverse();
    }
    let mut edges = p.edges.clone();
    edges.reverse();
    SweepPath {
        vertices,
        points,
        edges,
        closed: p.closed,
    }
}

pub fn sweep(
    model: &mut Model,
    undo: &mut UndoStack,
    profile: FaceId,
    p: &SweepPath,
) -> Result<Swept, SweepError> {
    sweep_face(model, BodyId::Root, profile, p, undo, LABEL)
}

pub fn solid(c: &Context) -> SolidReport {
    check_geometry(c)
}

/// Each loop of a face (outer first, then holes) as its points in loop order.
pub fn face_loops(c: &Context, f: FaceId) -> Vec<Vec<DVec3>> {
    let face = c.face(f).unwrap();
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .map(|l| c.loop_points(l))
        .collect()
}

/// Two point cycles are the same up to where they start (same direction), within `tol`.
pub fn same_cycle(a: &[DVec3], b: &[DVec3], tol: f64) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let n = a.len();
    (0..n).any(|shift| (0..n).all(|i| a[i].distance(b[(i + shift) % n]) <= tol))
}

/// Two faces are the same: same outer cycle, and their holes pair up.
pub fn same_face(a: &[Vec<DVec3>], b: &[Vec<DVec3>], tol: f64) -> bool {
    if a.len() != b.len() || !same_cycle(&a[0], &b[0], tol) {
        return false;
    }
    let mut used = vec![false; b.len()];
    a[1..].iter().all(|ha| {
        (1..b.len()).any(|j| {
            if !used[j] && same_cycle(ha, &b[j], tol) {
                used[j] = true;
                true
            } else {
                false
            }
        })
    })
}

/// The two contexts hold the same faces (as oriented point loops, ids ignored).
pub fn same_faces(a: &Context, b: &Context, tol: f64) -> bool {
    let fa: Vec<Vec<Vec<DVec3>>> = a.faces().map(|(f, _)| face_loops(a, f)).collect();
    let fb: Vec<Vec<Vec<DVec3>>> = b.faces().map(|(f, _)| face_loops(b, f)).collect();
    if fa.len() != fb.len() {
        return false;
    }
    let mut used = vec![false; fb.len()];
    fa.iter().all(|x| {
        (0..fb.len()).any(|j| {
            if !used[j] && same_face(x, &fb[j], tol) {
                used[j] = true;
                true
            } else {
                false
            }
        })
    })
}

/// The front normal of a face, unit length.
pub fn unit_normal(c: &Context, f: FaceId) -> DVec3 {
    c.face_plane(f).unwrap().normal.normalize()
}

/// Two unit vectors `u`, `w` across a plane with unit normal `n`, with `u x w = n`.
pub fn basis(n: DVec3) -> (DVec3, DVec3) {
    let helper = if n.z.abs() < 0.9 { DVec3::Z } else { DVec3::X };
    let u = helper.cross(n).normalize();
    let w = n.cross(u);
    (u, w)
}

/// A rectangle around `centre` in the plane spanned by `u` and `w` (unit, perpendicular):
/// `u` from `u0` to `u1`, `w` from `w0` to `w1`; counter-clockwise seen from `u x w`.
pub fn rect_points(
    centre: DVec3,
    u: DVec3,
    w: DVec3,
    u0: f64,
    u1: f64,
    w0: f64,
    w1: f64,
) -> [DVec3; 4] {
    [
        centre + u * u0 + w * w0,
        centre + u * u1 + w * w0,
        centre + u * u1 + w * w1,
        centre + u * u0 + w * w1,
    ]
}

/// A regular polygon around `centre` in the plane with unit normal `n`, counter-clockwise
/// seen from `n`, first corner at angle `phase`.
pub fn ngon(centre: DVec3, n: DVec3, radius: f64, sides: usize, phase: f64) -> Vec<DVec3> {
    let (u, w) = basis(n);
    (0..sides)
        .map(|i| {
            let a = phase + std::f64::consts::TAU * i as f64 / sides as f64;
            centre + (u * a.cos() + w * a.sin()) * radius
        })
        .collect()
}
