// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Push/Pull: extrude a face along its normal, or slide it when its neighbours stretch.
//!
//! Behaviour: `docs/specs/m3-pushpull.md`.

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::topology::{FaceId, Violation};

/// Why a push/pull was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum PushPullError {
    MissingBody(BodyId),
    MissingFace(FaceId),
    /// The distance is not finite, or not longer than `LINEAR_TOL`.
    BadDistance,
    /// The result would break a `validate` invariant that held before.
    Invalid(Violation),
    /// The result would make two coplanar faces overlap that didn't before (lower id first).
    Overlap(FaceId, FaceId),
    /// A slide would turn this face (a stretched neighbour) inside out.
    Inverted(FaceId),
    Model(ModelError),
}

/// The faces a push/pull left in place of the pushed face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushPulled {
    /// The face at the new position. The pushed face itself when it slid.
    pub cap: FaceId,
    /// New side faces, one per loop use: outer loop first, then each hole, in loop order.
    /// Empty when the face slid.
    pub sides: Vec<FaceId>,
    /// The face left at the old position, for a face that shared no edge with another face.
    pub base: Option<FaceId>,
}

/// Push or pull `face` of `body` by `distance` along its front normal (negative goes
/// backwards), as one undo step labelled `label`.
pub fn push_pull(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<PushPulled, PushPullError> {
    let _ = (model, body, face, distance, undo, label);
    unimplemented!("m3-pushpull")
}
