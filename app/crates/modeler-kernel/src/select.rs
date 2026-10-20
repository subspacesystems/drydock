// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Selection logic: window and crossing boxes, and the connected sets that double and triple
//! clicks select. Pure functions over the model; the app keeps the selection.
//!
//! Behaviour: `docs/specs/m2-select-box.md`.

use crate::model::{BodyId, InstanceId, Model};
use crate::topology::{Context, EdgeId, FaceId};
use glam::{DVec2, DVec3};

/// One selectable thing in a body. Ordered edges first, then faces, then instances, each by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Entity {
    Edge(EdgeId),
    Face(FaceId),
    Instance(InstanceId),
}

/// How a selection box picks entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxMode {
    /// Entities entirely inside the box.
    Window,
    /// Entities inside or touching the box.
    Crossing,
}

impl BoxMode {
    /// The mode of a box dragged from `start` to `end` (screen pixels).
    pub fn from_drag(start: DVec2, end: DVec2) -> BoxMode {
        let _ = (start, end);
        unimplemented!("m2-select-box")
    }
}

/// The entities of `body` that the screen box with opposite corners `a` and `b` selects.
/// `project` maps a point in `body` coordinates to screen pixels, `None` when it can't be
/// seen. Sorted, no duplicates.
pub fn box_select(
    model: &Model,
    body: BodyId,
    project: &dyn Fn(DVec3) -> Option<DVec2>,
    a: DVec2,
    b: DVec2,
    mode: BoxMode,
) -> Vec<Entity> {
    let _ = (model, body, project, a, b, mode);
    unimplemented!("m2-select-box")
}

/// What a double click on `entity` selects. Sorted, no duplicates.
pub fn bounded(ctx: &Context, entity: Entity) -> Vec<Entity> {
    let _ = (ctx, entity);
    unimplemented!("m2-select-box")
}

/// What a triple click on `entity` selects: everything connected to it. Sorted, no duplicates.
pub fn connected(ctx: &Context, entity: Entity) -> Vec<Entity> {
    let _ = (ctx, entity);
    unimplemented!("m2-select-box")
}
