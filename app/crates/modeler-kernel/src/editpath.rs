// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Editing inside an instance: the path of instances opened for editing, the body it opens
//! and the transform between that body's coordinates and the world.
//!
//! Behaviour: `docs/specs/m4-edit-path.md`. Picking and inference in the open body are
//! `infer::pick_in` and `infer::infer_in`.

use glam::DMat4;

use crate::model::{BodyId, InstanceId, Model};

/// The instances opened for editing, outermost first: `steps[0]` is an instance of the
/// root, and each later step an instance inside the definition of the step before it.
/// Empty: the root is open.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct EditPath {
    pub steps: Vec<InstanceId>,
}

impl EditPath {
    /// The empty path: the root is open.
    pub fn root() -> EditPath {
        EditPath::default()
    }

    pub fn is_root(&self) -> bool {
        self.steps.is_empty()
    }

    /// The open body: the root for the empty path, else the definition of the last step.
    /// `None` when a step isn't an instance of the body the steps before it open.
    pub fn body(&self, model: &Model) -> Option<BodyId> {
        self.resolve(model).map(|(body, _)| body)
    }

    /// Maps the open body's coordinates to world coordinates: the steps' transforms
    /// multiplied outermost first (identity for the root). `None` when `body` is `None`.
    pub fn transform(&self, model: &Model) -> Option<DMat4> {
        self.resolve(model).map(|(_, transform)| transform)
    }

    /// This path with `instance`, an instance of the open body, opened inside it. `None` when
    /// the path doesn't resolve or the open body has no such instance.
    pub fn open(&self, model: &Model, instance: InstanceId) -> Option<EditPath> {
        let mut next = self.clone();
        next.steps.push(instance);
        next.body(model)?;
        Some(next)
    }

    /// This path with the innermost instance closed. The root stays the root.
    pub fn close(&self) -> EditPath {
        let mut next = self.clone();
        next.steps.pop();
        next
    }

    /// The longest leading part of this path that still resolves in `model` (after an undo or
    /// an erase took an instance or definition away).
    pub fn valid_prefix(&self, model: &Model) -> EditPath {
        let mut body = BodyId::Root;
        let mut prefix = EditPath::root();
        for &step in &self.steps {
            let Some(instance) = model.body(body).and_then(|b| b.instance(step)) else {
                break;
            };
            let next = BodyId::Definition(instance.definition);
            if model.body(next).is_none() {
                break;
            }
            prefix.steps.push(step);
            body = next;
        }
        prefix
    }

    /// Walk the steps from the root, returning the open body and its world transform.
    fn resolve(&self, model: &Model) -> Option<(BodyId, DMat4)> {
        let mut body = BodyId::Root;
        let mut transform = DMat4::IDENTITY;
        for &step in &self.steps {
            let instance = model.body(body)?.instance(step)?;
            let next = BodyId::Definition(instance.definition);
            model.body(next)?;
            transform *= instance.transform;
            body = next;
        }
        Some((body, transform))
    }
}
