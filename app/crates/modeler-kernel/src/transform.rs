// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Transform a selection: the kernel side of Move, Rotate, Scale and Flip.
//!
//! Behaviour: `docs/specs/m3-transform.md`.

use std::collections::BTreeSet;

use glam::DMat4;

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::select::Entity;
use crate::topology::{Context, VertexId, Violation};

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
    let _ = (ctx, selection);
    unimplemented!("m3-transform")
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
    let _ = (model, body, selection, matrix, undo, label);
    unimplemented!("m3-transform")
}
