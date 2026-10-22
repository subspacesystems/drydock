// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Transform a selection: the kernel side of Move, Rotate, Scale and Flip.
//!
//! Behaviour: `docs/specs/m3-transform.md`.

use std::collections::BTreeSet;

use glam::{DMat4, DVec3};

use crate::model::{BodyId, FaceMaterials, InstanceId, Model, ModelEdit, ModelError, UndoStack};
use crate::select::Entity;
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
