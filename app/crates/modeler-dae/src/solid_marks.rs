// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Problem marks for 3D printing: the edges and faces of one group or component that keep
//! it from being a solid, in world space, and a scene copy with them drawn in
//! [`PROBLEM_COLOR`].
//!
//! Behaviour: `docs/specs/m7-solid-ui.md`.

use glam::{DMat4, DVec3};
use modeler_kernel::model as km;

use crate::{FlatScene, Triangle};

/// Color of the material [`tint_problems`] adds, for faces and edges alike.
pub const PROBLEM_COLOR: [f32; 4] = [0.85, 0.1, 0.6, 1.0];

/// How close a scene triangle's corners or a scene line's ends must be to a mark's for
/// [`tint_problems`] to take it, in meters.
pub const MARK_TOLERANCE_M: f64 = 1e-5;

/// What is wrong with one group or component, in world space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidMarks {
    /// Every triangle of the reversed and internal faces.
    pub triangles: Vec<Triangle>,
    /// The open, loose and shared edges.
    pub segments: Vec<[DVec3; 2]>,
}

impl SolidMarks {
    /// No triangles and no segments.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty() && self.segments.is_empty()
    }
}

/// The marks of `instance` (an instance in `body`): `check_geometry` of its definition's own
/// geometry (groups and components nested in it are not looked into), the triangles of its
/// `reversed_faces` and `internal_faces` and the end points of its `open_edges`,
/// `loose_edges` and `shared_edges`, placed by the instance's transform and then by `outer`.
/// The triangles are the ones `kernel_scene` draws for those faces. Empty when the instance
/// or its definition doesn't exist or nothing is wrong.
pub fn solid_marks(
    _model: &km::Model,
    _body: km::BodyId,
    _instance: km::InstanceId,
    _outer: DMat4,
) -> SolidMarks {
    unimplemented!("m7-solid-ui")
}

/// A copy of `scene` with every triangle that matches a mark triangle (the same three corners
/// in any order, each within [`MARK_TOLERANCE_M`]) moved into one batch, and every line
/// segment that matches a mark segment (the same two ends in either order) moved out of its
/// line batch into one new line batch, both drawn in a material named "Problem" in
/// [`PROBLEM_COLOR`] that is added after the scene's own. When nothing matches (or `marks`
/// is empty) the copy is the scene unchanged, with no material added. Must stay fast for
/// scenes of 100 000 triangles and as many marks.
pub fn tint_problems(_scene: &FlatScene, _marks: &SolidMarks) -> FlatScene {
    unimplemented!("m7-solid-ui")
}
