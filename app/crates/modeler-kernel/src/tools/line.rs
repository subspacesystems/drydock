// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Line tool: click, click, click... Each click draws one edge from the previous point; the
//! chain ends when an edge closes a face, or on `Cancel`.

use glam::DVec3;

use super::{Input, Target, ToolError};

#[derive(Clone, Debug, Default)]
pub struct LineTool {}

impl LineTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// The point the next edge starts from, while drawing. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        unimplemented!()
    }

    /// The edge the next click would draw.
    pub fn preview(&self) -> Option<(DVec3, DVec3)> {
        unimplemented!()
    }

    /// Length of `preview`, for the measurement box.
    pub fn length(&self) -> Option<f64> {
        unimplemented!()
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        let _ = (input, target);
        unimplemented!()
    }
}
