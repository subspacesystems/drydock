// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rectangle tool: click one corner, click the opposite one (or type `width,height`).

use glam::DVec3;

use crate::geom::LINEAR_TOL;
use crate::shapes::rectangle;
use crate::sticky::StickyError;
use crate::units::{ParseError, parse_lengths};

use super::plane::DrawPlane;
use super::{Input, Target, ToolError, draw_segments};

#[derive(Clone, Debug)]
pub struct RectTool {
    plane: DrawPlane,
    start: Option<DVec3>,
    cursor: Option<DVec3>,
}

fn finite(p: DVec3) -> bool {
    p.is_finite()
}

impl RectTool {
    pub fn new(plane: DrawPlane) -> Self {
        Self {
            plane,
            start: None,
            cursor: None,
        }
    }

    pub fn plane(&self) -> DrawPlane {
        self.plane
    }

    /// Use `plane` for the next shape. Ignored while a shape is in progress.
    pub fn set_plane(&mut self, plane: DrawPlane) {
        if self.start.is_none() {
            self.plane = plane;
        }
    }

    /// The first corner while drawing. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        self.start
    }

    /// The four corners the next click would draw.
    pub fn preview(&self) -> Option<[DVec3; 4]> {
        let a = self.start?;
        let c = self.cursor?;
        rectangle(a, c, self.plane.u, self.plane.v)
    }

    /// `(width, height)` of `preview`, along `u` and `v`, for the measurement box.
    pub fn size(&self) -> Option<(f64, f64)> {
        self.preview()?;
        let d = self.cursor? - self.start?;
        Some((d.dot(self.plane.u).abs(), d.dot(self.plane.v).abs()))
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
                    return Err(ToolError::Geometry(StickyError::NotFinite));
                }
                let Some(a) = self.start else {
                    self.start = Some(p);
                    self.cursor = None;
                    return Ok(());
                };
                match rectangle(a, p, self.plane.u, self.plane.v) {
                    Some(c) => self.place(c, target),
                    None => Ok(()),
                }
            }
            Input::Typed(text) => {
                let Some(a) = self.start else {
                    return Err(ToolError::NotStarted);
                };
                let vals = parse_lengths(&text, target.unit).map_err(ToolError::Parse)?;
                let (cu, cv) = match self.cursor {
                    Some(c) => {
                        let d = c - a;
                        (d.dot(self.plane.u), d.dot(self.plane.v))
                    }
                    None => (0.0, 0.0),
                };
                let su = if cu < 0.0 { -1.0 } else { 1.0 };
                let sv = if cv < 0.0 { -1.0 } else { 1.0 };
                let (w, h) = match vals.as_slice() {
                    [w] => {
                        if cv.abs() <= LINEAR_TOL {
                            return Err(ToolError::NoDirection);
                        }
                        (*w, cv.abs())
                    }
                    [w, h] => (*w, *h),
                    _ => return Err(ToolError::Parse(ParseError::Invalid)),
                };
                let b = a + self.plane.u * (w * su) + self.plane.v * (h * sv);
                match rectangle(a, b, self.plane.u, self.plane.v) {
                    Some(c) => self.place(c, target),
                    None => Err(ToolError::NoDirection),
                }
            }
            Input::Cancel => {
                self.start = None;
                self.cursor = None;
                Ok(())
            }
        }
    }

    fn place(&mut self, c: [DVec3; 4], target: &mut Target) -> Result<(), ToolError> {
        let segs: Vec<_> = (0..4).map(|i| (c[i], c[(i + 1) % 4])).collect();
        draw_segments(target, &segs, "Rectangle")?;
        self.start = None;
        self.cursor = None;
        Ok(())
    }
}
