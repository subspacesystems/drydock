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
        let _ = model;
        unimplemented!()
    }

    /// Maps the open body's coordinates to world coordinates: the steps' transforms
    /// multiplied outermost first (identity for the root). `None` when `body` is `None`.
    pub fn transform(&self, model: &Model) -> Option<DMat4> {
        let _ = model;
        unimplemented!()
    }

    /// This path with `instance`, an instance of the open body, opened inside it. `None` when
    /// the path doesn't resolve or the open body has no such instance.
    pub fn open(&self, model: &Model, instance: InstanceId) -> Option<EditPath> {
        let _ = (model, instance);
        unimplemented!()
    }

    /// This path with the innermost instance closed. The root stays the root.
    pub fn close(&self) -> EditPath {
        unimplemented!()
    }

    /// The longest leading part of this path that still resolves in `model` (after an undo or
    /// an erase took an instance or definition away).
    pub fn valid_prefix(&self, model: &Model) -> EditPath {
        let _ = model;
        unimplemented!()
    }
}
