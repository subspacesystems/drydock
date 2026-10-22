// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sticky Move: a transform whose result heals and splits against the rest of the body.
//!
//! Behaviour: `docs/specs/m3-move-merge.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat4, DVec2, DVec3};

use crate::geom::{self, LINEAR_TOL, SegmentHit};
use crate::model::{BodyId, FaceMaterials, InstanceId, Model, ModelEdit, ModelError, UndoStack};
use crate::select::Entity;
use crate::sticky::StickyError;
use crate::topology::{Context, EdgeId, EdgeUse, Face, FaceId, Loop, TopoError, VertexId};
use crate::transform::{TransformError, leaves_geometry_in_place, moved_vertices};

fn bad_matrix(matrix: &DMat4) -> bool {
    if matrix.to_cols_array().iter().any(|v| !v.is_finite()) {
        return true;
    }
    if matrix.x_axis.w != 0.0
        || matrix.y_axis.w != 0.0
        || matrix.z_axis.w != 0.0
        || matrix.w_axis.w != 1.0
    {
        return true;
    }
    matrix.determinant().abs() < 1e-9
}

fn has_entity(model: &Model, body: BodyId, entity: Entity) -> bool {
    let Some(target) = model.body(body) else {
        return false;
    };
    match entity {
        Entity::Edge(id) => target.geometry().edge(id).is_some(),
        Entity::Face(id) => target.geometry().face(id).is_some(),
        Entity::Instance(id) => target.instance(id).is_some(),
    }
}

fn reversed_loop(l: &Loop) -> Loop {
    Loop {
        uses: l
            .uses
            .iter()
            .rev()
            .map(|u| EdgeUse {
                edge: u.edge,
                reversed: !u.reversed,
            })
            .collect(),
    }
}

fn reversed_face(face: &Face) -> Face {
    Face {
        outer: reversed_loop(&face.outer),
        inner: face.inner.iter().rev().map(reversed_loop).collect(),
    }
}

fn rollback(model: &mut Model, edit: &ModelEdit) {
    let _ = model.apply_edit(&edit.inverse());
}

fn face_uses_any(face: &Face, edges: &BTreeSet<EdgeId>) -> bool {
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| l.uses.iter())
        .any(|u| edges.contains(&u.edge))
}

fn face_vertices(ctx: &Context, face: &Face) -> Vec<VertexId> {
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| ctx.loop_vertices(l))
        .collect()
}

fn edge_positions(ctx: &Context, edge: EdgeId) -> Option<[DVec3; 2]> {
    let e = ctx.edge(edge)?;
    Some([
        ctx.vertex(e.ends[0])?.position,
        ctx.vertex(e.ends[1])?.position,
    ])
}

/// The intersection of two edges that is not just their shared endpoint.
fn new_contact_between(ctx: &Context, a: EdgeId, b: EdgeId) -> bool {
    let (Some(ea), Some(eb)) = (ctx.edge(a), ctx.edge(b)) else {
        return false;
    };
    let (Some(pa), Some(pb)) = (edge_positions(ctx, a), edge_positions(ctx, b)) else {
        return false;
    };
    let shared: Vec<VertexId> = ea
        .ends
        .iter()
        .copied()
        .filter(|v| eb.ends.contains(v))
        .collect();
    let hit = geom::segment_intersection(pa[0], pa[1], pb[0], pb[1], LINEAR_TOL);
    match shared.as_slice() {
        [] => !matches!(hit, SegmentHit::None),
        [only] => {
            let expected_a = if ea.ends[0] == *only { 0.0 } else { 1.0 };
            let expected_b = if eb.ends[0] == *only { 0.0 } else { 1.0 };
            match hit {
                SegmentHit::None => false,
                SegmentHit::Point { ta, tb } => !(ta == expected_a && tb == expected_b),
                SegmentHit::Overlap { .. } => true,
            }
        }
        _ => false,
    }
}

/// True when the moved geometry touches the rest of the body in a way a plain transform
/// would leave unhealed.
fn has_new_contact(ctx: &Context, moved: &BTreeSet<VertexId>) -> bool {
    if moved.is_empty() {
        return false;
    }

    let moved_positions: Vec<DVec3> = moved
        .iter()
        .filter_map(|v| ctx.vertex(*v).map(|v| v.position))
        .collect();
    let unmoved_vertices: Vec<DVec3> = ctx
        .vertices()
        .filter(|(id, _)| !moved.contains(id))
        .map(|(_, v)| v.position)
        .collect();

    for p in &moved_positions {
        if unmoved_vertices
            .iter()
            .any(|q| p.distance(*q) <= LINEAR_TOL)
        {
            return true;
        }
    }

    let mut dirty = Vec::new();
    let mut clean = Vec::new();
    for (id, edge) in ctx.edges() {
        if edge.ends.iter().any(|v| moved.contains(v)) {
            dirty.push(id);
        } else {
            clean.push(id);
        }
    }

    for &e in &dirty {
        for &u in &clean {
            if new_contact_between(ctx, e, u) {
                return true;
            }
        }
        let Some(edge) = ctx.edge(e) else {
            continue;
        };
        let moved_ends = edge
            .ends
            .iter()
            .copied()
            .filter(|v| moved.contains(v))
            .filter_map(|v| ctx.vertex(v).map(|v| v.position));
        for &f in &clean {
            let Some([c, d]) = edge_positions(ctx, f) else {
                continue;
            };
            // A moved vertex lying on an unmoved edge.
            if moved_ends
                .clone()
                .any(|p| geom::point_segment_distance(p, c, d) <= LINEAR_TOL)
            {
                return true;
            }
        }
    }

    for (a, b) in ctx.face_overlaps() {
        let (Some(fa), Some(fb)) = (ctx.face(a), ctx.face(b)) else {
            continue;
        };
        let moved_a = face_vertices(ctx, fa).iter().any(|v| moved.contains(v));
        let moved_b = face_vertices(ctx, fb).iter().any(|v| moved.contains(v));
        if moved_a != moved_b {
            return true;
        }
    }

    false
}

#[derive(Clone)]
struct OldFace {
    materials: FaceMaterials,
    moved: bool,
    outer: Vec<DVec3>,
    inner: Vec<Vec<DVec3>>,
}

impl OldFace {
    fn contains(&self, p: DVec3) -> bool {
        let Some(plane) = geom::fit_plane(&self.outer) else {
            return false;
        };
        let n = plane.normal;
        let q = geom::to_plane_2d(&[p], n)[0];
        let outer = geom::to_plane_2d(&self.outer, n);
        if geom::point_in_polygon_2d(q, &outer, LINEAR_TOL) != geom::Containment::Inside {
            return false;
        }
        self.inner.iter().all(|hole| {
            let hole = geom::to_plane_2d(hole, n);
            geom::point_in_polygon_2d(q, &hole, LINEAR_TOL) == geom::Containment::Outside
        })
    }
}

fn centroid_2d(points: &[DVec2]) -> Option<DVec2> {
    let mut area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let cross = a.x * b.y - b.x * a.y;
        area += cross;
        cx += (a.x + b.x) * cross;
        cy += (a.y + b.y) * cross;
    }
    if area.abs() < 1e-15 {
        if points.is_empty() {
            return None;
        }
        let sum = points.iter().copied().sum::<DVec2>();
        return Some(sum / points.len() as f64);
    }
    Some(DVec2::new(cx, cy) / (3.0 * area))
}

/// A representative point strictly inside the face, when one is easy to find.
fn interior_point(ctx: &Context, f: FaceId) -> Option<DVec3> {
    let face = ctx.face(f)?;
    let plane = ctx.face_plane(f)?;
    let n = plane.normal;
    let outer = ctx.loop_points(&face.outer);
    if outer.len() < 3 {
        return None;
    }
    let outer_2d = geom::to_plane_2d(&outer, n);
    let (u, v) = geom::plane_basis(n);
    let to_3d = |p: DVec2| u * p.x + v * p.y + n * plane.point.dot(n);

    if let Some(c) = centroid_2d(&outer_2d)
        && geom::point_in_polygon_2d(c, &outer_2d, LINEAR_TOL) == geom::Containment::Inside
        && face.inner.iter().all(|hole| {
            let hole = geom::to_plane_2d(&ctx.loop_points(hole), n);
            geom::point_in_polygon_2d(c, &hole, LINEAR_TOL) == geom::Containment::Outside
        })
    {
        return Some(to_3d(c));
    }

    let first = face.outer.uses.first()?;
    let from = ctx.use_start(*first)?;
    let edge = ctx.edge(first.edge)?;
    let to = if edge.ends[0] == from {
        edge.ends[1]
    } else {
        edge.ends[0]
    };
    let a = ctx.vertex(from)?.position;
    let b = ctx.vertex(to)?.position;
    let dir = b - a;
    if dir.length() <= LINEAR_TOL {
        return None;
    }
    let inward = n.cross(dir).normalize();
    Some((a + b) * 0.5 + inward * (LINEAR_TOL * 10.0))
}

fn restick(
    model: &mut Model,
    body: BodyId,
    moved: &BTreeSet<VertexId>,
    edit: &mut ModelEdit,
) -> Result<(), ModelError> {
    let Some(target) = model.body(body) else {
        return Err(ModelError::MissingBody(body));
    };
    let ctx = target.geometry();

    let dirty: Vec<(EdgeId, [DVec3; 2])> = ctx
        .edges()
        .filter(|(_, edge)| edge.ends.iter().any(|v| moved.contains(v)))
        .filter_map(|(id, _)| edge_positions(ctx, id).map(|p| (id, p)))
        .collect();
    let dirty_ids: BTreeSet<EdgeId> = dirty.iter().map(|(id, _)| *id).collect();

    let removed: Vec<(FaceId, Face, bool)> = ctx
        .faces()
        .filter(|(_, face)| face_uses_any(face, &dirty_ids))
        .map(|(id, face)| {
            let verts = face_vertices(ctx, face);
            let all_moved = !verts.is_empty() && verts.iter().all(|v| moved.contains(v));
            (id, face.clone(), all_moved)
        })
        .collect();

    let old_faces: Vec<OldFace> = removed
        .iter()
        .map(|(id, face, all_moved)| OldFace {
            materials: target.face_materials(*id),
            moved: *all_moved,
            outer: ctx.loop_points(&face.outer),
            inner: face.inner.iter().map(|l| ctx.loop_points(l)).collect(),
        })
        .collect();

    let moved_ids: Vec<VertexId> = moved
        .iter()
        .copied()
        .filter(|v| ctx.vertex(*v).is_some())
        .collect();

    let rebuilt = model.edit_geometry(body, edit, |ctx, log| {
        for (id, _, _) in &removed {
            ctx.delete_face(*id, log)?;
        }
        for (id, _) in &dirty {
            ctx.delete_edge(*id, log)?;
        }
        for v in &moved_ids {
            if ctx.vertex(*v).is_some() {
                ctx.delete_vertex(*v, log)?;
            }
        }

        let mut added: Vec<(FaceId, Option<FaceId>)> = Vec::new();
        for (_, [a, b]) in &dirty {
            match ctx.add_edge(*a, *b, log) {
                Ok(chain) => added.extend(chain.faces),
                Err(StickyError::Degenerate) => {}
                Err(StickyError::NotFinite) => return Err(TopoError::Mismatch),
                Err(StickyError::MissingEdge(e)) => {
                    return Err(TopoError::MissingEdge(e));
                }
            }
        }

        let stray: Vec<VertexId> = ctx
            .vertices()
            .filter(|(id, _)| ctx.edges_of_vertex(*id).is_empty())
            .map(|(id, _)| id)
            .collect();
        for v in stray {
            ctx.delete_vertex(v, log)?;
        }
        Ok::<_, TopoError>(added)
    });

    let added = match rebuilt {
        Ok(Ok(added)) => added,
        Ok(Err(error)) => return Err(ModelError::Topo(error)),
        Err(error) => return Err(error),
    };

    let origins: BTreeMap<FaceId, Option<FaceId>> = added.into_iter().collect();
    let samples: Vec<(FaceId, DVec3)> = {
        let Some(target) = model.body(body) else {
            return Err(ModelError::MissingBody(body));
        };
        let ctx = target.geometry();
        ctx.faces()
            .filter_map(|(id, _)| interior_point(ctx, id).map(|p| (id, p)))
            .collect()
    };

    for (face_id, sample) in samples {
        let moved_material = old_faces
            .iter()
            .filter(|old| old.moved && old.materials != FaceMaterials::default())
            .find(|old| old.contains(sample))
            .map(|old| old.materials);
        let origin_material = origins
            .get(&face_id)
            .and_then(|origin| *origin)
            .map(|old| {
                model
                    .body(body)
                    .map(|target| target.face_materials(old))
                    .unwrap_or_default()
            })
            .filter(|materials| *materials != FaceMaterials::default());
        let old_material = old_faces
            .iter()
            .filter(|old| !old.moved && old.materials != FaceMaterials::default())
            .find(|old| old.contains(sample))
            .map(|old| old.materials);

        let materials = moved_material.or(origin_material).or(old_material);
        if let Some(materials) = materials {
            let current = model
                .body(body)
                .map(|target| target.face_materials(face_id))
                .unwrap_or_default();
            if current != materials {
                model.set_face_materials(body, face_id, materials, edit)?;
            }
        }
    }

    Ok(())
}

/// Like `transform::transform`, then welds, splits and re-forms faces where the moved
/// geometry meets the rest of the body.
pub fn transform_sticky(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<(), TransformError> {
    if model.body(body).is_some()
        && !selection.is_empty()
        && leaves_geometry_in_place(model, body, selection, matrix)
    {
        return Ok(());
    }
    match transform_attached(model, body, selection, matrix, undo, label) {
        Err(error @ TransformError::Invalid(_)) => {
            // Geometry glued to the rest by a vertex or two can't carry that vertex along
            // without bending its neighbours: pull the selection loose instead.
            match transform_detached(model, body, selection, matrix, undo, label) {
                Ok(true) => Ok(()),
                _ => Err(error),
            }
        }
        other => other,
    }
}

/// Moves the selection and leaves everything not selected where it is, even where they
/// share vertices. Only for selections whose faces are all selected with them. `Ok(false)`:
/// not applicable (nothing was changed).
fn transform_detached(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<bool, TransformError> {
    let Some(target) = model.body(body) else {
        return Err(TransformError::MissingBody(body));
    };
    let ctx = target.geometry();
    if selection.iter().any(|e| matches!(e, Entity::Instance(_))) {
        return Ok(false);
    }
    let mut edges: BTreeSet<EdgeId> = BTreeSet::new();
    let mut faces: BTreeSet<FaceId> = BTreeSet::new();
    for entity in selection {
        match *entity {
            Entity::Edge(id) => {
                edges.insert(id);
            }
            Entity::Face(id) => {
                let Some(face) = ctx.face(id) else {
                    return Ok(false);
                };
                faces.insert(id);
                edges.extend(
                    std::iter::once(&face.outer)
                        .chain(face.inner.iter())
                        .flat_map(|l| l.uses.iter())
                        .map(|u| u.edge),
                );
            }
            Entity::Instance(_) => {}
        }
    }
    let moved = moved_vertices(ctx, selection);
    let shared = ctx
        .edges()
        .any(|(id, e)| !edges.contains(&id) && e.ends.iter().any(|v| moved.contains(v)));
    // Every face on a selected edge must move with it.
    let loose_face = edges
        .iter()
        .flat_map(|e| ctx.faces_of_edge(*e))
        .any(|f| !faces.contains(&f));
    if !shared || loose_face {
        return Ok(false);
    }
    let before = ctx.validate();

    let new_edges: Vec<(EdgeId, [DVec3; 2])> = edges
        .iter()
        .filter_map(|&e| {
            edge_positions(ctx, e)
                .map(|[a, b]| (e, [matrix.transform_point3(a), matrix.transform_point3(b)]))
        })
        .collect();
    let old_faces: Vec<OldFace> = faces
        .iter()
        .filter_map(|&f| {
            let face = ctx.face(f)?;
            let place = |points: Vec<DVec3>| -> Vec<DVec3> {
                points
                    .into_iter()
                    .map(|p| matrix.transform_point3(p))
                    .collect()
            };
            Some(OldFace {
                materials: target.face_materials(f),
                moved: true,
                outer: place(ctx.loop_points(&face.outer)),
                inner: face
                    .inner
                    .iter()
                    .map(|l| place(ctx.loop_points(l)))
                    .collect(),
            })
        })
        .collect();
    let ends: BTreeSet<VertexId> = moved.clone();

    let mut edit = ModelEdit::new();
    let rebuilt = model.edit_geometry(body, &mut edit, |ctx, log| {
        for f in &faces {
            ctx.delete_face(*f, log)?;
        }
        for e in &edges {
            ctx.delete_edge(*e, log)?;
        }
        for v in &ends {
            if ctx.vertex(*v).is_some() && ctx.edges_of_vertex(*v).is_empty() {
                ctx.delete_vertex(*v, log)?;
            }
        }
        for (_, [a, b]) in &new_edges {
            match ctx.add_edge(*a, *b, log) {
                Ok(_) | Err(StickyError::Degenerate) => {}
                Err(StickyError::NotFinite) => return Err(TopoError::Mismatch),
                Err(StickyError::MissingEdge(e)) => return Err(TopoError::MissingEdge(e)),
            }
        }
        Ok::<_, TopoError>(())
    });
    match rebuilt {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            rollback(model, &edit);
            return Err(TransformError::Model(ModelError::Topo(error)));
        }
        Err(error) => {
            rollback(model, &edit);
            return Err(TransformError::Model(error));
        }
    }

    let samples: Vec<(FaceId, DVec3)> = {
        let ctx = model.body(body).map(|t| t.geometry());
        let Some(ctx) = ctx else {
            rollback(model, &edit);
            return Err(TransformError::MissingBody(body));
        };
        ctx.faces()
            .filter_map(|(id, _)| interior_point(ctx, id).map(|p| (id, p)))
            .collect()
    };
    for (face_id, sample) in samples {
        let materials = old_faces
            .iter()
            .filter(|old| old.materials != FaceMaterials::default())
            .find(|old| old.contains(sample))
            .map(|old| old.materials);
        if let Some(materials) = materials
            && let Err(error) = model.set_face_materials(body, face_id, materials, &mut edit)
        {
            rollback(model, &edit);
            return Err(TransformError::Model(error));
        }
    }

    let invalid = model.body(body).and_then(|t| {
        t.geometry()
            .validate()
            .into_iter()
            .find(|v| !before.contains(v))
    });
    if let Some(violation) = invalid {
        rollback(model, &edit);
        return Err(TransformError::Invalid(violation));
    }
    undo.push(label, edit);
    Ok(true)
}

fn transform_attached(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<(), TransformError> {
    if model.body(body).is_none() {
        return Err(TransformError::MissingBody(body));
    }
    if selection.is_empty() {
        return Err(TransformError::Empty);
    }
    if bad_matrix(&matrix) {
        return Err(TransformError::BadMatrix);
    }
    for &entity in selection {
        if !has_entity(model, body, entity) {
            return Err(TransformError::MissingEntity(entity));
        }
    }
    if matrix == DMat4::IDENTITY {
        return Ok(());
    }

    let geometry_selected = selection.iter().any(|e| !matches!(e, Entity::Instance(_)));
    let negative = matrix.determinant() < 0.0;

    let (moved, before) = {
        let Some(target) = model.body(body) else {
            return Err(TransformError::MissingBody(body));
        };
        let moved = moved_vertices(target.geometry(), selection);
        let before = if geometry_selected {
            target.geometry().validate()
        } else {
            Vec::new()
        };
        (moved, before)
    };

    let mut edit = ModelEdit::new();

    let moves: Vec<(VertexId, DVec3)> = {
        let Some(target) = model.body(body) else {
            return Err(TransformError::MissingBody(body));
        };
        let ctx = target.geometry();
        moved
            .iter()
            .filter_map(|&v| {
                ctx.vertex(v)
                    .map(|vertex| (v, matrix.transform_point3(vertex.position)))
            })
            .collect()
    };
    if !moves.is_empty() {
        let result = model.edit_geometry(body, &mut edit, |ctx, log| {
            for (vertex, position) in &moves {
                ctx.set_vertex_position(*vertex, *position, log)?;
            }
            Ok::<(), TopoError>(())
        });
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                rollback(model, &edit);
                return Err(TransformError::Model(ModelError::Topo(error)));
            }
            Err(error) => {
                rollback(model, &edit);
                return Err(TransformError::Model(error));
            }
        }
    }

    let instances: BTreeSet<InstanceId> = selection
        .iter()
        .filter_map(|e| match e {
            Entity::Instance(id) => Some(*id),
            _ => None,
        })
        .collect();
    for id in instances {
        let Some(mut instance) = model
            .body(body)
            .and_then(|target| target.instance(id))
            .cloned()
        else {
            rollback(model, &edit);
            return Err(TransformError::Model(ModelError::MissingInstance(body, id)));
        };
        instance.transform = matrix * instance.transform;
        if let Err(error) = model.set_instance(body, id, instance, &mut edit) {
            rollback(model, &edit);
            return Err(TransformError::Model(error));
        }
    }

    let flips: Vec<(FaceId, Face)> = if negative {
        let Some(target) = model.body(body) else {
            return Err(TransformError::MissingBody(body));
        };
        let ctx = target.geometry();
        ctx.faces()
            .filter_map(|(id, face)| {
                let mut count = 0;
                let mut all_moved = true;
                for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                    for v in ctx.loop_vertices(l) {
                        count += 1;
                        if !moved.contains(&v) {
                            all_moved = false;
                        }
                    }
                }
                (count > 0 && all_moved).then(|| (id, reversed_face(face)))
            })
            .collect()
    } else {
        Vec::new()
    };

    for (old_id, face) in flips {
        let Some(materials) = model.body(body).map(|target| target.face_materials(old_id)) else {
            rollback(model, &edit);
            return Err(TransformError::Model(ModelError::MissingBody(body)));
        };
        let removed = model.edit_geometry(body, &mut edit, |ctx, log| ctx.delete_face(old_id, log));
        match removed {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                rollback(model, &edit);
                return Err(TransformError::Model(ModelError::Topo(error)));
            }
            Err(error) => {
                rollback(model, &edit);
                return Err(TransformError::Model(error));
            }
        }
        let inserted = model.edit_geometry(body, &mut edit, |ctx, log| ctx.insert_face(face, log));
        let new_id = match inserted {
            Ok(Ok(id)) => id,
            Ok(Err(error)) => {
                rollback(model, &edit);
                return Err(TransformError::Model(ModelError::Topo(error)));
            }
            Err(error) => {
                rollback(model, &edit);
                return Err(TransformError::Model(error));
            }
        };
        if materials != FaceMaterials::default()
            && let Err(error) = model.set_face_materials(body, new_id, materials, &mut edit)
        {
            rollback(model, &edit);
            return Err(TransformError::Model(error));
        }
    }

    if geometry_selected {
        let contact = model
            .body(body)
            .map(|target| has_new_contact(target.geometry(), &moved))
            .unwrap_or(false);
        if contact && let Err(error) = restick(model, body, &moved, &mut edit) {
            rollback(model, &edit);
            return Err(TransformError::Model(error));
        }

        let invalid = {
            let Some(target) = model.body(body) else {
                rollback(model, &edit);
                return Err(TransformError::Model(ModelError::MissingBody(body)));
            };
            target
                .geometry()
                .validate()
                .into_iter()
                .find(|v| !before.contains(v))
        };
        if let Some(violation) = invalid {
            rollback(model, &edit);
            return Err(TransformError::Invalid(violation));
        }
    }

    undo.push(label, edit);
    Ok(())
}
