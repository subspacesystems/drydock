// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Arc tool: click the center, click the start point (sets radius), click the end direction
//! (counter-clockwise from the start). Typed: radius, then a signed angle in degrees.
//! Typing `8s` sets the number of segments.

use glam::DVec3;

use crate::geom::LINEAR_TOL;
use crate::shapes::{arc, sweep_angle};
use crate::sticky::StickyError;
use crate::units::{ParseError, parse_length};

use super::circle::segment_count;
use super::plane::DrawPlane;
use super::{Input, Target, ToolError, draw_segments};

#[derive(Clone, Debug)]
enum Stage {
    Idle,
    Center(DVec3),
    Start { center: DVec3, start: DVec3 },
}

#[derive(Clone, Debug)]
pub struct ArcTool {
    plane: DrawPlane,
    stage: Stage,
    cursor: Option<DVec3>,
    segments: usize,
}

impl ArcTool {
    /// Idle, 12 segments.
    pub fn new(plane: DrawPlane) -> Self {
        Self {
            plane,
            stage: Stage::Idle,
            cursor: None,
            segments: 12,
        }
    }

    /// The last point placed (the center, then the start point). Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        match self.stage {
            Stage::Idle => None,
            Stage::Center(c) => Some(c),
            Stage::Start { start, .. } => Some(start),
        }
    }

    pub fn segments(&self) -> usize {
        self.segments
    }

    fn radial(&self, c: DVec3, p: DVec3) -> Option<DVec3> {
        let q = self.plane.project(c, p) - c;
        (q.length() > LINEAR_TOL).then_some(q)
    }

    /// Radius of the arc in progress, once the center and a cursor (or start) are known.
    pub fn radius(&self) -> Option<f64> {
        match self.stage {
            Stage::Idle => None,
            Stage::Center(c) => Some(self.radial(c, self.cursor?)?.length()),
            Stage::Start { center, start } => Some((start - center).length()),
        }
    }

    /// Sweep of `preview` in radians, counter-clockwise positive.
    pub fn sweep(&self) -> Option<f64> {
        let Stage::Start { center, start } = self.stage else {
            return None;
        };
        let q = self.radial(center, self.cursor?)?;
        let s = sweep_angle(center, self.plane.normal(), start, center + q)?;
        (s >= 1e-9).then_some(s)
    }

    /// The points the next click would draw (only once the start point is placed).
    pub fn preview(&self) -> Option<Vec<DVec3>> {
        let Stage::Start { center, start } = self.stage else {
            return None;
        };
        let s = self.sweep()?;
        let pts = arc(
            center,
            self.plane.normal(),
            start - center,
            (start - center).length(),
            0.0,
            s,
            self.segments,
        );
        (!pts.is_empty()).then_some(pts)
    }

    pub fn handle(&mut self, input: Input, target: &mut Target) -> Result<(), ToolError> {
        match input {
            Input::Move(p) => {
                if p.is_finite() && !matches!(self.stage, Stage::Idle) {
                    self.cursor = Some(p);
                }
                Ok(())
            }
            Input::Click(p) => {
                if !p.is_finite() {
                    return Err(ToolError::Geometry(StickyError::NotFinite));
                }
                match self.stage {
                    Stage::Idle => {
                        self.stage = Stage::Center(p);
                        self.cursor = None;
                        Ok(())
                    }
                    Stage::Center(c) => {
                        if let Some(q) = self.radial(c, p) {
                            self.stage = Stage::Start {
                                center: c,
                                start: c + q,
                            };
                            self.cursor = None;
                        }
                        Ok(())
                    }
                    Stage::Start { center, start } => {
                        let Some(q) = self.radial(center, p) else {
                            return Ok(());
                        };
                        let Some(s) = sweep_angle(center, self.plane.normal(), start, center + q)
                        else {
                            return Ok(());
                        };
                        if s < 1e-9 {
                            return Ok(());
                        }
                        self.place(center, start, s, target)
                    }
                }
            }
            Input::Typed(text) => {
                if let Some(n) = segment_count(&text, 1) {
                    self.segments = n?;
                    return Ok(());
                }
                match self.stage {
                    Stage::Idle => Err(ToolError::NotStarted),
                    Stage::Center(c) => {
                        let r = parse_length(&text, target.unit)
                            .map_err(ToolError::Parse)?
                            .abs();
                        if r <= LINEAR_TOL {
                            return Err(ToolError::NoDirection);
                        }
                        let x = self
                            .cursor
                            .and_then(|p| self.radial(c, p))
                            .unwrap_or(self.plane.u)
                            .normalize();
                        self.stage = Stage::Start {
                            center: c,
                            start: c + x * r,
                        };
                        self.cursor = None;
                        Ok(())
                    }
                    Stage::Start { center, start } => {
                        let t = text.trim();
                        let t = t.strip_suffix('°').unwrap_or(t);
                        let deg: f64 = t
                            .parse()
                            .map_err(|_| ToolError::Parse(ParseError::Invalid))?;
                        if !deg.is_finite() || deg == 0.0 || deg.abs() > 360.0 {
                            return Err(ToolError::Parse(ParseError::Invalid));
                        }
                        self.place(center, start, deg.to_radians(), target)
                    }
                }
            }
            Input::Cancel => {
                self.stage = Stage::Idle;
                self.cursor = None;
                Ok(())
            }
        }
    }

    fn place(
        &mut self,
        c: DVec3,
        start: DVec3,
        sweep: f64,
        target: &mut Target,
    ) -> Result<(), ToolError> {
        let pts = arc(
            c,
            self.plane.normal(),
            start - c,
            (start - c).length(),
            0.0,
            sweep,
            self.segments,
        );
        let segs: Vec<_> = pts.windows(2).map(|w| (w[0], w[1])).collect();
        draw_segments(target, &segs, "Arc")?;
        self.stage = Stage::Idle;
        self.cursor = None;
        Ok(())
    }
}
