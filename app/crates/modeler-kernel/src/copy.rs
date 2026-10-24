// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Copy: take a snapshot of a selection (the clipboard) and put copies of it into a body,
//! stuck to the geometry there, as one undo step. Used by Move with copy and by
//! Cut / Copy / Paste.
//!
//! Behaviour: `docs/specs/m4-copy.md`.

use glam::{DMat4, DVec3};

use crate::model::{BodyId, Model, ModelError, ModelViolation, UndoStack};
use crate::select::Entity;

/// Why a copy, extract or insert was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum CopyError {
    /// Nothing is selected, or the clipboard holds nothing.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in selection order).
    Missing(Entity),
    /// The matrix is not finite, not affine, or (nearly) singular.
    BadMatrix,
    /// The model refused a record (also: the body doesn't exist, or a cycle of instances).
    Model(ModelError),
    /// The result would add these `Model::validate` violations.
    Invalid(Vec<ModelViolation>),
}

/// A self-contained snapshot of a selection: its geometry, face materials and instances in
/// the source body's coordinates, with every definition and material they reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Clipboard {
    model: Model,
}

impl Clipboard {
    /// The snapshot as a model: its root holds the copied selection; its definitions and
    /// materials are exact copies of the source's, under the source's ids.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Whether the snapshot has no edge, face or instance.
    pub fn is_empty(&self) -> bool {
        unimplemented!()
    }

    /// The smallest axis-aligned box (min, max) around every copied vertex and every vertex
    /// the copied instances show, in the source body's coordinates; `None` if there are none.
    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        unimplemented!()
    }
}

/// Snapshot `selection` of `body`. The model is not changed.
pub fn extract(model: &Model, body: BodyId, selection: &[Entity]) -> Result<Clipboard, CopyError> {
    let _ = (model, body, selection);
    unimplemented!()
}

/// Put a copy of `clipboard` into `body`, each point at `matrix · p`, as one undo step
/// labelled `label`. Returns the entities of `body` that came from it, sorted.
pub fn insert(
    model: &mut Model,
    body: BodyId,
    clipboard: &Clipboard,
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<Entity>, CopyError> {
    let _ = (model, body, clipboard, matrix, undo, label);
    unimplemented!()
}

/// `extract` then `insert` into the same body: copies `selection` of `body` to `matrix`
/// times where it is, as one undo step. Returns the copy's entities, sorted.
pub fn copy_selection(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<Entity>, CopyError> {
    let _ = (model, body, selection, matrix, undo, label);
    unimplemented!()
}
