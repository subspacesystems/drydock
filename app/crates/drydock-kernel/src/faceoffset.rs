// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Offset a face in its own plane: every boundary loop moves sideways by one distance, so the
//! face's region grows (positive distance) or shrinks (negative), with mitred corners.
//!
//! Behaviour: `docs/specs/m9-offset-face.md`. Not the M7 clearance offset in `offset.rs`, which
//! moves a solid's faces along their normals.

use glam::DVec3;

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::topology::{Context, EdgeId, FaceId, Violation};

/// Longest mitre, as a multiple of the distance. A corner whose mitre would be longer (an
/// interior angle under about 29 degrees on the side the loop moves away from) is bevelled.
pub const MITRE_LIMIT: f64 = 4.0;

/// Why an offset was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum FaceOffsetError {
    MissingBody(BodyId),
    MissingFace(FaceId),
    /// The distance is not finite, or not longer than `LINEAR_TOL`.
    BadDistance,
    /// The face has no plane, or one of its loop points is off it by more than `LINEAR_TOL`.
    NotPlanar(FaceId),
    /// An offset edge would shrink to `LINEAR_TOL` or less, or turn around: the distance is
    /// too large for the loop (inward for the outer loop, outward for a hole).
    Collapsed,
    /// The offset loops are not a face outline: a loop crosses or touches itself, two loops
    /// cross or touch, or an offset hole is not strictly inside the offset outer loop.
    LoopsCross,
    /// The result would break a `validate` invariant that held before.
    Invalid(Violation),
    /// The result would make two coplanar faces overlap that didn't before (lower id first).
    Overlap(FaceId, FaceId),
    Model(ModelError),
}

/// The faces an offset left. Every face fronts the way the offset face did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceOffset {
    /// The face inside the offset outer loop. Inward: a new face bounded by the offset outer
    /// loop, with the offset holes. Outward: the original face, unchanged.
    pub inner: FaceId,
    /// The ring between the two outer loops. Inward: the original face (same id), now its old
    /// outer loop with the offset outer loop as its only hole. Outward: a new face, the offset
    /// outer loop with the old outer loop as its only hole.
    pub ring: FaceId,
    /// One new face per hole of the original face, in the face's hole order, between the old
    /// hole loop and its offset.
    pub hole_rings: Vec<FaceId>,
    /// The new edges of each offset loop: outer loop first, then each hole in order; each list
    /// in the old loop's direction, starting at the offset of the loop's first vertex.
    pub edges: Vec<Vec<EdgeId>>,
}

/// The points of `face`'s loops offset by `distance` in the face plane (positive grows the
/// face), without changing anything: outer loop first, then each hole in order, each in the
/// old loop's direction starting at the offset of its first vertex; a bevelled corner gives
/// two points. Fails as [`offset_face`] does, up to and including `LoopsCross`.
pub fn offset_loops(
    ctx: &Context,
    face: FaceId,
    distance: f64,
) -> Result<Vec<Vec<DVec3>>, FaceOffsetError> {
    let _ = (ctx, face, distance);
    unimplemented!("m9-offset-face")
}

/// Offset `face` of `body` by `distance` in its plane (positive outward), as one undo step
/// labelled `label`.
pub fn offset_face(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<FaceOffset, FaceOffsetError> {
    let _ = (model, body, face, distance, undo, label);
    unimplemented!("m9-offset-face")
}
