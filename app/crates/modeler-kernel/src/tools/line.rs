// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Line tool: click, click, click... Each click draws one edge from the previous point; the
//! chain ends when an edge closes a face, or on `Cancel`.

use glam::DVec3;

use crate::geom::LINEAR_TOL;

use super::{Input, Target, ToolError, draw_segments};

#[derive(Clone, Debug, Default)]
pub struct LineTool {
    start: Option<DVec3>,
    cursor: Option<DVec3>,
}

fn finite(p: DVec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

impl LineTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// The point the next edge starts from, while drawing. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        self.start
    }

    /// The edge the next click would draw.
    pub fn preview(&self) -> Option<(DVec3, DVec3)> {
        let start = self.start?;
        let cursor = self.cursor?;
        (cursor.distance(start) > LINEAR_TOL).then_some((start, cursor))
    }

    /// Length of `preview`, for the measurement box.
    pub fn length(&self) -> Option<f64> {
        let (start, cursor) = self.preview()?;
        Some(cursor.distance(start))
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        match input {
            Input::Move(p) => {
                if finite(p) && self.start.is_some() {
                    self.cursor = Some(p);
                }
                Ok(())
            }
            Input::Click(p) => {
                if !finite(p) {
                    return Err(ToolError::Geometry(crate::sticky::StickyError::NotFinite));
                }
                let Some(start) = self.start else {
                    self.start = Some(p);
                    self.cursor = None;
                    return Ok(());
                };
                if p.distance(start) <= LINEAR_TOL {
                    return Ok(());
                }
                self.place(p, target)
            }
            Input::Typed(text) => {
                let Some(start) = self.start else {
                    return Err(ToolError::NotStarted);
                };
                let len =
                    crate::units::parse_length(&text, target.unit).map_err(ToolError::Parse)?;
                let Some(cursor) = self.cursor else {
                    return Err(ToolError::NoDirection);
                };
                let direction = cursor - start;
                if direction.length() <= LINEAR_TOL {
                    return Err(ToolError::NoDirection);
                }
                self.place(start + direction.normalize() * len, target)
            }
            Input::Cancel => {
                self.start = None;
                self.cursor = None;
                Ok(())
            }
        }
    }

    fn place(&mut self, p: DVec3, target: &mut Target) -> Result<(), ToolError> {
        let start = self.start.unwrap_or(p);
        let chains = draw_segments(target, &[(start, p)], "Line")?;
        let Some(chain) = chains.first() else {
            self.start = None;
            self.cursor = None;
            return Ok(());
        };
        if !chain.faces.is_empty() {
            self.start = None;
            self.cursor = None;
            return Ok(());
        }
        let end = chain.vertices.last().and_then(|v| {
            target
                .model
                .body(target.body)?
                .geometry()
                .vertex(*v)
                .map(|vertex| vertex.position)
        });
        self.start = end;
        self.cursor = None;
        Ok(())
    }
}
