// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Grow or shrink a flat-faced solid: every face moves along its outward normal by the same
//! distance, so a part prints larger or smaller by a fit clearance.
//!
//! Behaviour: `docs/specs/m7-clearance-offset.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat3, DVec2, DVec3};

use crate::geom;
use crate::model::{BodyId, Model, ModelEdit, UndoStack};
use crate::solid::{SolidReport, check_body, check_geometry};
use crate::topology::{Context, EdgeId, Edit, Face, FaceId, VertexId};

/// Two outward normals at a vertex belong to the same plane when their dot product is
/// above this.
pub const SAME_PLANE_DOT: f64 = 1.0 - 1e-9;

/// A normal adds a direction to a vertex's basis when its cross product with the first
/// basis normal (second pick) or its triple product with the first two (third pick) is
/// larger than this.
pub const RANK_TOL: f64 = 1e-6;

/// A vertex's move fits a plane when its distance along that plane's normal is within
/// `RESIDUAL_REL * |distance|` of the distance.
pub const RESIDUAL_REL: f64 = 1e-6;

/// Why a body can't be grown or shrunk. Nothing changed. Id lists are in ascending order.
#[derive(Clone, Debug, PartialEq)]
pub enum OffsetError {
    /// The distance is zero, NaN or infinite.
    BadDistance,
    /// The body doesn't exist.
    MissingBody(BodyId),
    /// The body isn't a solid (`SolidReport::is_solid`); the check says why.
    NotSolid(Box<SolidReport>),
    /// The body has this many arcs or circles: only flat-faced parts can be offset.
    Curved(usize),
    /// Vertices whose faces' moved planes don't meet in one point.
    Unsupported(Vec<VertexId>),
    /// Edges that would shrink to nothing or turn round, and faces whose loops would; the
    /// part is thinner than twice the distance there, or a hole or cavity would close.
    Collapse {
        edges: Vec<EdgeId>,
        faces: Vec<FaceId>,
    },
    /// The moved surface would touch or cross itself (a gap or slot would close).
    SelfIntersect,
}

/// What an offset did.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetReport {
    /// Vertices moved (every vertex of the body).
    pub moved: usize,
    /// Volume in cubic meters before the offset.
    pub volume_before: f64,
    /// Volume in cubic meters after the offset.
    pub volume_after: f64,
}

/// Where every vertex of `ctx` goes when each face moves `distance` along its outward
/// normal (positive grows, negative shrinks), after every check in the spec. `ctx` is not
/// changed.
pub fn offset_positions(
    ctx: &Context,
    distance: f64,
) -> Result<BTreeMap<VertexId, DVec3>, OffsetError> {
    if !distance.is_finite() || distance == 0.0 {
        return Err(OffsetError::BadDistance);
    }

    let report = check_geometry(ctx);
    if !report.is_solid() {
        return Err(OffsetError::NotSolid(Box::new(report)));
    }

    let curved = ctx.curve_count();
    if curved > 0 {
        return Err(OffsetError::Curved(curved));
    }

    let mut normals: BTreeMap<FaceId, DVec3> = BTreeMap::new();
    let mut no_plane: BTreeSet<FaceId> = BTreeSet::new();
    for (f, _) in ctx.faces() {
        match ctx.face_plane(f) {
            Some(plane) => {
                let mut normal = plane.normal;
                if report.reversed_faces.binary_search(&f).is_ok() {
                    normal = -normal;
                }
                normals.insert(f, normal);
            }
            None => {
                no_plane.insert(f);
            }
        }
    }

    let mut moves: BTreeMap<VertexId, DVec3> = BTreeMap::new();
    let mut unsupported = Vec::new();
    for (v, vertex) in ctx.vertices() {
        let mut faces = BTreeSet::new();
        for e in ctx.edges_of_vertex(v) {
            faces.extend(ctx.faces_of_edge(e));
        }
        if faces.is_empty() || faces.iter().any(|f| no_plane.contains(f)) {
            unsupported.push(v);
            continue;
        }

        let mut kept: Vec<DVec3> = Vec::new();
        for f in faces {
            let normal = normals[&f];
            if kept.iter().any(|k| k.dot(normal) > SAME_PLANE_DOT) {
                continue;
            }
            kept.push(normal);
        }

        match solve_move(&kept, distance) {
            Some(m) => {
                moves.insert(v, vertex.position + m);
            }
            None => unsupported.push(v),
        }
    }
    if !unsupported.is_empty() {
        return Err(OffsetError::Unsupported(unsupported));
    }

    let mut collapsed_edges = Vec::new();
    for (e, edge) in ctx.edges() {
        let Some(a0) = ctx.vertex(edge.ends[0]) else {
            continue;
        };
        let Some(b0) = ctx.vertex(edge.ends[1]) else {
            continue;
        };
        let (Some(&a1), Some(&b1)) = (moves.get(&edge.ends[0]), moves.get(&edge.ends[1])) else {
            continue;
        };
        let old = b0.position - a0.position;
        let new = b1 - a1;
        if new.length() <= geom::LINEAR_TOL || new.dot(old) <= 0.0 {
            collapsed_edges.push(e);
        }
    }

    let mut collapsed_faces = Vec::new();
    for (f, face) in ctx.faces() {
        let mut collapses = false;
        for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
            let old_points = ctx.loop_points(l);
            let new_points: Vec<DVec3> = ctx
                .loop_vertices(l)
                .into_iter()
                .filter_map(|v| moves.get(&v).copied())
                .collect();
            let old_area = geom::area_vector(&old_points);
            let new_area = geom::area_vector(&new_points);
            if new_area.length() <= 1e-12 || new_area.dot(old_area) <= 0.0 {
                collapses = true;
                break;
            }
        }
        if collapses {
            collapsed_faces.push(f);
        }
    }
    if !collapsed_edges.is_empty() || !collapsed_faces.is_empty() {
        return Err(OffsetError::Collapse {
            edges: collapsed_edges,
            faces: collapsed_faces,
        });
    }

    let mut moved = ctx.clone();
    let mut scratch = Edit::new();
    for (&v, &p) in &moves {
        moved
            .set_vertex_position(v, p, &mut scratch)
            .expect("a vertex of the context exists");
    }

    if !moved.validate().is_empty() {
        return Err(OffsetError::SelfIntersect);
    }
    if !moved.face_overlaps().is_empty() {
        return Err(OffsetError::SelfIntersect);
    }
    if edge_pierces_face(&moved) {
        return Err(OffsetError::SelfIntersect);
    }
    let after = check_geometry(&moved);
    if !after.is_solid() || after.shells != report.shells {
        return Err(OffsetError::SelfIntersect);
    }

    Ok(moves)
}

/// Offset the body's geometry by `distance`, appending the vertex moves to `log`. On an
/// error the model and `log` are unchanged.
pub fn offset_body(
    model: &mut Model,
    body: BodyId,
    distance: f64,
    log: &mut ModelEdit,
) -> Result<OffsetReport, OffsetError> {
    if !distance.is_finite() || distance == 0.0 {
        return Err(OffsetError::BadDistance);
    }
    let Some(current) = model.body(body) else {
        return Err(OffsetError::MissingBody(body));
    };
    let report = check_body(model, body).ok_or(OffsetError::MissingBody(body))?;
    if !report.is_solid() {
        return Err(OffsetError::NotSolid(Box::new(report)));
    }

    let moves = offset_positions(current.geometry(), distance)?;
    let moved = moves.len();
    let volume_before = report.volume.unwrap_or(0.0);

    model
        .edit_geometry(body, log, |ctx, edit| {
            for (&v, &p) in &moves {
                ctx.set_vertex_position(v, p, edit)
                    .expect("a vertex of the context exists");
            }
        })
        .map_err(|_| OffsetError::MissingBody(body))?;

    let after = check_body(model, body).ok_or(OffsetError::MissingBody(body))?;
    Ok(OffsetReport {
        moved,
        volume_before,
        volume_after: after.volume.unwrap_or(0.0),
    })
}

/// Offset each body by `distance`, all or nothing per body; the bodies that succeed go into
/// one undo step named `label` (none when no body changed). One result per entry of
/// `bodies`; a body listed again is offset once and repeats its first result.
pub fn offset_bodies(
    model: &mut Model,
    bodies: &[BodyId],
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Vec<Result<OffsetReport, OffsetError>> {
    let mut edit = ModelEdit::new();
    let mut results = Vec::with_capacity(bodies.len());
    let mut cache: BTreeMap<BodyId, Result<OffsetReport, OffsetError>> = BTreeMap::new();
    let mut changed = false;

    for &body in bodies {
        if let Some(result) = cache.get(&body) {
            results.push(result.clone());
            continue;
        }
        let result = offset_body(model, body, distance, &mut edit);
        if result.is_ok() {
            changed = true;
        }
        cache.insert(body, result.clone());
        results.push(result);
    }

    if changed {
        undo.push(label, edit);
    }
    results
}

/// The move that puts a vertex on every kept plane, or `None` when they don't meet in one
/// point.
fn solve_move(kept: &[DVec3], distance: f64) -> Option<DVec3> {
    let b1 = kept[0];

    let mut b2 = None;
    let mut best_cross = RANK_TOL;
    for &n in kept {
        let cross = b1.cross(n).length();
        if cross > best_cross {
            best_cross = cross;
            b2 = Some(n);
        }
    }

    let m = if let Some(b2) = b2 {
        let cross = b1.cross(b2);
        let mut b3 = None;
        let mut best_triple = RANK_TOL;
        for &n in kept {
            let triple = cross.dot(n).abs();
            if triple > best_triple {
                best_triple = triple;
                b3 = Some(n);
            }
        }
        if let Some(b3) = b3 {
            let matrix = DMat3::from_cols(b1, b2, b3).transpose();
            let inverse = matrix.inverse();
            if !inverse.is_finite() {
                return None;
            }
            inverse * DVec3::splat(distance)
        } else {
            let a = distance / (1.0 + b1.dot(b2));
            a * (b1 + b2)
        }
    } else {
        distance * b1
    };

    let tolerance = RESIDUAL_REL * distance.abs();
    for &n in kept {
        if (n.dot(m) - distance).abs() > tolerance {
            return None;
        }
    }
    Some(m)
}

/// Every vertex used by any loop of `face`.
fn face_vertices(ctx: &Context, face: &Face) -> BTreeSet<VertexId> {
    let mut out = BTreeSet::new();
    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
        out.extend(ctx.loop_vertices(l));
    }
    out
}

/// Whether any edge of `ctx` crosses or touches a face it doesn't belong to.
fn edge_pierces_face(ctx: &Context) -> bool {
    let tol = geom::LINEAR_TOL;
    for (_, edge) in ctx.edges() {
        let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
            continue;
        };
        let (a, b) = (a.position, b.position);
        let edge_lo = a.min(b) - DVec3::splat(tol);
        let edge_hi = a.max(b) + DVec3::splat(tol);

        for (f, face) in ctx.faces() {
            let vertices = face_vertices(ctx, face);
            if vertices.contains(&edge.ends[0]) || vertices.contains(&edge.ends[1]) {
                continue;
            }
            let Some(plane) = ctx.face_plane(f) else {
                continue;
            };

            let mut points = Vec::new();
            for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                points.extend(ctx.loop_points(l));
            }
            if points.is_empty() {
                continue;
            }
            let mut face_lo = DVec3::splat(f64::INFINITY);
            let mut face_hi = DVec3::splat(f64::NEG_INFINITY);
            for p in &points {
                face_lo = face_lo.min(*p);
                face_hi = face_hi.max(*p);
            }
            face_lo -= DVec3::splat(tol);
            face_hi += DVec3::splat(tol);
            if edge_lo.x > face_hi.x
                || edge_hi.x < face_lo.x
                || edge_lo.y > face_hi.y
                || edge_hi.y < face_lo.y
                || edge_lo.z > face_hi.z
                || edge_hi.z < face_lo.z
            {
                continue;
            }

            let da = plane.signed_distance(a);
            let db = plane.signed_distance(b);
            let mut candidates = Vec::new();
            if (da > tol && db < -tol) || (da < -tol && db > tol) {
                let t = da / (da - db);
                candidates.push(a + (b - a) * t);
            }
            if da.abs() <= tol {
                candidates.push(a);
            }
            if db.abs() <= tol {
                candidates.push(b);
            }
            if candidates.is_empty() {
                continue;
            }

            let normal = plane.normal;
            let outer = geom::to_plane_2d(&ctx.loop_points(&face.outer), normal);
            let holes: Vec<Vec<DVec2>> = face
                .inner
                .iter()
                .map(|l| geom::to_plane_2d(&ctx.loop_points(l), normal))
                .collect();
            for candidate in candidates {
                let p = geom::to_plane_2d(&[candidate], normal)[0];
                if geom::point_in_polygon_2d(p, &outer, tol) == geom::Containment::Outside {
                    continue;
                }
                let inside_hole = holes
                    .iter()
                    .any(|h| geom::point_in_polygon_2d(p, h, tol) == geom::Containment::Inside);
                if !inside_hole {
                    return true;
                }
            }
        }
    }
    false
}
