// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Three-point arc tool: click the arc's start, click its end, click a point the arc passes
//! through (the bulge). Typed: the chord length, then the bulge height.
//! Typing `8s` sets the number of segments.
//!
//! Behaviour: `docs/specs/m2-arc-3pt.md`.

use glam::DVec3;

use super::plane::DrawPlane;
use super::{Input, Target, ToolError};

#[derive(Clone, Debug)]
pub struct Arc3Tool {
    plane: DrawPlane,
}

/// The `segments + 1` points of the arc that runs from `a` to `b` and passes through
/// `through`, all measured in the plane through `a` with normal `normal`. The first point is
/// exactly `a` and the last exactly `b`. `None` when the three points are (nearly) on a line,
/// when `a` and `b` are within `LINEAR_TOL`, when `segments` is 0, or on non-finite input.
pub fn three_point_arc(
    normal: DVec3,
    a: DVec3,
    b: DVec3,
    through: DVec3,
    segments: usize,
) -> Option<Vec<DVec3>> {
    let _ = (normal, a, b, through, segments);
    unimplemented!("m2-arc-3pt")
}

impl Arc3Tool {
    /// Idle, 12 segments.
    pub fn new(plane: DrawPlane) -> Self {
        Self { plane }
    }

    pub fn plane(&self) -> DrawPlane {
        self.plane
    }

    /// Use `plane` for the next shape. Ignored while a shape is in progress.
    pub fn set_plane(&mut self, plane: DrawPlane) {
        let _ = plane;
        unimplemented!("m2-arc-3pt")
    }

    /// Points placed so far: 0 idle, 1 the start, 2 the start and the end.
    pub fn placed(&self) -> usize {
        unimplemented!("m2-arc-3pt")
    }

    /// The last point placed. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        unimplemented!("m2-arc-3pt")
    }

    pub fn segments(&self) -> usize {
        unimplemented!("m2-arc-3pt")
    }

    /// Chord length: start to cursor (one point placed) or start to end (two placed).
    pub fn chord(&self) -> Option<f64> {
        unimplemented!("m2-arc-3pt")
    }

    /// Radius of the arc `preview` shows.
    pub fn radius(&self) -> Option<f64> {
        unimplemented!("m2-arc-3pt")
    }

    /// One point placed: the chord `[start, cursor]`. Two placed: the arc the next click
    /// would draw. Otherwise `None`.
    pub fn preview(&self) -> Option<Vec<DVec3>> {
        unimplemented!("m2-arc-3pt")
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        let _ = (input, target);
        unimplemented!("m2-arc-3pt")
    }
}
