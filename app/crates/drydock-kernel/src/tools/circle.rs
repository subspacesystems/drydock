// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Circle tool: click the center, click a point on the circle (or type the radius).
//! Typing `12s` sets the number of segments.

use glam::DVec3;

use crate::geom::LINEAR_TOL;
use crate::shapes::circle;
use crate::sticky::StickyError;
use crate::units::{ParseError, parse_length};

use super::plane::DrawPlane;
use super::{Input, Target, ToolError, draw_curve};

#[derive(Clone, Debug)]
pub struct CircleTool {
    plane: DrawPlane,
    center: Option<DVec3>,
    cursor: Option<DVec3>,
    segments: usize,
}

/// `Some(n)` for text like `12s`; `Err` when it has the form but `n` is not allowed.
pub(super) fn segment_count(text: &str, min: usize) -> Option<Result<usize, ToolError>> {
    let t = text.trim();
    let digits = t.strip_suffix(['s', 'S'])?;
    Some(match digits.trim().parse::<usize>() {
        Ok(n) if (min..=360).contains(&n) => Ok(n),
        _ => Err(ToolError::Parse(ParseError::Invalid)),
    })
}

impl CircleTool {
    /// Idle, 24 segments.
    pub fn new(plane: DrawPlane) -> Self {
        Self {
            plane,
            center: None,
            cursor: None,
            segments: 24,
        }
    }

    pub fn plane(&self) -> DrawPlane {
        self.plane
    }

    /// Use `plane` for the next shape. Ignored while a shape is in progress.
    pub fn set_plane(&mut self, plane: DrawPlane) {
        if self.center.is_none() {
            self.plane = plane;
        }
    }

    /// The center while drawing. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        self.center
    }

    pub fn segments(&self) -> usize {
        self.segments
    }

    fn radial(&self, p: DVec3) -> Option<DVec3> {
        let c = self.center?;
        let q = self.plane.project(c, p) - c;
        (q.length() > LINEAR_TOL).then_some(q)
    }

    /// The points the next click would draw.
    pub fn preview(&self) -> Option<Vec<DVec3>> {
        let c = self.center?;
        let q = self.radial(self.cursor?)?;
        Some(circle(c, self.plane.normal(), q, q.length(), self.segments))
    }

    /// Radius of `preview`, for the measurement box.
    pub fn radius(&self) -> Option<f64> {
        Some(self.radial(self.cursor?)?.length())
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        match input {
            Input::Move(p) => {
                if p.is_finite() && self.center.is_some() {
                    self.cursor = Some(p);
                }
                Ok(())
            }
            Input::Click(p) => {
                if !p.is_finite() {
                    return Err(ToolError::Geometry(StickyError::NotFinite));
                }
                let Some(c) = self.center else {
                    self.center = Some(p);
                    self.cursor = None;
                    return Ok(());
                };
                match self.radial(p) {
                    Some(q) => self.place(c, q, q.length(), target),
                    None => Ok(()),
                }
            }
            Input::Typed(text) => {
                if let Some(n) = segment_count(&text, 3) {
                    self.segments = n?;
                    return Ok(());
                }
                let Some(c) = self.center else {
                    return Err(ToolError::NotStarted);
                };
                let r = parse_length(&text, target.unit)
                    .map_err(ToolError::Parse)?
                    .abs();
                if r <= LINEAR_TOL {
                    return Err(ToolError::NoDirection);
                }
                let x = self
                    .cursor
                    .and_then(|p| self.radial(p))
                    .unwrap_or(self.plane.u);
                self.place(c, x, r, target)
            }
            Input::Cancel => {
                self.center = None;
                self.cursor = None;
                Ok(())
            }
        }
    }

    fn place(&mut self, c: DVec3, x: DVec3, r: f64, target: &mut Target) -> Result<(), ToolError> {
        let pts = circle(c, self.plane.normal(), x, r, self.segments);
        draw_curve(target, &pts, true, "Circle")?;
        self.center = None;
        self.cursor = None;
        Ok(())
    }
}
