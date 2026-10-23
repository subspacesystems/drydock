// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Erase: delete the selected edges, faces and instances of a body in one undo step.
//!
//! Behaviour: `docs/specs/m3-erase.md`.

use std::collections::BTreeSet;

use crate::model::{BodyId, Model, ModelEdit, ModelError, UndoStack};
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
    let Some(current) = model.body(body) else {
        return Err(EraseError::Model(ModelError::MissingBody(body)));
    };
    for &entity in selection {
        let present = match entity {
            Entity::Edge(id) => current.geometry().edge(id).is_some(),
            Entity::Face(id) => current.geometry().face(id).is_some(),
            Entity::Instance(id) => current.instance(id).is_some(),
        };
        if !present {
            return Err(EraseError::Missing(entity));
        }
    }

    let faces: BTreeSet<FaceId> = selection
        .iter()
        .filter_map(|entity| match entity {
            Entity::Face(id) => Some(*id),
            _ => None,
        })
        .collect();
    let edges: BTreeSet<EdgeId> = selection
        .iter()
        .filter_map(|entity| match entity {
            Entity::Edge(id) => Some(*id),
            _ => None,
        })
        .collect();
    let instances: BTreeSet<_> = selection
        .iter()
        .filter_map(|entity| match entity {
            Entity::Instance(id) => Some(*id),
            _ => None,
        })
        .collect();

    let mut edit = ModelEdit::new();
    let geometry = model
        .edit_geometry(body, &mut edit, |ctx, log| {
            for face in faces {
                ctx.delete_face(face, log)
                    .map_err(|error| EraseError::Model(ModelError::Topo(error)))?;
            }
            for edge in edges {
                if ctx.edge(edge).is_some() {
                    ctx.erase_edge(edge, log).map_err(EraseError::Sticky)?;
                }
            }
            Ok(())
        })
        .map_err(EraseError::Model)?;
    if let Err(error) = geometry {
        let _ = model.apply_edit(&edit.inverse());
        return Err(error);
    }

    for instance in instances {
        if let Err(error) = model.remove_instance(body, instance, &mut edit) {
            let _ = model.apply_edit(&edit.inverse());
            return Err(EraseError::Model(error));
        }
    }

    undo.push(label, edit);
    Ok(())
}
