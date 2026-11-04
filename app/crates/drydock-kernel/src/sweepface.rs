// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Follow Me: sweep a planar profile face along a path, with mitred joints. An open path gives
//! a solid with the profile as its start cap and a new end cap; a closed path gives a ring.
//!
//! Behaviour: `docs/specs/m9-sweep-face.md`. The path comes from `sweep.rs`.

use glam::DVec3;

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::sweep::SweepPath;
use crate::topology::{Context, EdgeId, FaceId, VertexId, Violation};

/// Smallest angle, in degrees, between the profile's plane and the path segment the sweep
/// starts along. A profile closer to parallel than this is refused (`ProfileParallel`).
pub const MIN_PROFILE_ANGLE_DEG: f64 = 1.0;

/// Largest turn, in degrees, between two consecutive path segments. A sharper turn (the path
/// nearly doubling back) has no usable mitre plane (`DegenerateMitre`).
pub const MAX_TURN_DEG: f64 = 179.0;

/// Why a sweep was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum SweepError {
    MissingBody(BodyId),
    /// The profile face isn't in the body.
    MissingFace(FaceId),
    /// Too few points (open: 2, closed: 3), `vertices` and `points` of different lengths, a
    /// point that is not finite, or a segment (a closed path's closing one too) no longer
    /// than `LINEAR_TOL`.
    BadPath,
    /// The lowest-id edge of the profile that another face also uses.
    ProfileShared(EdgeId),
    /// The path turns by more than `MAX_TURN_DEG` at this vertex.
    DegenerateMitre(VertexId),
    /// Closed path: the profile's plane doesn't meet the path.
    NotOnPath,
    /// Closed path: the profile's plane meets the path at this vertex, but the profile doesn't
    /// lie in the vertex's mitre plane.
    MitreMismatch(VertexId),
    /// The profile's plane is within `MIN_PROFILE_ANGLE_DEG` of parallel to the segment the
    /// sweep starts along.
    ProfileParallel,
    /// Some profile point would not move forward (by more than `LINEAR_TOL`) on the way to
    /// the section plane at this path vertex: the profile is too big for the turn, or lies
    /// beyond the vertex.
    Folded(VertexId),
    /// Closed path: carried once round, the profile doesn't come back onto itself (a path
    /// that isn't flat twists it).
    Twisted,
    /// The result would break a `validate` invariant that held before.
    Invalid(Violation),
    /// The result would make two coplanar faces overlap that didn't before (lower id first).
    Overlap(FaceId, FaceId),
    Model(ModelError),
}

/// The faces a sweep left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Swept {
    /// Open path: the profile face (same id), fronting away from the sweep. Closed: `None`.
    pub start: Option<FaceId>,
    /// Open path: the new face at the end of the path. Closed: `None`.
    pub end: Option<FaceId>,
    /// The side faces left after coplanar ones merged, ascending.
    pub sides: Vec<FaceId>,
}

/// The profile's cross-sections along the path, without changing anything (for the tool's
/// preview): one per section plane in sweep order; each holds the profile's loops (outer
/// first, then each hole in the face's order), each loop's points in the profile loop's own
/// order starting at its first use's start vertex. Fails as [`sweep_face`] does, up to and
/// including `Twisted`.
pub fn sweep_sections(
    ctx: &Context,
    profile: FaceId,
    path: &SweepPath,
) -> Result<Vec<Vec<Vec<DVec3>>>, SweepError> {
    let _ = (ctx, profile, path);
    unimplemented!("m9-sweep-face")
}

/// Sweep `profile` of `body` along `path` as one undo step labelled `label`. Only
/// `path.points`, `path.closed` and (for errors) `path.vertices` are used; the path's
/// vertices and edges are left as they are.
pub fn sweep_face(
    model: &mut Model,
    body: BodyId,
    profile: FaceId,
    path: &SweepPath,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Swept, SweepError> {
    let _ = (model, body, profile, path, undo, label);
    unimplemented!("m9-sweep-face")
}
