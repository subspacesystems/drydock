// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid repair for 3D printing: remove internal, stray and loose geometry, fill flat holes
//! and turn reversed faces round, in one undo step.
//!
//! Behaviour: `docs/specs/m7-solid-repair.md`.

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::solid::SolidReport;
use crate::topology::{Context, EdgeId, Edit, FaceId};

/// Something the repair left as it was. Edge lists are in ascending id order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unfixed {
    /// Connected open edges that don't form one closed loop: some vertex has other than two
    /// of them.
    NotOneLoop(Vec<EdgeId>),
    /// A closed loop of open edges whose vertices don't lie in one plane.
    NotFlat(Vec<EdgeId>),
    /// A flat loop in the same plane as another one, one inside the other (the border of a
    /// ring-shaped hole).
    Nested(Vec<EdgeId>),
    /// The face that would fill the loop isn't a valid face.
    BadFill(Vec<EdgeId>),
    /// The face that would fill the loop overlaps a face that's already there (the border
    /// of a flat sheet, not a hole in a solid).
    Covered(Vec<EdgeId>),
    /// An edge still used by three or more faces.
    SharedEdge(EdgeId),
    /// Groups and components placed inside the body (`repair_body` only).
    NestedInstances(usize),
}

/// What a repair did and what it couldn't do. Id lists are in ascending order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepairReport {
    /// Internal faces deleted (step 1).
    pub removed_internal: Vec<FaceId>,
    /// Stray faces deleted (step 2).
    pub removed_stray: Vec<FaceId>,
    /// Edges deleted because no face used them (step 3), whether loose before the repair or
    /// left loose by steps 1 and 2.
    pub removed_edges: Vec<EdgeId>,
    /// Faces added to fill holes (step 4).
    pub filled: Vec<FaceId>,
    /// Faces turned round (step 5); they keep their ids.
    pub reversed: Vec<FaceId>,
    /// What's left, in the order of the spec.
    pub unfixed: Vec<Unfixed>,
    /// The solid check of the result.
    pub after: SolidReport,
}

impl RepairReport {
    /// True when the repair removed, added or turned anything.
    pub fn changed(&self) -> bool {
        !(self.removed_internal.is_empty()
            && self.removed_stray.is_empty()
            && self.removed_edges.is_empty()
            && self.filled.is_empty()
            && self.reversed.is_empty())
    }
}

/// Why `repair_body` was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepairError {
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
}

/// Repair the geometry of one context, appending every change to `log`.
pub fn repair_geometry(ctx: &mut Context, log: &mut Edit) -> RepairReport {
    let _ = (ctx, log);
    unimplemented!("m7-solid-repair")
}

/// Repair the body's geometry, carry the paint of turned faces to their new sides and push
/// one undo step named `label`. Pushes nothing and leaves the model alone when nothing
/// changes.
pub fn repair_body(
    model: &mut Model,
    body: BodyId,
    undo: &mut UndoStack,
    label: &str,
) -> Result<RepairReport, RepairError> {
    let _ = (model, body, undo, label);
    unimplemented!("m7-solid-repair")
}
