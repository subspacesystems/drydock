// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Paint: put a material on one side of faces and on instances; add, edit and purge
//! materials, each as one undo step.
//!
//! Behaviour: `docs/specs/m4-paint.md`.

use crate::model::{BodyId, Material, MaterialId, Model, ModelError, UndoStack};
use crate::select::Entity;

/// The side of a face that paint goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
}

/// Why a paint or material operation was refused. The model and the undo stack are
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaintError {
    /// Nothing to paint.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in target order).
    Missing(Entity),
    /// A color channel is not finite or outside 0..=1.
    BadColor,
    /// The model refused a record (also: missing body or material).
    Model(ModelError),
}

/// Paint `material` (`None` = the default material) on `side` of every face in `targets`
/// and on every instance in `targets`; edges are skipped. Returns how many entities
/// changed; pushes one undo step named `label` when that is more than zero.
pub fn paint(
    model: &mut Model,
    body: BodyId,
    targets: &[Entity],
    side: Side,
    material: Option<MaterialId>,
    undo: &mut UndoStack,
    label: &str,
) -> Result<usize, PaintError> {
    let _ = (model, body, targets, side, material, undo, label);
    unimplemented!()
}

/// The material on `side` of a face, or on an instance (`side` ignored). Outer `None`: the
/// entity is an edge or isn't in the body.
pub fn sample(
    model: &Model,
    body: BodyId,
    entity: Entity,
    side: Side,
) -> Option<Option<MaterialId>> {
    let _ = (model, body, entity, side);
    unimplemented!()
}

/// `base` if no material has that name, else `base 2`, `base 3`, ... (the first free one).
/// An empty or blank `base` counts as `Material`.
pub fn unique_material_name(model: &Model, base: &str) -> String {
    let _ = (model, base);
    unimplemented!()
}

/// Add `material` under a unique name (see [`unique_material_name`]) as one undo step.
pub fn create_material(
    model: &mut Model,
    material: Material,
    undo: &mut UndoStack,
    label: &str,
) -> Result<MaterialId, PaintError> {
    let _ = (model, material, undo, label);
    unimplemented!()
}

/// Replace material `id` with `material` as one undo step (none when nothing changes). A
/// name used by another material is made unique the same way.
pub fn edit_material(
    model: &mut Model,
    id: MaterialId,
    material: Material,
    undo: &mut UndoStack,
    label: &str,
) -> Result<(), PaintError> {
    let _ = (model, id, material, undo, label);
    unimplemented!()
}

/// Remove every material that no existing face side and no instance uses, as one undo
/// step (none when nothing goes). Returns the removed ids, ascending.
pub fn purge_materials(
    model: &mut Model,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<MaterialId>, PaintError> {
    let _ = (model, undo, label);
    unimplemented!()
}
