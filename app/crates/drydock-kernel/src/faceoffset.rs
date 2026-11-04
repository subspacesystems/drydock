// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Offset a face in its own plane: every boundary loop moves sideways by one distance, so the
//! face's region grows (positive distance) or shrinks (negative), with mitred corners.
//!
//! Behaviour: `docs/specs/m9-offset-face.md`. Not the M7 clearance offset in `offset.rs`, which
//! moves a solid's faces along their normals.

use std::collections::BTreeSet;

use glam::DVec3;

use crate::geom::{self, LINEAR_TOL};
use crate::model::{BodyId, FaceMaterials, Model, ModelEdit, ModelError, UndoStack};
use crate::topology::{Context, EdgeId, EdgeUse, Face, FaceId, Loop, Record, TopoError, Violation};

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

fn reverse_loop(loop_: &Loop) -> Loop {
    let mut uses: Vec<EdgeUse> = loop_.uses.iter().rev().copied().collect();
    for use_ in &mut uses {
        use_.reversed = !use_.reversed;
    }
    Loop { uses }
}

fn face_loops(face: &Face) -> Vec<Loop> {
    let mut loops = vec![face.outer.clone()];
    loops.extend(face.inner.iter().cloned());
    loops
}

struct LoopOffset {
    points: Vec<DVec3>,
    single: Vec<Option<usize>>,
}

fn offset_one(points: &[DVec3], n: DVec3, distance: f64) -> Result<LoopOffset, FaceOffsetError> {
    let k = points.len();
    let mut out = Vec::new();
    let mut starts = Vec::with_capacity(k);
    let mut single = Vec::with_capacity(k);
    for j in 0..k {
        let p = points[j];
        let e0 = (p - points[(j + k - 1) % k]).normalize_or_zero();
        let e1 = (points[(j + 1) % k] - p).normalize_or_zero();
        if e0 == DVec3::ZERO || e1 == DVec3::ZERO {
            return Err(FaceOffsetError::LoopsCross);
        }
        let n0 = e0.cross(n);
        let n1 = e1.cross(n);
        let c = n0.dot(n1);
        if c < -1.0 + 1e-9 {
            return Err(FaceOffsetError::LoopsCross);
        }
        let m = (n0 + n1) / (1.0 + c);
        let turn = e0.cross(e1).dot(n);
        starts.push(out.len());
        if distance * turn > 0.0 && m.length() > MITRE_LIMIT {
            single.push(None);
            out.push(p + n0 * distance);
            out.push(p + n1 * distance);
        } else {
            single.push(Some(out.len()));
            out.push(p + m * distance);
        }
    }

    for j in 0..k {
        let edge = points[(j + 1) % k] - points[j];
        let Some(e) = edge.try_normalize() else {
            return Err(FaceOffsetError::LoopsCross);
        };
        let next = (j + 1) % k;
        let last = if next == 0 {
            out.len() - 1
        } else {
            starts[next] - 1
        };
        let from = out[last];
        let to = out[starts[next]];
        if (to - from).dot(e) <= LINEAR_TOL {
            return Err(FaceOffsetError::Collapsed);
        }
    }

    let _ = starts;
    Ok(LoopOffset {
        points: out,
        single,
    })
}

fn offset_new_loops(
    ctx: &Context,
    face: FaceId,
    distance: f64,
) -> Result<Vec<LoopOffset>, FaceOffsetError> {
    let face_data = ctx
        .face(face)
        .cloned()
        .ok_or(FaceOffsetError::MissingFace(face))?;
    if !distance.is_finite() || distance.abs() <= LINEAR_TOL {
        return Err(FaceOffsetError::BadDistance);
    }
    let plane = ctx
        .face_plane(face)
        .ok_or(FaceOffsetError::NotPlanar(face))?;
    let n = plane.normal;
    let loops = face_loops(&face_data);

    let mut old_points = Vec::with_capacity(loops.len());
    for loop_ in &loops {
        let pts = ctx.loop_points(loop_);
        if geom::max_plane_deviation(&pts, &plane) > LINEAR_TOL {
            return Err(FaceOffsetError::NotPlanar(face));
        }
        old_points.push(pts);
    }

    let mut new_loops = Vec::with_capacity(loops.len());
    for pts in &old_points {
        new_loops.push(offset_one(pts, n, distance)?);
    }
    let new_points: Vec<&[DVec3]> = new_loops.iter().map(|loop_| &loop_.points[..]).collect();

    for poly in &new_points {
        let poly2 = geom::to_plane_2d(poly, n);
        if !geom::polygon_is_simple_2d(&poly2, LINEAR_TOL) {
            return Err(FaceOffsetError::LoopsCross);
        }
    }
    for a in 0..new_points.len() {
        for b in a + 1..new_points.len() {
            for i in 0..new_points[a].len() {
                let a0 = new_points[a][i];
                let a1 = new_points[a][(i + 1) % new_points[a].len()];
                for j in 0..new_points[b].len() {
                    let b0 = new_points[b][j];
                    let b1 = new_points[b][(j + 1) % new_points[b].len()];
                    if geom::segment_intersection(a0, a1, b0, b1, LINEAR_TOL)
                        != geom::SegmentHit::None
                    {
                        return Err(FaceOffsetError::LoopsCross);
                    }
                }
            }
        }
    }
    let outer2 = geom::to_plane_2d(new_points[0], n);
    for poly in new_points[1..].iter().copied() {
        for p in poly {
            let p2 = geom::to_plane_2d(std::slice::from_ref(p), n)[0];
            if geom::point_in_polygon_2d(p2, &outer2, LINEAR_TOL) != geom::Containment::Inside {
                return Err(FaceOffsetError::LoopsCross);
            }
        }
    }

    Ok(new_loops)
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
    offset_new_loops(ctx, face, distance)
        .map(|loops| loops.into_iter().map(|loop_| loop_.points).collect())
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
    offset_face_impl(model, body, face, distance, undo, label, false)
}

/// [`offset_face`], but removes a coplanar face that exactly fills one of the face's holes
/// before offsetting, so an outward offset can shrink a filled hole. The Offset tool uses
/// this; the faces filling the holes are gone afterwards.
pub fn offset_face_with_hole_fills(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<FaceOffset, FaceOffsetError> {
    offset_face_impl(model, body, face, distance, undo, label, true)
}

fn offset_face_impl(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
    fill_holes: bool,
) -> Result<FaceOffset, FaceOffsetError> {
    let body_ref = model.body(body).ok_or(FaceOffsetError::MissingBody(body))?;
    let ctx = body_ref.geometry();
    let old_face = ctx
        .face(face)
        .cloned()
        .ok_or(FaceOffsetError::MissingFace(face))?;
    let old_materials = body_ref.face_materials(face);
    let offsets = offset_new_loops(ctx, face, distance)?;
    let new_points: Vec<&[DVec3]> = offsets.iter().map(|offset| &offset.points[..]).collect();
    let old_loops = face_loops(&old_face);
    let old_vertices: Vec<Vec<crate::topology::VertexId>> = old_loops
        .iter()
        .map(|loop_| ctx.loop_vertices(loop_))
        .collect();
    let face_edges: BTreeSet<EdgeId> = old_loops
        .iter()
        .flat_map(|loop_| loop_.uses.iter().map(|use_| use_.edge))
        .collect();
    let face_curves: Vec<(crate::curve::Curve, usize)> = ctx
        .curves()
        .filter_map(|(id, curve)| {
            let edges = crate::curve::curve_edges(ctx, id);
            if edges.is_empty() || !edges.iter().all(|edge| face_edges.contains(edge)) {
                return None;
            }
            let at = old_loops.iter().position(|loop_| {
                let own: BTreeSet<EdgeId> = loop_.uses.iter().map(|use_| use_.edge).collect();
                edges.iter().all(|edge| own.contains(edge))
            })?;
            Some((curve.clone(), at))
        })
        .collect();
    let fill_faces: Vec<FaceId> = if fill_holes && distance > 0.0 {
        let hole_edges: Vec<BTreeSet<EdgeId>> = old_loops
            .iter()
            .skip(1)
            .map(|loop_| loop_.uses.iter().map(|use_| use_.edge).collect())
            .collect();
        ctx.faces()
            .filter(|(id, other)| {
                *id != face
                    && other.inner.is_empty()
                    && hole_edges.iter().any(|edges| {
                        other
                            .outer
                            .uses
                            .iter()
                            .map(|use_| use_.edge)
                            .collect::<BTreeSet<_>>()
                            == *edges
                    })
            })
            .map(|(id, _)| id)
            .collect()
    } else {
        Vec::new()
    };
    let before_violations = ctx.validate();
    let before_overlaps = ctx.face_overlaps();

    let mut edit = ModelEdit::new();
    let built = model
        .edit_geometry(body, &mut edit, |ctx, log| -> Result<_, TopoError> {
            let mut new_loops = Vec::with_capacity(new_points.len());
            let mut edges = Vec::with_capacity(new_points.len());
            let mut vertex_ids = Vec::with_capacity(new_points.len());
            for points in new_points.iter().copied() {
                let mut verts = Vec::with_capacity(points.len());
                for point in points {
                    verts.push(ctx.insert_vertex(*point, log));
                }
                let mut uses = Vec::with_capacity(verts.len());
                let mut ids = Vec::with_capacity(verts.len());
                for i in 0..verts.len() {
                    let edge = ctx.insert_edge(verts[i], verts[(i + 1) % verts.len()], log)?;
                    uses.push(EdgeUse::forward(edge));
                    ids.push(edge);
                }
                new_loops.push(Loop { uses });
                edges.push(ids);
                vertex_ids.push(verts);
            }
            for id in &fill_faces {
                ctx.delete_face(*id, log)?;
            }
            let mut hole_rings = Vec::with_capacity(new_loops.len().saturating_sub(1));
            let (inner, ring);
            if distance < 0.0 {
                let remove = Record::RemoveFace(face, old_face.clone());
                ctx.apply(&remove)?;
                log.records.push(remove);
                let readd = Record::AddFace(
                    face,
                    Face {
                        outer: old_loops[0].clone(),
                        inner: vec![reverse_loop(&new_loops[0])],
                    },
                );
                ctx.apply(&readd)?;
                log.records.push(readd);
                ring = face;
                inner = ctx.insert_face(
                    Face {
                        outer: new_loops[0].clone(),
                        inner: new_loops[1..].to_vec(),
                    },
                    log,
                )?;
                for (i, old) in old_loops.iter().enumerate().skip(1) {
                    hole_rings.push(ctx.insert_face(
                        Face {
                            outer: reverse_loop(&new_loops[i]),
                            inner: vec![old.clone()],
                        },
                        log,
                    )?);
                }
            } else {
                inner = face;
                ring = ctx.insert_face(
                    Face {
                        outer: new_loops[0].clone(),
                        inner: vec![reverse_loop(&old_loops[0])],
                    },
                    log,
                )?;
                for (i, old) in old_loops.iter().enumerate().skip(1) {
                    hole_rings.push(ctx.insert_face(
                        Face {
                            outer: reverse_loop(old),
                            inner: vec![new_loops[i].clone()],
                        },
                        log,
                    )?);
                }
            }

            for (curve, at) in &face_curves {
                let map = |vertex| {
                    let index = old_vertices[*at].iter().position(|old| *old == vertex)?;
                    let point = offsets[*at].single[index]?;
                    Some(vertex_ids[*at][point])
                };
                let mapped = |chain: &[_]| -> Option<Vec<_>> {
                    chain.iter().map(|vertex| map(*vertex)).collect()
                };
                let accept = |chain: &[_]| -> Option<crate::curve::Curve> {
                    let fitted = crate::curve::fit(ctx, chain, curve.start)?;
                    if fitted.center.distance(curve.center) > LINEAR_TOL {
                        return None;
                    }
                    let on_circle = chain.iter().all(|vertex| {
                        let Some(vertex) = ctx.vertex(*vertex) else {
                            return false;
                        };
                        let d = vertex.position - curve.center;
                        let radial = (d - curve.normal * d.dot(curve.normal)).length();
                        (radial - fitted.radius).abs() <= LINEAR_TOL
                            && d.dot(curve.normal).abs() <= LINEAR_TOL
                    });
                    on_circle.then_some(fitted)
                };
                let mut fitted = mapped(&curve.vertices).and_then(|chain| accept(&chain));
                if fitted.is_none() && !curve.is_closed() && curve.vertices.len() >= 5 {
                    fitted = mapped(&curve.vertices[1..curve.vertices.len() - 1])
                        .and_then(|chain| accept(&chain));
                }
                if let Some(fitted) = fitted {
                    ctx.insert_curve(fitted, log);
                }
            }

            Ok((inner, ring, hole_rings, edges))
        })
        .map_err(FaceOffsetError::Model)?;
    let (inner, ring, hole_rings, edges) = match built {
        Ok(value) => value,
        Err(error) => {
            let _ = model.apply_edit(&edit.inverse());
            return Err(FaceOffsetError::Model(ModelError::Topo(error)));
        }
    };

    if old_materials != FaceMaterials::default() {
        let mut faces: Vec<FaceId> = vec![inner, ring];
        faces.extend(hole_rings.iter().copied());
        let mut seen = BTreeSet::new();
        for id in faces {
            if !seen.insert(id) {
                continue;
            }
            let current = model
                .body(body)
                .ok_or(FaceOffsetError::MissingBody(body))?
                .face_materials(id);
            if current != old_materials
                && let Err(error) = model.set_face_materials(body, id, old_materials, &mut edit)
            {
                let _ = model.apply_edit(&edit.inverse());
                return Err(FaceOffsetError::Model(error));
            }
        }
    }

    let (new_violation, new_overlap) = {
        let ctx = model
            .body(body)
            .ok_or(FaceOffsetError::MissingBody(body))?
            .geometry();
        (
            ctx.validate()
                .into_iter()
                .find(|violation| !before_violations.contains(violation)),
            ctx.face_overlaps()
                .into_iter()
                .find(|pair| !before_overlaps.contains(pair)),
        )
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
    Ok(FaceOffset {
        inner,
        ring,
        hole_rings,
        edges,
    })
}
