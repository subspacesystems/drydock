// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tape tool: click a start point and a second point to read the distance; start on an edge
//! and click (or type an offset) to leave a guide line parallel to it. With the point-guide
//! modifier on, each click leaves a guide point.
//!
//! Behaviour: `docs/specs/m3-tool-tape.md`.

use glam::DVec3;

use crate::geom::LINEAR_TOL;
use crate::model::{ModelEdit, ModelError};
use crate::tape::{Guide, measure, parallel_guide, point_guide};
use crate::units::{self, ParseError};

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
        (self.stage == TapeStage::End)
            .then_some(self.start)
            .flatten()
    }

    /// The edge the guide will be parallel to, at `End` of a parallel measurement.
    pub fn edge(&self) -> Option<(DVec3, DVec3)> {
        if self.stage == TapeStage::End {
            self.edge
        } else {
            None
        }
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
        if self.stage != TapeStage::End {
            return None;
        }
        let cursor = self.cursor?;
        if let Some((a, b)) = self.edge {
            line_distance(a, b, cursor)
        } else {
            self.start.map(|start| measure(start, cursor))
        }
    }

    /// The distance of the last finished measurement or guide, in meters.
    pub fn last_distance(&self) -> Option<f64> {
        self.last
    }

    /// The guide the next click would leave, in a parallel measurement.
    pub fn preview(&self) -> Option<Guide> {
        if self.stage != TapeStage::End {
            return None;
        }
        let (a, b) = self.edge?;
        parallel_guide(a, b, self.cursor?, None)
    }

    /// Guides go into `target.body` as one undo step each.
    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), TapeError> {
        match input {
            Input::Move(p) => self.moved(p),
            Input::Click(p) => self.clicked(p, target),
            Input::Typed(text) => self.typed(&text, target),
            Input::Cancel => {
                self.cancel();
                Ok(())
            }
        }
    }

    fn add(&mut self, target: &mut Target, guide: Guide, label: &str) -> Result<(), TapeError> {
        let mut edit = ModelEdit::new();
        target
            .model
            .add_guide(target.body, guide, &mut edit)
            .map_err(TapeError::Model)?;
        target.undo.push(label, edit);
        Ok(())
    }

    fn reset(&mut self) {
        self.stage = TapeStage::Start;
        self.start = None;
        self.edge = None;
        self.cursor = None;
    }

    fn moved(&mut self, p: DVec3) -> Result<(), TapeError> {
        if !finite(p) {
            return Err(TapeError::NotFinite);
        }
        if self.stage == TapeStage::End {
            self.cursor = Some(p);
        }
        Ok(())
    }

    fn clicked(&mut self, p: DVec3, target: &mut Target) -> Result<(), TapeError> {
        if !finite(p) {
            return Err(TapeError::NotFinite);
        }
        match self.stage {
            TapeStage::Start => {
                if self.point_guides {
                    let guide = point_guide(p).ok_or(TapeError::NotFinite)?;
                    return self.add(target, guide, "Guide point");
                }
                self.start = Some(p);
                self.stage = TapeStage::End;
                self.cursor = None;
                self.edge = self.hover_edge.filter(|&(a, b)| a.distance(b) > LINEAR_TOL);
                self.hover_edge = None;
                Ok(())
            }
            TapeStage::End => {
                if let Some((a, b)) = self.edge {
                    let guide = parallel_guide(a, b, p, None).ok_or(TapeError::NoDirection)?;
                    let last = guide.distance_to(a);
                    self.add(target, guide, "Guide")?;
                    self.last = Some(last);
                } else {
                    let Some(start) = self.start else {
                        return Ok(());
                    };
                    self.last = Some(measure(start, p));
                }
                self.reset();
                Ok(())
            }
        }
    }

    fn typed(&mut self, text: &str, target: &mut Target) -> Result<(), TapeError> {
        if self.stage == TapeStage::Start {
            return Err(TapeError::NotStarted);
        }
        let Some((a, b)) = self.edge else {
            return Err(TapeError::NotParallel);
        };
        let length = units::parse_length(text, target.unit).map_err(TapeError::Parse)?;
        if length <= LINEAR_TOL {
            return Err(TapeError::BadLength);
        }
        let cursor = self.cursor.ok_or(TapeError::NoDirection)?;
        let guide = parallel_guide(a, b, cursor, Some(length)).ok_or(TapeError::NoDirection)?;
        self.add(target, guide, "Guide")?;
        self.last = Some(length);
        self.reset();
        Ok(())
    }

    fn cancel(&mut self) {
        if self.stage == TapeStage::End {
            self.reset();
        }
    }
}

fn finite(p: DVec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

fn line_distance(a: DVec3, b: DVec3, p: DVec3) -> Option<f64> {
    let edge = b - a;
    let length = edge.length();
    if !length.is_finite() || length < LINEAR_TOL {
        return None;
    }
    Some((p - a).cross(edge / length).length())
}
