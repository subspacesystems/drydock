// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Offset a face in its own plane: every boundary loop moves sideways by one distance, so the
//! face's region grows (positive distance) or shrinks (negative), with mitred corners.
//!
//! Behaviour: `docs/specs/m9-offset-face.md`. Not the M7 clearance offset in `offset.rs`, which
//! moves a solid's faces along their normals.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DVec2, DVec3};

use crate::curve::{self, Curve};
use crate::geom::{self, LINEAR_TOL};
use crate::model::{BodyId, Model, ModelEdit, ModelError, UndoStack};
use crate::topology::{
    Context, EdgeId, EdgeUse, Face, FaceId, Loop, Record, TopoError, VertexId, Violation,
};

/// Longest mitre, as a multiple of the distance. A corner whose mitre would be longer (an
/// interior angle under about 29 degrees on the side the loop moves away from) is bevelled.
pub const MITRE_LIMIT: f64 = 4.0;

/// Why an offset was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum FaceOffsetError {
    MissingBody(BodyId),
    MissingFace(FaceId),
    /// The distance is not finite, or not longer than `LINEAR_TOL`.
    BadDistance,
    /// The face has no plane, or one of its loop points is off it by more than `LINEAR_TOL`.
    NotPlanar(FaceId),
    /// An offset edge would shrink to `LINEAR_TOL` or less, or turn around: the distance is
    /// too large for the loop (inward for the outer loop, outward for a hole).
    Collapsed,
    /// The offset loops are not a face outline: a loop crosses or touches itself, two loops
    /// cross or touch, or an offset hole is not strictly inside the offset outer loop.
    LoopsCross,
    /// The result would break a `validate` invariant that held before.
    Invalid(Violation),
    /// The result would make two coplanar faces overlap that didn't before (lower id first).
    Overlap(FaceId, FaceId),
    Model(ModelError),
}

/// The faces an offset left. Every face fronts the way the offset face did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceOffset {
    /// The face inside the offset outer loop. Inward: a new face bounded by the offset outer
    /// loop, with the offset holes. Outward: the original face, unchanged.
    pub inner: FaceId,
    /// The ring between the two outer loops. Inward: the original face (same id), now its old
    /// outer loop with the offset outer loop as its only hole. Outward: a new face, the offset
    /// outer loop with the old outer loop as its only hole.
    pub ring: FaceId,
    /// One new face per hole of the original face, in the face's hole order, between the old
    /// hole loop and its offset.
    pub hole_rings: Vec<FaceId>,
    /// The new edges of each offset loop: outer loop first, then each hole in order; each list
    /// in the old loop's direction, starting at the offset of the loop's first vertex.
    pub edges: Vec<Vec<EdgeId>>,
}

struct LoopOffset {
    old_vertices: Vec<VertexId>,
    points: Vec<DVec3>,
    vertex_point: Vec<Option<usize>>,
}

fn face_loops(face: &Face) -> Vec<Loop> {
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .cloned()
        .collect()
}

fn reverse_loop(loop_: &Loop) -> Loop {
    let mut uses: Vec<EdgeUse> = loop_.uses.iter().rev().copied().collect();
    for use_ in &mut uses {
        use_.reversed = !use_.reversed;
    }
    Loop { uses }
}

fn offset_loop(
    ctx: &Context,
    loop_: &Loop,
    n: DVec3,
    distance: f64,
) -> Result<LoopOffset, FaceOffsetError> {
    let old_vertices = ctx.loop_vertices(loop_);
    let points = ctx.loop_points(loop_);
    let k = points.len();
    let mut corners: Vec<Vec<DVec3>> = Vec::with_capacity(k);

    for j in 0..k {
        let prev = points[(j + k - 1) % k];
        let next = points[(j + 1) % k];
        let a = points[j] - prev;
        let b = next - points[j];
        if a.length() == 0.0 || b.length() == 0.0 {
            return Err(FaceOffsetError::LoopsCross);
        }
        let e0 = a.normalize();
        let e1 = b.normalize();
        let n0 = e0.cross(n);
        let n1 = e1.cross(n);
        let c = n0.dot(n1);
        if c < -1.0 + 1e-9 {
            return Err(FaceOffsetError::LoopsCross);
        }
        let m = (n0 + n1) / (1.0 + c);
        let turn = e0.cross(e1).dot(n);
        if distance * turn > 0.0 && m.length() > MITRE_LIMIT {
            corners.push(vec![points[j] + distance * n0, points[j] + distance * n1]);
        } else {
            corners.push(vec![points[j] + distance * m]);
        }
    }

    for j in 0..k {
        let e = (points[(j + 1) % k] - points[j]).normalize();
        let from = *corners[j].last().expect("a corner has points");
        let to = corners[(j + 1) % k][0];
        if (to - from).dot(e) <= LINEAR_TOL {
            return Err(FaceOffsetError::Collapsed);
        }
    }

    let mut new_points = Vec::new();
    let mut vertex_point = Vec::with_capacity(k);
    for corner in &corners {
        if corner.len() == 1 {
            vertex_point.push(Some(new_points.len()));
        } else {
            vertex_point.push(None);
        }
        new_points.extend_from_slice(corner);
    }

    Ok(LoopOffset {
        old_vertices,
        points: new_points,
        vertex_point,
    })
}

fn to_3d(p: DVec2) -> DVec3 {
    DVec3::new(p.x, p.y, 0.0)
}

fn prepare_offsets(
    ctx: &Context,
    face: FaceId,
    distance: f64,
) -> Result<(Vec<LoopOffset>, DVec3), FaceOffsetError> {
    let face_data = ctx.face(face).ok_or(FaceOffsetError::MissingFace(face))?;
    if !distance.is_finite() || distance.abs() <= LINEAR_TOL {
        return Err(FaceOffsetError::BadDistance);
    }
    let plane = ctx
        .face_plane(face)
        .ok_or(FaceOffsetError::NotPlanar(face))?;
    let n = plane.normal;
    let loops = face_loops(face_data);
    for loop_ in &loops {
        if geom::max_plane_deviation(&ctx.loop_points(loop_), &plane) > LINEAR_TOL {
            return Err(FaceOffsetError::NotPlanar(face));
        }
    }

    let offsets: Vec<LoopOffset> = loops
        .iter()
        .map(|loop_| offset_loop(ctx, loop_, n, distance))
        .collect::<Result<_, _>>()?;

    let polys: Vec<Vec<DVec2>> = offsets
        .iter()
        .map(|offset| geom::to_plane_2d(&offset.points, n))
        .collect();
    for poly in &polys {
        if !geom::polygon_is_simple_2d(poly, LINEAR_TOL) {
            return Err(FaceOffsetError::LoopsCross);
        }
    }
    for i in 0..polys.len() {
        for j in i + 1..polys.len() {
            let a = &polys[i];
            let b = &polys[j];
            for ai in 0..a.len() {
                let a0 = to_3d(a[ai]);
                let a1 = to_3d(a[(ai + 1) % a.len()]);
                for bi in 0..b.len() {
                    let b0 = to_3d(b[bi]);
                    let b1 = to_3d(b[(bi + 1) % b.len()]);
                    if geom::segment_intersection(a0, a1, b0, b1, LINEAR_TOL)
                        != geom::SegmentHit::None
                    {
                        return Err(FaceOffsetError::LoopsCross);
                    }
                }
            }
        }
    }
    if let Some(outer) = polys.first() {
        for hole in &polys[1..] {
            for p in hole {
                if geom::point_in_polygon_2d(*p, outer, LINEAR_TOL) != geom::Containment::Inside {
                    return Err(FaceOffsetError::LoopsCross);
                }
            }
        }
    }

    Ok((offsets, n))
}

fn curve_is_concentric(ctx: &Context, old: &Curve, chain: &[VertexId], new: &Curve) -> bool {
    if new.center.distance(old.center) > LINEAR_TOL {
        return false;
    }
    for id in chain {
        let Some(vertex) = ctx.vertex(*id) else {
            return false;
        };
        let d = vertex.position - new.center;
        let h = d.dot(new.normal);
        let rho = (d - new.normal * h).length();
        if (rho - new.radius).abs() > LINEAR_TOL || h.abs() > LINEAR_TOL {
            return false;
        }
    }
    true
}

/// The points of `face`'s loops offset by `distance` in the face plane (positive grows the
/// face), without changing anything: outer loop first, then each hole in order, each in the
/// old loop's direction starting at the offset of its first vertex; a bevelled corner gives
/// two points. Fails as [`offset_face`] does, up to and including `LoopsCross`.
pub fn offset_loops(
    ctx: &Context,
    face: FaceId,
    distance: f64,
) -> Result<Vec<Vec<DVec3>>, FaceOffsetError> {
    let (offsets, _) = prepare_offsets(ctx, face, distance)?;
    Ok(offsets.into_iter().map(|offset| offset.points).collect())
}

/// Offset `face` of `body` by `distance` in its plane (positive outward), as one undo step
/// labelled `label`.
pub fn offset_face(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<FaceOffset, FaceOffsetError> {
    let body_ref = model.body(body).ok_or(FaceOffsetError::MissingBody(body))?;
    let (face_data, old_materials, face_curves, before_violations, before_overlaps) = {
        let ctx = body_ref.geometry();
        let face_data = ctx
            .face(face)
            .cloned()
            .ok_or(FaceOffsetError::MissingFace(face))?;
        let old_materials = body_ref.face_materials(face);
        let loops = face_loops(&face_data);
        let mut edges = BTreeSet::new();
        for loop_ in &loops {
            for use_ in &loop_.uses {
                edges.insert(use_.edge);
            }
        }
        let face_curves: Vec<Curve> = ctx
            .curves()
            .filter_map(|(id, curve)| {
                let curve_edges = curve::curve_edges(ctx, id);
                (!curve_edges.is_empty() && curve_edges.iter().all(|edge| edges.contains(edge)))
                    .then(|| curve.clone())
            })
            .collect();
        let before_violations = ctx.validate();
        let before_overlaps = ctx.face_overlaps();
        (
            face_data,
            old_materials,
            face_curves,
            before_violations,
            before_overlaps,
        )
    };

    let (offsets, _) = {
        let ctx = model
            .body(body)
            .ok_or(FaceOffsetError::MissingBody(body))?
            .geometry();
        prepare_offsets(ctx, face, distance)?
    };

    let mut edit = ModelEdit::new();
    let built = model.edit_geometry(
        body,
        &mut edit,
        |ctx, log| -> Result<FaceOffset, TopoError> {
            let mut new_vertices: Vec<Vec<VertexId>> = Vec::with_capacity(offsets.len());
            let mut new_edges: Vec<Vec<EdgeId>> = Vec::with_capacity(offsets.len());
            let mut new_loops: Vec<Loop> = Vec::with_capacity(offsets.len());
            for offset in &offsets {
                let vertices: Vec<VertexId> = offset
                    .points
                    .iter()
                    .map(|p| ctx.insert_vertex(*p, log))
                    .collect();
                let mut edges = Vec::with_capacity(vertices.len());
                for i in 0..vertices.len() {
                    edges.push(ctx.insert_edge(
                        vertices[i],
                        vertices[(i + 1) % vertices.len()],
                        log,
                    )?);
                }
                new_loops.push(Loop {
                    uses: edges.iter().map(|edge| EdgeUse::forward(*edge)).collect(),
                });
                new_vertices.push(vertices);
                new_edges.push(edges);
            }

            let old_loops = face_loops(&face_data);
            let old_outer = old_loops[0].clone();
            let old_holes = &old_loops[1..];

            let result = if distance < 0.0 {
                ctx.delete_face(face, log)?;
                let readd = Record::AddFace(
                    face,
                    Face {
                        outer: old_outer.clone(),
                        inner: vec![reverse_loop(&new_loops[0])],
                    },
                );
                ctx.apply(&readd)?;
                log.records.push(readd);
                let inner = ctx.insert_face(
                    Face {
                        outer: new_loops[0].clone(),
                        inner: new_loops[1..].to_vec(),
                    },
                    log,
                )?;
                let mut hole_rings = Vec::with_capacity(old_holes.len());
                for (i, old_hole) in old_holes.iter().enumerate() {
                    hole_rings.push(ctx.insert_face(
                        Face {
                            outer: reverse_loop(&new_loops[i + 1]),
                            inner: vec![old_hole.clone()],
                        },
                        log,
                    )?);
                }
                FaceOffset {
                    inner,
                    ring: face,
                    hole_rings,
                    edges: new_edges,
                }
            } else {
                let ring = ctx.insert_face(
                    Face {
                        outer: new_loops[0].clone(),
                        inner: vec![reverse_loop(&old_outer)],
                    },
                    log,
                )?;
                let mut hole_rings = Vec::with_capacity(old_holes.len());
                for (i, old_hole) in old_holes.iter().enumerate() {
                    hole_rings.push(ctx.insert_face(
                        Face {
                            outer: reverse_loop(old_hole),
                            inner: vec![new_loops[i + 1].clone()],
                        },
                        log,
                    )?);
                }
                FaceOffset {
                    inner: face,
                    ring,
                    hole_rings,
                    edges: new_edges,
                }
            };

            let mut vertex_map: BTreeMap<VertexId, VertexId> = BTreeMap::new();
            for (i, offset) in offsets.iter().enumerate() {
                for (j, old) in offset.old_vertices.iter().enumerate() {
                    if let Some(point) = offset.vertex_point[j] {
                        vertex_map.insert(*old, new_vertices[i][point]);
                    }
                }
            }

            for old in &face_curves {
                let mut mapped = None;
                if let Some(chain) = old
                    .vertices
                    .iter()
                    .map(|v| vertex_map.get(v).copied())
                    .collect::<Option<Vec<_>>>()
                    && let Some(new) = curve::fit(ctx, &chain, old.start)
                    && curve_is_concentric(ctx, old, &chain, &new)
                {
                    mapped = Some(new);
                }
                if mapped.is_none() && !old.is_closed() && old.vertices.len() >= 5 {
                    let middle = &old.vertices[1..old.vertices.len() - 1];
                    if let Some(chain) = middle
                        .iter()
                        .map(|v| vertex_map.get(v).copied())
                        .collect::<Option<Vec<_>>>()
                        && let Some(new) = curve::fit(ctx, &chain, old.start)
                        && curve_is_concentric(ctx, old, &chain, &new)
                    {
                        mapped = Some(new);
                    }
                }
                if let Some(new) = mapped {
                    ctx.insert_curve(new, log);
                }
            }

            Ok(result)
        },
    );

    let result = match built {
        Ok(Ok(result)) => result,
        Ok(Err(err)) => {
            let _ = model.apply_edit(&edit.inverse());
            return Err(FaceOffsetError::Model(ModelError::Topo(err)));
        }
        Err(err) => {
            let _ = model.apply_edit(&edit.inverse());
            return Err(FaceOffsetError::Model(err));
        }
    };

    let mut faces: Vec<FaceId> = vec![result.inner, result.ring];
    faces.extend(result.hole_rings.iter().copied());
    for g in faces {
        let current = model
            .body(body)
            .ok_or(FaceOffsetError::MissingBody(body))?
            .face_materials(g);
        if current != old_materials
            && let Err(err) = model.set_face_materials(body, g, old_materials, &mut edit)
        {
            let _ = model.apply_edit(&edit.inverse());
            return Err(FaceOffsetError::Model(err));
        }
    }

    let (new_violation, new_overlap) = {
        let ctx = model
            .body(body)
            .ok_or(FaceOffsetError::MissingBody(body))?
            .geometry();
        let new_violation = ctx
            .validate()
            .into_iter()
            .find(|v| !before_violations.contains(v));
        let new_overlap = ctx
            .face_overlaps()
            .into_iter()
            .find(|pair| !before_overlaps.contains(pair));
        (new_violation, new_overlap)
    };

    if let Some(violation) = new_violation {
        let _ = model.apply_edit(&edit.inverse());
        return Err(FaceOffsetError::Invalid(violation));
    }
    if let Some((a, b)) = new_overlap {
        let _ = model.apply_edit(&edit.inverse());
        return Err(FaceOffsetError::Overlap(a, b));
    }

    undo.push(label, edit);
    Ok(result)
}
