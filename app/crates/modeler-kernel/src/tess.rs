// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Triangulation of planar faces for drawing.
//!
//! Behaviour: `docs/specs/m1-kernel-scene.md`.

use glam::DVec2;

/// Triangles covering the polygon `outer` minus the `holes`. A triangle is three indices
/// into the points listed in order: `outer`, then each hole in turn. Every triangle runs
/// counter-clockwise (or is flat); no new points are added.
pub fn triangulate(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[usize; 3]> {
    let _ = (outer, holes);
    unimplemented!()
}
