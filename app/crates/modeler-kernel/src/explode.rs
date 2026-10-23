// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Explode: put an instance's contents into the body that holds it, stuck to the geometry
//! there, and remove the instance, as one undo step.
//!
//! Behaviour: `docs/specs/m4-explode.md`.

use crate::model::{BodyId, InstanceId, Model, ModelError, ModelViolation, UndoStack};
use crate::select::Entity;

/// Why an explode was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum ExplodeError {
    /// The body has no such instance.
    MissingInstance(InstanceId),
    /// The instance's transform has a non-finite entry or isn't invertible.
    BadTransform,
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
    /// The result would add these `Model::validate` violations.
    Invalid(Vec<ModelViolation>),
}

/// Explode `instance` of `body`: its definition's geometry, face materials and nested
/// instances go into `body` where the instance showed them. Returns the entities of `body`
/// that came from it, sorted.
pub fn explode(
    model: &mut Model,
    body: BodyId,
    instance: InstanceId,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<Entity>, ExplodeError> {
    let _ = (model, body, instance, undo, label);
    unimplemented!()
}
