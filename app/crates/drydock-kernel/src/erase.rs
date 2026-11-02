// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Erase: delete the selected edges, faces and instances of a body in one undo step.
//!
//! Behaviour: `docs/specs/m3-erase.md`.

use std::collections::BTreeSet;

use crate::model::{BodyId, InstanceId, Model, ModelEdit, ModelError, UndoStack};
use crate::select::Entity;
use crate::sticky::StickyError;
use crate::topology::{EdgeId, FaceId};

/// Why erasing was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EraseError {
    /// Nothing is selected.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in selection order).
    Missing(Entity),
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
    /// The geometry refused to erase an edge.
    Sticky(StickyError),
}

/// Erase `selection` from `body` and push one undo step named `label`.
pub fn erase(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    undo: &mut UndoStack,
    label: &str,
) -> Result<(), EraseError> {
    if selection.is_empty() {
        return Err(EraseError::Empty);
    }
    let current = model
        .body(body)
        .ok_or(EraseError::Model(ModelError::MissingBody(body)))?;
    for &entity in selection {
        let present = match entity {
            Entity::Edge(e) => current.geometry().edge(e).is_some(),
            Entity::Face(f) => current.geometry().face(f).is_some(),
            Entity::Instance(i) => current.instance(i).is_some(),
        };
        if !present {
            return Err(EraseError::Missing(entity));
        }
    }

    let mut faces: BTreeSet<FaceId> = BTreeSet::new();
    let mut edges: BTreeSet<EdgeId> = BTreeSet::new();
    let mut instances: BTreeSet<InstanceId> = BTreeSet::new();
    for &entity in selection {
        match entity {
            Entity::Edge(e) => {
                edges.insert(e);
            }
            Entity::Face(f) => {
                faces.insert(f);
            }
            Entity::Instance(i) => {
                instances.insert(i);
            }
        }
    }

    let mut log = ModelEdit::new();
    let result = model.edit_geometry(body, &mut log, |ctx, edit| {
        for &f in &faces {
            if ctx.face(f).is_some() {
                ctx.delete_face(f, edit)
                    .map_err(|e| EraseError::Model(ModelError::Topo(e)))?;
            }
        }
        for &e in &edges {
            if ctx.edge(e).is_some() {
                ctx.erase_edge(e, edit).map_err(EraseError::Sticky)?;
            }
        }
        Ok::<(), EraseError>(())
    });
    let result = match result {
        Ok(inner) => inner,
        Err(error) => Err(EraseError::Model(error)),
    };
    if let Err(error) = result {
        let _ = model.apply_edit(&log.inverse());
        return Err(error);
    }

    for &instance in &instances {
        if let Err(error) = model.remove_instance(body, instance, &mut log) {
            let _ = model.apply_edit(&log.inverse());
            return Err(EraseError::Model(error));
        }
    }

    undo.push(label, log);
    Ok(())
}
