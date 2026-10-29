// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fit check for 3D printing: which triangles of two parts come closer to the other part
//! than the clearance, and a scene copy with those triangles tinted.
//!
//! Behaviour: `docs/specs/m7-clearance.md`.

use glam::{DMat4, DVec3};
use modeler_kernel::model as km;

use crate::FlatScene;

/// A triangle's corners, in meters.
pub type Triangle = [DVec3; 3];

/// The clearance the app starts with: 0.2 mm, in meters.
pub const DEFAULT_CLEARANCE_M: f64 = 0.0002;

/// Color of the material [`tint_fit`] adds.
pub const FIT_COLOR: [f32; 4] = [0.95, 0.75, 0.1, 1.0];

/// How close a scene triangle's corners must be to a flagged triangle's corners for
/// [`tint_fit`] to tint it, in meters.
pub const MATCH_TOLERANCE_M: f64 = 1e-5;

/// Which triangles of two parts are closer to the other part than the clearance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FitReport {
    /// Indices into the first list, ascending.
    pub a: Vec<usize>,
    /// Indices into the second list, ascending.
    pub b: Vec<usize>,
}

/// The shortest distance between the two closed triangles (their points, not just their
/// corners): 0 when they touch, share a point or cross.
pub fn triangle_distance(_a: &Triangle, _b: &Triangle) -> f64 {
    unimplemented!("m7-clearance")
}

/// The triangles of `a` and of `b` that lie within `clearance` (exclusive) of some
/// triangle of the other list.
pub fn close_triangles(_a: &[Triangle], _b: &[Triangle], _clearance: f64) -> FitReport {
    unimplemented!("m7-clearance")
}

/// Every triangle of the faces of `instance` (an instance in `body`) and of the instances
/// nested in it, placed by the instance's transform and then by `outer`: the triangles
/// `kernel_scene` draws for it.
pub fn instance_triangles(
    _model: &km::Model,
    _body: km::BodyId,
    _instance: km::InstanceId,
    _outer: DMat4,
) -> Vec<Triangle> {
    unimplemented!("m7-clearance")
}

/// A copy of `scene` with every triangle that matches one of `flagged` moved into one batch
/// drawn in [`FIT_COLOR`].
pub fn tint_fit(_scene: &FlatScene, _flagged: &[Triangle]) -> FlatScene {
    unimplemented!("m7-clearance")
}
