// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rectangle tool: click one corner, click the opposite one (or type `width,height`).

use glam::DVec3;

use super::plane::DrawPlane;
use super::{Input, Target, ToolError};

#[derive(Clone, Debug)]
pub struct RectTool {
    #[allow(dead_code)]
    plane: DrawPlane,
}

impl RectTool {
    pub fn new(plane: DrawPlane) -> Self {
        Self { plane }
    }

    /// The first corner while drawing. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        unimplemented!()
    }

    /// The four corners the next click would draw.
    pub fn preview(&self) -> Option<[DVec3; 4]> {
        unimplemented!()
    }

    /// `(width, height)` of `preview`, along `u` and `v`, for the measurement box.
    pub fn size(&self) -> Option<(f64, f64)> {
        unimplemented!()
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        let _ = (input, target);
        unimplemented!()
    }
}
