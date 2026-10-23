// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Make component / make group: move a selection of a body into a new definition and put
//! one instance of it in its place, as one undo step.
//!
//! Behaviour: `docs/specs/m4-make-component.md`.

use crate::model::{
    BodyId, DefinitionInfo, InstanceId, Model, ModelError, ModelViolation, UndoStack,
};
use crate::select::Entity;

/// Why making a component was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentError {
    /// Nothing is selected.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in selection order).
    Missing(Entity),
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
    /// The result would add these `Model::validate` violations.
    Invalid(Vec<ModelViolation>),
}

/// The definition name to use for `base`: trimmed; blank becomes `Group` or `Component`
/// (by `group`); then `"<name> <n>"`, smallest `n >= 2`, while a definition has that name.
pub fn unique_definition_name(model: &Model, base: &str, group: bool) -> String {
    let _ = (model, base, group);
    unimplemented!()
}

/// Move `selection` out of `body` into a new definition described by `info` (name made
/// unique) and place one instance of it where the selection was. Returns the instance.
pub fn make_component(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    info: DefinitionInfo,
    undo: &mut UndoStack,
    label: &str,
) -> Result<InstanceId, ComponentError> {
    let _ = (model, body, selection, info, undo, label);
    unimplemented!()
}
