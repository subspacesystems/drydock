// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Erase: delete the selected edges, faces and instances of a body in one undo step.
//!
//! Behaviour: `docs/specs/m3-erase.md`.

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::select::Entity;
use crate::sticky::StickyError;

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
    let _ = (model, body, selection, undo, label);
    unimplemented!("m3-erase")
}
