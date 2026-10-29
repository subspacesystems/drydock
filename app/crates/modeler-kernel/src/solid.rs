// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid check for 3D printing: open, loose and shared edges, internal and reversed faces,
//! shells and volume of one body.
//!
//! Behaviour: `docs/specs/m7-solid-check.md`.

use glam::DVec3;

use crate::model::{BodyId, Model};
use crate::topology::{Context, EdgeId, FaceId};

/// What `check_geometry` found in one context. Every list is in ascending id order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidReport {
    /// Groups of faces joined across edges that exactly two faces use.
    pub shells: usize,
    /// Edges exactly one face uses (borders of holes in the surface).
    pub open_edges: Vec<EdgeId>,
    /// Edges no face uses.
    pub loose_edges: Vec<EdgeId>,
    /// Edges three or more faces use.
    pub shared_edges: Vec<EdgeId>,
    /// Faces whose every edge is a shared edge (walls inside the solid).
    pub internal_faces: Vec<FaceId>,
    /// Faces whose front points into the material instead of out of it.
    pub reversed_faces: Vec<FaceId>,
    /// Groups and components placed inside the body (`check_body` only).
    pub nested_instances: usize,
    /// Enclosed volume in cubic meters, with reversed faces counted the right way round;
    /// `Some` exactly when `is_solid`.
    pub volume: Option<f64>,
}

impl SolidReport {
    /// At least one shell, no open, loose or shared edges, no internal faces and no nested
    /// instances. Reversed faces don't count: see `is_clean`.
    pub fn is_solid(&self) -> bool {
        unimplemented!()
    }

    /// Solid and no reversed faces: ready to export as it is.
    pub fn is_clean(&self) -> bool {
        unimplemented!()
    }
}

/// Checks the geometry of one context; `nested_instances` is 0.
pub fn check_geometry(ctx: &Context) -> SolidReport {
    let _ = ctx;
    unimplemented!()
}

/// Checks the body's geometry and counts its instances; `None` when the body doesn't exist.
pub fn check_body(model: &Model, body: BodyId) -> Option<SolidReport> {
    let _ = (model, body);
    unimplemented!()
}

/// Winding number of `faces` (each counted with its front as stored) around `point`: about 1
/// inside a closed surface whose fronts point out, -1 inside one whose fronts point in, 0
/// outside.
pub fn winding_number(ctx: &Context, faces: &[FaceId], point: DVec3) -> f64 {
    let _ = (ctx, faces, point);
    unimplemented!()
}
