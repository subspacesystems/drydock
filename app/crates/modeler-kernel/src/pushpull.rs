// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Push/Pull: extrude a face along its normal, or slide it when its neighbours stretch.
//!
//! Behaviour: `docs/specs/m3-pushpull.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec3;

use crate::geom::{self, LINEAR_TOL};
use crate::model::{BodyId, FaceMaterials, Model, ModelEdit, ModelError, UndoStack};
use crate::topology::{
    Context, EdgeId, EdgeUse, Face, FaceId, Loop, Record, TopoError, VertexId, Violation,
};

/// Why a push/pull was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum PushPullError {
    MissingBody(BodyId),
    MissingFace(FaceId),
    /// The distance is not finite, or not longer than `LINEAR_TOL`.
    BadDistance,
    /// The result would break a `validate` invariant that held before.
    Invalid(Violation),
    /// The result would make two coplanar faces overlap that didn't before (lower id first).
    Overlap(FaceId, FaceId),
    /// A slide would turn this face (a stretched neighbour) inside out.
    Inverted(FaceId),
    Model(ModelError),
}

/// The faces a push/pull left in place of the pushed face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushPulled {
    /// The face at the new position. The pushed face itself when it slid.
    pub cap: FaceId,
    /// New side faces, one per loop use: outer loop first, then each hole, in loop order.
    /// Empty when the face slid.
    pub sides: Vec<FaceId>,
    /// The face left at the old position, for a face that shared no edge with another face.
    pub base: Option<FaceId>,
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

fn use_ends(ctx: &Context, use_: EdgeUse) -> Option<(VertexId, VertexId)> {
    let edge = ctx.edge(use_.edge)?;
    Some(if use_.reversed {
        (edge.ends[1], edge.ends[0])
    } else {
        (edge.ends[0], edge.ends[1])
    })
}

/// Every point lies strictly inside `face`'s outer loop and outside all of its holes.
fn points_inside_face(ctx: &Context, points: &[DVec3], face: &Face, normal: DVec3) -> bool {
    let outer = geom::to_plane_2d(&ctx.loop_points(&face.outer), normal);
    let holes: Vec<Vec<glam::DVec2>> = face
        .inner
        .iter()
        .map(|l| geom::to_plane_2d(&ctx.loop_points(l), normal))
        .collect();
    points.iter().all(|p| {
        let p2d = geom::to_plane_2d(std::slice::from_ref(p), normal)[0];
        geom::point_in_polygon_2d(p2d, &outer, LINEAR_TOL) == geom::Containment::Inside
            && holes.iter().all(|hole| {
                geom::point_in_polygon_2d(p2d, hole, LINEAR_TOL) == geom::Containment::Outside
            })
    })
}

fn roll_back(model: &mut Model, edit: &ModelEdit) {
    let _ = model.apply_edit(&edit.inverse());
}

/// Push or pull `face` of `body` by `distance` along its front normal (negative goes
/// backwards), as one undo step labelled `label`.
pub fn push_pull(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<PushPulled, PushPullError> {
    let body_ref = model.body(body).ok_or(PushPullError::MissingBody(body))?;
    let ctx = body_ref.geometry();
    let face_data = ctx
        .face(face)
        .cloned()
        .ok_or(PushPullError::MissingFace(face))?;
    if !distance.is_finite() || distance.abs() <= LINEAR_TOL {
        return Err(PushPullError::BadDistance);
    }

    let plane = ctx
        .face_plane(face)
        .ok_or(PushPullError::Invalid(Violation::NonPlanarFace(face)))?;
    let n = plane.normal;
    let q = plane.point;
    let t = distance * n;
    let old_materials = body_ref.face_materials(face);

    let loops = face_loops(&face_data);
    let mut edges = BTreeSet::new();
    for l in &loops {
        for u in &l.uses {
            edges.insert(u.edge);
        }
    }

    let isolated = edges
        .iter()
        .all(|e| ctx.faces_of_edge(*e).iter().all(|f| *f == face));

    let is_stretch = |e: EdgeId| -> bool {
        let faces = ctx.faces_of_edge(e);
        let mut others = faces.iter().copied().filter(|f| *f != face);
        let Some(other) = others.next() else {
            return false;
        };
        if others.next().is_some() {
            return false;
        }
        let Some(other_plane) = ctx.face_plane(other) else {
            return false;
        };
        let m = other_plane.normal;
        if m.dot(n).abs() > 1e-6 {
            return false;
        }
        let Some(other_face) = ctx.face(other) else {
            return false;
        };
        ctx.loop_points(&other_face.outer)
            .iter()
            .all(|p| n.dot(*p - q) <= LINEAR_TOL)
    };

    let slide = edges.iter().all(|e| is_stretch(*e));

    let before_violations = ctx.validate();
    let before_overlaps = ctx.face_overlaps();

    let mut edit = ModelEdit::new();

    if slide {
        let mut moved: BTreeSet<VertexId> = BTreeSet::new();
        for l in &loops {
            for v in ctx.loop_vertices(l) {
                moved.insert(v);
            }
        }

        let mut affected: BTreeSet<FaceId> = BTreeSet::new();
        for &v in &moved {
            for e in ctx.edges_of_vertex(v) {
                for f in ctx.faces_of_edge(e) {
                    affected.insert(f);
                }
            }
        }
        let before_normals: Vec<(FaceId, DVec3)> = affected
            .iter()
            .filter_map(|f| ctx.face_plane(*f).map(|p| (*f, p.normal)))
            .collect();

        model
            .edit_geometry(body, &mut edit, |ctx, log| {
                for &v in &moved {
                    if let Some(vertex) = ctx.vertex(v) {
                        let _ = ctx.set_vertex_position(v, vertex.position + t, log);
                    }
                }
            })
            .map_err(PushPullError::Model)?;

        let (new_violation, inverted, new_overlap) = {
            let ctx = model
                .body(body)
                .ok_or(PushPullError::MissingBody(body))?
                .geometry();
            let new_violation = ctx
                .validate()
                .into_iter()
                .find(|v| !before_violations.contains(v));
            let inverted = before_normals.iter().find_map(|(f, m1)| {
                let m2 = ctx.face_plane(*f)?.normal;
                (m2.dot(*m1) < 0.0).then_some(*f)
            });
            let new_overlap = ctx
                .face_overlaps()
                .into_iter()
                .find(|p| !before_overlaps.contains(p));
            (new_violation, inverted, new_overlap)
        };

        if let Some(v) = new_violation {
            roll_back(model, &edit);
            return Err(PushPullError::Invalid(v));
        }
        if let Some(f) = inverted {
            roll_back(model, &edit);
            return Err(PushPullError::Inverted(f));
        }
        if let Some((a, b)) = new_overlap {
            roll_back(model, &edit);
            return Err(PushPullError::Overlap(a, b));
        }

        undo.push(label, edit);
        return Ok(PushPulled {
            cap: face,
            sides: Vec::new(),
            base: None,
        });
    }

    let reversed = isolated && distance < 0.0;
    let working: Vec<Loop> = if reversed {
        loops.iter().map(reverse_loop).collect()
    } else {
        loops.clone()
    };

    let built = model
        .edit_geometry(body, &mut edit, |ctx, log| -> Result<_, TopoError> {
            let mut new_vertex: BTreeMap<VertexId, VertexId> = BTreeMap::new();
            let mut bottom_edge: BTreeMap<VertexId, EdgeId> = BTreeMap::new();

            for l in &working {
                for u in &l.uses {
                    let (a, _) = use_ends(ctx, *u).ok_or(TopoError::MissingEdge(u.edge))?;
                    if let std::collections::btree_map::Entry::Vacant(entry) = new_vertex.entry(a) {
                        let position =
                            ctx.vertex(a).ok_or(TopoError::MissingVertex(a))?.position + t;
                        let new = ctx.insert_vertex(position, log);
                        let edge = ctx.insert_edge(a, new, log)?;
                        entry.insert(new);
                        bottom_edge.insert(a, edge);
                    }
                }
            }

            let mut cap_loops: Vec<Loop> = Vec::new();
            let mut side_data: Vec<(EdgeUse, EdgeId, EdgeId, EdgeId)> = Vec::new();
            for l in &working {
                let mut cap_uses = Vec::with_capacity(l.uses.len());
                for u in &l.uses {
                    let (a, b) = use_ends(ctx, *u).ok_or(TopoError::MissingEdge(u.edge))?;
                    let top = ctx.insert_edge(new_vertex[&a], new_vertex[&b], log)?;
                    cap_uses.push(EdgeUse::forward(top));
                    side_data.push((*u, top, bottom_edge[&b], bottom_edge[&a]));
                }
                cap_loops.push(Loop { uses: cap_uses });
            }

            let mut cap_loops = cap_loops.into_iter();
            let cap = ctx.insert_face(
                Face {
                    outer: cap_loops.next().ok_or(TopoError::MissingFace(face))?,
                    inner: cap_loops.collect(),
                },
                log,
            )?;

            let bottoms: Vec<EdgeId> = side_data.iter().map(|(old, ..)| old.edge).collect();
            let mut sides = Vec::with_capacity(side_data.len());
            for (old, top, b_bottom, a_bottom) in side_data {
                let loop_ = Loop {
                    uses: vec![
                        old,
                        EdgeUse::forward(b_bottom),
                        EdgeUse::backward(top),
                        EdgeUse::backward(a_bottom),
                    ],
                };
                sides.push(ctx.insert_face(
                    Face {
                        outer: loop_,
                        inner: Vec::new(),
                    },
                    log,
                )?);
            }

            ctx.delete_face(face, log)?;

            let base = if isolated {
                let mut reversed_loops = working.iter().map(reverse_loop);
                let outer = reversed_loops.next().ok_or(TopoError::MissingFace(face))?;
                Some(ctx.insert_face(
                    Face {
                        outer,
                        inner: reversed_loops.collect(),
                    },
                    log,
                )?)
            } else {
                None
            };

            Ok((cap, sides, bottoms, base))
        })
        .map_err(PushPullError::Model)?;
    let (cap, mut sides, side_bottoms, base) = match built {
        Ok(v) => v,
        Err(e) => {
            roll_back(model, &edit);
            return Err(PushPullError::Model(ModelError::Topo(e)));
        }
    };

    let mut cap_materials = old_materials;
    if reversed {
        std::mem::swap(&mut cap_materials.front, &mut cap_materials.back);
    }
    if cap_materials != FaceMaterials::default()
        && let Err(e) = model.set_face_materials(body, cap, cap_materials, &mut edit)
    {
        roll_back(model, &edit);
        return Err(PushPullError::Model(e));
    }

    let mut new_faces: BTreeSet<FaceId> = sides.iter().copied().collect();
    new_faces.insert(cap);
    if let Some(base) = base {
        new_faces.insert(base);
    }

    let mut final_cap = cap;
    let healed = model.edit_geometry(body, &mut edit, |ctx, log| -> Result<(), TopoError> {
        for (i, side_slot) in sides.iter_mut().enumerate() {
            let side = *side_slot;
            let Some(&bottom) = side_bottoms.get(i) else {
                continue;
            };
            let faces = ctx.faces_of_edge(bottom);
            if faces.len() != 2 {
                continue;
            }
            let Some(other) = faces.iter().copied().find(|f| *f != side) else {
                continue;
            };
            if new_faces.contains(&other) {
                continue;
            }
            let (Some(side_plane), Some(other_plane)) =
                (ctx.face_plane(side), ctx.face_plane(other))
            else {
                continue;
            };
            if side_plane.normal.dot(other_plane.normal) <= 0.0
                || !crate::faces::same_plane(&side_plane, &other_plane)
            {
                continue;
            }
            if ctx.erase_edge(bottom, log).is_err() {
                continue;
            }
            *side_slot = if ctx.face(side).is_some() {
                side
            } else {
                other
            };
        }

        let Some(cap_face) = ctx.face(final_cap).cloned() else {
            return Ok(());
        };
        if !cap_face.inner.is_empty() {
            return Ok(());
        }
        let Some(cap_plane) = ctx.face_plane(final_cap) else {
            return Ok(());
        };
        let cap_points = ctx.loop_points(&cap_face.outer);

        let mut target = None;
        for (f, face) in ctx.faces() {
            if f == final_cap {
                continue;
            }
            let Some(plane) = ctx.face_plane(f) else {
                continue;
            };
            if cap_plane.normal.dot(plane.normal) >= 0.0
                || !crate::faces::same_plane(&cap_plane, &plane)
            {
                continue;
            }
            if points_inside_face(ctx, &cap_points, face, plane.normal) {
                target = Some(f);
                break;
            }
        }

        if let Some(f) = target {
            let Some(target_face) = ctx.face(f).cloned() else {
                return Ok(());
            };
            ctx.delete_face(final_cap, log)?;
            let mut punched = target_face.clone();
            punched.inner.push(cap_face.outer.clone());
            let remove = Record::RemoveFace(f, target_face);
            ctx.apply(&remove)?;
            log.records.push(remove);
            let add = Record::AddFace(f, punched);
            ctx.apply(&add)?;
            log.records.push(add);
            final_cap = f;
        }
        Ok(())
    });
    if let Err(e) = healed {
        roll_back(model, &edit);
        return Err(PushPullError::Model(e));
    }

    let (new_violation, new_overlap) = {
        let ctx = model
            .body(body)
            .ok_or(PushPullError::MissingBody(body))?
            .geometry();
        let new_violation = ctx
            .validate()
            .into_iter()
            .find(|v| !before_violations.contains(v));
        let new_overlap = ctx
            .face_overlaps()
            .into_iter()
            .find(|p| !before_overlaps.contains(p));
        (new_violation, new_overlap)
    };

    if let Some(v) = new_violation {
        roll_back(model, &edit);
        return Err(PushPullError::Invalid(v));
    }
    if let Some((a, b)) = new_overlap {
        roll_back(model, &edit);
        return Err(PushPullError::Overlap(a, b));
    }

    undo.push(label, edit);
    Ok(PushPulled {
        cap: final_cap,
        sides,
        base,
    })
}
