// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Transform a selection: the kernel side of Move, Rotate, Scale and Flip.
//!
//! Behaviour: `docs/specs/m3-transform.md`.

use std::collections::BTreeSet;

use glam::{DMat4, DVec3};

use crate::geom;
use crate::model::{BodyId, FaceMaterials, InstanceId, Model, ModelEdit, ModelError, UndoStack};
use crate::select::Entity;
use crate::texture::moved_maps;
use crate::topology::{Context, EdgeUse, Face, FaceId, Loop, TopoError, VertexId, Violation};

/// Why a transform was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformError {
    /// Nothing is selected.
    Empty,
    /// The matrix is not finite, not affine, or (nearly) singular.
    BadMatrix,
    MissingBody(BodyId),
    /// The selection names an edge, face or instance the body doesn't have.
    MissingEntity(Entity),
    /// The result would break an invariant that held before.
    Invalid(Violation),
    Model(ModelError),
}

/// Every vertex `transform` would move for `selection`: both ends of each edge, every loop
/// vertex of each face. Instances move no vertices. Unknown entities are skipped.
pub fn moved_vertices(ctx: &Context, selection: &[Entity]) -> BTreeSet<VertexId> {
    let mut out = BTreeSet::new();
    for entity in selection {
        match *entity {
            Entity::Edge(id) => {
                if let Some(edge) = ctx.edge(id) {
                    out.extend(edge.ends);
                }
            }
            Entity::Face(id) => {
                if let Some(face) = ctx.face(id) {
                    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                        out.extend(ctx.loop_vertices(l));
                    }
                }
            }
            Entity::Instance(_) => {}
        }
    }
    out
}

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

/// The materials to store on each face whose every vertex moves, after carrying its maps by
/// `matrix`. Faces that move only in part are left out.
pub(crate) fn moved_face_materials(
    model: &Model,
    body: BodyId,
    moved: &BTreeSet<VertexId>,
    matrix: DMat4,
) -> Vec<(FaceId, FaceMaterials)> {
    let Some(target) = model.body(body) else {
        return Vec::new();
    };
    let ctx = target.geometry();
    ctx.faces()
        .filter_map(|(id, face)| {
            let mut vertices = Vec::new();
            for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                vertices.extend(ctx.loop_vertices(l));
            }
            if vertices.is_empty() || !vertices.iter().all(|v| moved.contains(v)) {
                return None;
            }
            let normal = geom::polygon_normal(&ctx.loop_points(&face.outer));
            let old = target.face_materials(id);
            let new = moved_maps(model, old, normal, matrix);
            (new != old).then_some((id, new))
        })
        .collect()
}

/// True when `matrix` leaves every vertex of the selection where it is (a rotation of an
/// edge about its own axis, say). Such a transform changes nothing and is not an error.
pub(crate) fn leaves_geometry_in_place(
    model: &Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
) -> bool {
    let Some(target) = model.body(body) else {
        return false;
    };
    if selection.iter().any(|e| matches!(e, Entity::Instance(_))) {
        return false;
    }
    let ctx = target.geometry();
    let moved = moved_vertices(ctx, selection);
    !moved.is_empty()
        && moved.iter().all(|&v| {
            ctx.vertex(v).is_some_and(|vertex| {
                matrix
                    .transform_point3(vertex.position)
                    .distance(vertex.position)
                    <= 1e-9
            })
        })
}

/// Apply `matrix` to `selection` in `body` as one undo step labelled `label`.
pub fn transform(
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
    if matrix == DMat4::IDENTITY || leaves_geometry_in_place(model, body, selection, matrix) {
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
    let face_changes = moved_face_materials(model, body, &moved, matrix);

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

    for (face, materials) in face_changes {
        if let Err(error) = model.set_face_materials(body, face, materials, &mut edit) {
            rollback(model, &edit);
            return Err(TransformError::Model(error));
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
