// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Grow or shrink a flat-faced solid: every face moves along its outward normal by the same
//! distance, so a part prints larger or smaller by a fit clearance.
//!
//! Behaviour: `docs/specs/m7-clearance-offset.md`.

use std::collections::BTreeMap;

use glam::DVec3;

use crate::model::{BodyId, Model, ModelEdit, UndoStack};
use crate::solid::SolidReport;
use crate::topology::{Context, EdgeId, FaceId, VertexId};

/// Two outward normals at a vertex belong to the same plane when their dot product is
/// above this.
pub const SAME_PLANE_DOT: f64 = 1.0 - 1e-9;

/// A normal adds a direction to a vertex's basis when its cross product with the first
/// basis normal (second pick) or its triple product with the first two (third pick) is
/// larger than this.
pub const RANK_TOL: f64 = 1e-6;

/// A vertex's move fits a plane when its distance along that plane's normal is within
/// `RESIDUAL_REL * |distance|` of the distance.
pub const RESIDUAL_REL: f64 = 1e-6;

/// Why a body can't be grown or shrunk. Nothing changed. Id lists are in ascending order.
#[derive(Clone, Debug, PartialEq)]
pub enum OffsetError {
    /// The distance is zero, NaN or infinite.
    BadDistance,
    /// The body doesn't exist.
    MissingBody(BodyId),
    /// The body isn't a solid (`SolidReport::is_solid`); the check says why.
    NotSolid(Box<SolidReport>),
    /// The body has this many arcs or circles: only flat-faced parts can be offset.
    Curved(usize),
    /// Vertices whose faces' moved planes don't meet in one point.
    Unsupported(Vec<VertexId>),
    /// Edges that would shrink to nothing or turn round, and faces whose loops would; the
    /// part is thinner than twice the distance there, or a hole or cavity would close.
    Collapse {
        edges: Vec<EdgeId>,
        faces: Vec<FaceId>,
    },
    /// The moved surface would touch or cross itself (a gap or slot would close).
    SelfIntersect,
}

/// What an offset did.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetReport {
    /// Vertices moved (every vertex of the body).
    pub moved: usize,
    /// Volume in cubic meters before the offset.
    pub volume_before: f64,
    /// Volume in cubic meters after the offset.
    pub volume_after: f64,
}

/// Where every vertex of `ctx` goes when each face moves `distance` along its outward
/// normal (positive grows, negative shrinks), after every check in the spec. `ctx` is not
/// changed.
pub fn offset_positions(
    ctx: &Context,
    distance: f64,
) -> Result<BTreeMap<VertexId, DVec3>, OffsetError> {
    let _ = (ctx, distance);
    unimplemented!("m7-clearance-offset")
}

/// Offset the body's geometry by `distance`, appending the vertex moves to `log`. On an
/// error the model and `log` are unchanged.
pub fn offset_body(
    model: &mut Model,
    body: BodyId,
    distance: f64,
    log: &mut ModelEdit,
) -> Result<OffsetReport, OffsetError> {
    let _ = (model, body, distance, log);
    unimplemented!("m7-clearance-offset")
}

/// Offset each body by `distance`, all or nothing per body; the bodies that succeed go into
/// one undo step named `label` (none when no body changed). One result per entry of
/// `bodies`; a body listed again is offset once and repeats its first result.
pub fn offset_bodies(
    model: &mut Model,
    bodies: &[BodyId],
    distance: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Vec<Result<OffsetReport, OffsetError>> {
    let _ = (model, bodies, distance, undo, label);
    unimplemented!("m7-clearance-offset")
}
