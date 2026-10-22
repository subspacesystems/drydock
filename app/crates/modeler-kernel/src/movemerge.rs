// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sticky Move: a transform whose result heals and splits against the rest of the body.
//!
//! Behaviour: `docs/specs/m3-move-merge.md`.

use glam::DMat4;

use crate::model::{BodyId, Model, UndoStack};
use crate::select::Entity;
use crate::transform::TransformError;

/// Like `transform::transform`, then welds, splits and re-forms faces where the moved
/// geometry meets the rest of the body.
pub fn transform_sticky(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<(), TransformError> {
    let _ = (model, body, selection, matrix, undo, label);
    unimplemented!("m3-move-merge")
}
