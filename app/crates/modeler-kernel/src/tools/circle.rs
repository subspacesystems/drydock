// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Circle tool: click the center, click a point on the circle (or type the radius).
//! Typing `12s` sets the number of segments.

use glam::DVec3;

use super::plane::DrawPlane;
use super::{Input, Target, ToolError};

#[derive(Clone, Debug)]
pub struct CircleTool {
    #[allow(dead_code)]
    plane: DrawPlane,
}

impl CircleTool {
    /// Idle, 24 segments.
    pub fn new(plane: DrawPlane) -> Self {
        Self { plane }
    }

    /// The center while drawing. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        unimplemented!()
    }

    pub fn segments(&self) -> usize {
        unimplemented!()
    }

    /// The points the next click would draw.
    pub fn preview(&self) -> Option<Vec<DVec3>> {
        unimplemented!()
    }

    /// Radius of `preview`, for the measurement box.
    pub fn radius(&self) -> Option<f64> {
        unimplemented!()
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        let _ = (input, target);
        unimplemented!()
    }
}
