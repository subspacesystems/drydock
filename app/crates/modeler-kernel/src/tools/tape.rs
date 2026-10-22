// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tape tool: click a start point and a second point to read the distance; start on an edge
//! and click (or type an offset) to leave a guide line parallel to it. With the point-guide
//! modifier on, each click leaves a guide point.
//!
//! Behaviour: `docs/specs/m3-tool-tape.md`.

use glam::DVec3;

use crate::model::ModelError;
use crate::tape::Guide;
use crate::units::ParseError;

use super::{Input, Target};

/// Why the tool refused an input. The tool, the model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum TapeError {
    /// The typed text is not a length.
    Parse(ParseError),
    /// A typed length needs a measurement in progress.
    NotStarted,
    /// A typed length only means something for a guide parallel to an edge.
    NotParallel,
    /// A clicked or moved-to point is not finite.
    NotFinite,
    /// The cursor gives no side to put the guide on (it is on the edge's line, or has not
    /// moved since the start click).
    NoDirection,
    /// The typed length is zero or negative.
    BadLength,
    Model(ModelError),
}

/// What the next click does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapeStage {
    /// Place the start point.
    Start,
    /// Measure, or place the guide.
    End,
}

#[derive(Clone, Debug)]
pub struct TapeTool {
    stage: TapeStage,
    start: Option<DVec3>,
    edge: Option<(DVec3, DVec3)>,
    hover_edge: Option<(DVec3, DVec3)>,
    point_guides: bool,
    cursor: Option<DVec3>,
    last: Option<f64>,
}

impl Default for TapeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TapeTool {
    pub fn new() -> Self {
        Self {
            stage: TapeStage::Start,
            start: None,
            edge: None,
            hover_edge: None,
            point_guides: false,
            cursor: None,
            last: None,
        }
    }

    pub fn stage(&self) -> TapeStage {
        self.stage
    }

    /// The start point at `End`, else `None`. Use it as `Probe::anchor`.
    pub fn start(&self) -> Option<DVec3> {
        let _ = &self.start;
        unimplemented!("m3-tool-tape")
    }

    /// The edge the guide will be parallel to, at `End` of a parallel measurement.
    pub fn edge(&self) -> Option<(DVec3, DVec3)> {
        let _ = &self.edge;
        unimplemented!("m3-tool-tape")
    }

    /// The edge under the cursor, as the editor's hit test found it; `None` when the cursor
    /// is not on an edge. Read by the next start click.
    pub fn set_hover_edge(&mut self, edge: Option<(DVec3, DVec3)>) {
        self.hover_edge = edge;
    }

    /// The point-guide modifier: while on, a click at `Start` leaves a guide point.
    pub fn set_point_guides(&mut self, on: bool) {
        self.point_guides = on;
    }

    /// The live distance in meters at `End`: start to cursor, or for a parallel measurement
    /// the cursor's distance from the edge's line. `None` before the cursor moves.
    pub fn distance(&self) -> Option<f64> {
        let _ = (&self.cursor, &self.hover_edge);
        unimplemented!("m3-tool-tape")
    }

    /// The distance of the last finished measurement or guide, in meters.
    pub fn last_distance(&self) -> Option<f64> {
        self.last
    }

    /// The guide the next click would leave, in a parallel measurement.
    pub fn preview(&self) -> Option<Guide> {
        unimplemented!("m3-tool-tape")
    }

    /// Guides go into `target.body` as one undo step each.
    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), TapeError> {
        let _ = (input, target);
        unimplemented!("m3-tool-tape")
    }
}
