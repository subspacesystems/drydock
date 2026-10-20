// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Arc tool: click the center, click the start point (sets radius), click the end direction
//! (counter-clockwise from the start). Typed: radius, then a signed angle in degrees.
//! Typing `8s` sets the number of segments.

use glam::DVec3;

use super::plane::DrawPlane;
use super::{Input, Target, ToolError};

#[derive(Clone, Debug)]
pub struct ArcTool {
    #[allow(dead_code)]
    plane: DrawPlane,
}

impl ArcTool {
    /// Idle, 12 segments.
    pub fn new(plane: DrawPlane) -> Self {
        Self { plane }
    }

    /// The last point placed (the center, then the start point). Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        unimplemented!()
    }

    pub fn segments(&self) -> usize {
        unimplemented!()
    }

    /// The points the next click would draw (only once the start point is placed).
    pub fn preview(&self) -> Option<Vec<DVec3>> {
        unimplemented!()
    }

    /// Radius of the arc in progress, once the center and a cursor (or start) are known.
    pub fn radius(&self) -> Option<f64> {
        unimplemented!()
    }

    /// Sweep of `preview` in radians, counter-clockwise positive.
    pub fn sweep(&self) -> Option<f64> {
        unimplemented!()
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        let _ = (input, target);
        unimplemented!()
    }
}
