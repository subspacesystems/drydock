// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Three-point arc tool: click the arc's start, click its end, click a point the arc passes
//! through (the bulge). Typed: the chord length, then the bulge height.
//! Typing `8s` sets the number of segments.
//!
//! Behaviour: `docs/specs/m2-arc-3pt.md`.

use glam::DVec3;

use crate::geom::LINEAR_TOL;
use crate::sticky::StickyError;
use crate::units::parse_length;

use super::circle::segment_count;
use super::plane::DrawPlane;
use super::{Input, Target, ToolError, draw_segments};

const TAU: f64 = std::f64::consts::TAU;

#[derive(Clone, Debug)]
enum Stage {
    Idle,
    Start(DVec3),
    End { a: DVec3, b: DVec3 },
}

#[derive(Clone, Debug)]
pub struct Arc3Tool {
    plane: DrawPlane,
    stage: Stage,
    cursor: Option<DVec3>,
    segments: usize,
}

#[derive(Clone, Debug)]
struct ArcFrame {
    center: DVec3,
    e1: DVec3,
    e2: DVec3,
    radius: f64,
    start: f64,
    sweep: f64,
}

fn wrap_tau(angle: f64) -> f64 {
    angle.rem_euclid(TAU)
}

/// Circumcircle of `a`, `b` and `through` in the plane through `a` with normal `normal`.
fn arc_frame(normal: DVec3, a: DVec3, b: DVec3, through: DVec3) -> Option<ArcFrame> {
    if !a.is_finite() || !b.is_finite() || !through.is_finite() || !normal.is_finite() {
        return None;
    }
    let n_len = normal.length();
    if !n_len.is_finite() || n_len == 0.0 {
        return None;
    }
    let n = normal / n_len;
    let pb = b - n * (b - a).dot(n);
    let pt = through - n * (through - a).dot(n);
    if !pb.is_finite() || !pt.is_finite() {
        return None;
    }
    let p = pb - a;
    let q = pt - a;
    if p.length() <= LINEAR_TOL || q.length() <= LINEAR_TOL {
        return None;
    }
    if pt.distance(pb) <= LINEAR_TOL {
        return None;
    }
    let e1 = p / p.length();
    let e2 = n.cross(e1);
    let qx = q.dot(e1);
    let qy = q.dot(e2);
    if qy.abs() <= LINEAR_TOL {
        return None;
    }
    let cx = p.length() * 0.5;
    let cy = (q.length_squared() - p.length() * qx) / (2.0 * qy);
    let center = a + e1 * cx + e2 * cy;
    let radius = cx.hypot(cy);
    if !center.is_finite() || !radius.is_finite() || radius <= LINEAR_TOL {
        return None;
    }
    let start = (-cy).atan2(-cx);
    let end = (-cy).atan2(p.length() - cx);
    let through_angle = (qy - cy).atan2(qx - cx);
    let ccw = wrap_tau(end - start);
    let offset = wrap_tau(through_angle - start);
    let sweep = if offset <= ccw { ccw } else { ccw - TAU };
    if !sweep.is_finite() || sweep.abs() < 1e-9 {
        return None;
    }
    Some(ArcFrame {
        center,
        e1,
        e2,
        radius,
        start,
        sweep,
    })
}

/// The `segments + 1` points of the arc that runs from `a` to `b` and passes through
/// `through`, all measured in the plane through `a` with normal `normal`. The first point is
/// exactly `a` and the last exactly `b`. `None` when the three points are (nearly) on a line,
/// when `a` and `b` are within `LINEAR_TOL`, when `segments` is 0, or on non-finite input.
pub fn three_point_arc(
    normal: DVec3,
    a: DVec3,
    b: DVec3,
    through: DVec3,
    segments: usize,
) -> Option<Vec<DVec3>> {
    if segments == 0 {
        return None;
    }
    let frame = arc_frame(normal, a, b, through)?;
    let n = normal / normal.length();
    let end = b - n * (b - a).dot(n);
    let mut points: Vec<DVec3> = (0..=segments)
        .map(|k| {
            let t = frame.start + frame.sweep * k as f64 / segments as f64;
            frame.center + (frame.e1 * t.cos() + frame.e2 * t.sin()) * frame.radius
        })
        .collect();
    points[0] = a;
    points[segments] = end;
    Some(points)
}

impl Arc3Tool {
    /// Idle, 12 segments.
    pub fn new(plane: DrawPlane) -> Self {
        Self {
            plane,
            stage: Stage::Idle,
            cursor: None,
            segments: 12,
        }
    }

    pub fn plane(&self) -> DrawPlane {
        self.plane
    }

    /// Use `plane` for the next shape. Ignored while a shape is in progress.
    pub fn set_plane(&mut self, plane: DrawPlane) {
        if matches!(self.stage, Stage::Idle) {
            self.plane = plane;
        }
    }

    /// Points placed so far: 0 idle, 1 the start, 2 the start and the end.
    pub fn placed(&self) -> usize {
        match self.stage {
            Stage::Idle => 0,
            Stage::Start(_) => 1,
            Stage::End { .. } => 2,
        }
    }

    /// The last point placed. Use it as `Probe::anchor`.
    pub fn anchor(&self) -> Option<DVec3> {
        match self.stage {
            Stage::Idle => None,
            Stage::Start(a) => Some(a),
            Stage::End { b, .. } => Some(b),
        }
    }

    pub fn segments(&self) -> usize {
        self.segments
    }

    fn radial(&self, a: DVec3, p: DVec3) -> Option<DVec3> {
        let q = self.plane.project(a, p) - a;
        (q.length() > LINEAR_TOL).then_some(q)
    }

    /// Chord length: start to cursor (one point placed) or start to end (two placed).
    pub fn chord(&self) -> Option<f64> {
        match self.stage {
            Stage::Idle => None,
            Stage::Start(a) => Some(self.radial(a, self.cursor?)?.length()),
            Stage::End { a, b } => Some(b.distance(a)),
        }
    }

    /// Radius of the arc `preview` shows.
    pub fn radius(&self) -> Option<f64> {
        let Stage::End { a, b } = self.stage else {
            return None;
        };
        let cursor = self.cursor?;
        let frame = arc_frame(self.plane.normal(), a, b, cursor)?;
        Some(frame.radius)
    }

    /// One point placed: the chord `[start, cursor]`. Two placed: the arc the next click
    /// would draw. Otherwise `None`.
    pub fn preview(&self) -> Option<Vec<DVec3>> {
        match self.stage {
            Stage::Idle => None,
            Stage::Start(a) => {
                let q = self.radial(a, self.cursor?)?;
                Some(vec![a, a + q])
            }
            Stage::End { a, b } => {
                let cursor = self.cursor?;
                three_point_arc(self.plane.normal(), a, b, cursor, self.segments)
            }
        }
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
                        self.stage = Stage::Start(p);
                        self.cursor = None;
                        Ok(())
                    }
                    Stage::Start(a) => {
                        let Some(q) = self.radial(a, p) else {
                            return Ok(());
                        };
                        self.stage = Stage::End { a, b: a + q };
                        self.cursor = None;
                        Ok(())
                    }
                    Stage::End { a, b } => {
                        let Some(points) =
                            three_point_arc(self.plane.normal(), a, b, p, self.segments)
                        else {
                            return Ok(());
                        };
                        self.place(&points, target)
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
                    Stage::Start(a) => {
                        let r = parse_length(&text, target.unit)
                            .map_err(ToolError::Parse)?
                            .abs();
                        if r <= LINEAR_TOL {
                            return Err(ToolError::NoDirection);
                        }
                        let x = match self.cursor {
                            Some(p) => match self.radial(a, p) {
                                Some(q) => q / q.length(),
                                None => self.plane.u,
                            },
                            None => self.plane.u,
                        };
                        self.stage = Stage::End { a, b: a + x * r };
                        self.cursor = None;
                        Ok(())
                    }
                    Stage::End { a, b } => {
                        let h = parse_length(&text, target.unit)
                            .map_err(ToolError::Parse)?
                            .abs();
                        if h <= LINEAR_TOL {
                            return Err(ToolError::NoDirection);
                        }
                        let mid = (a + b) * 0.5;
                        let left = self.plane.normal().cross(b - a);
                        if !left.is_finite() || left.length() == 0.0 {
                            return Err(ToolError::NoDirection);
                        }
                        let left = left / left.length();
                        let side = match self.cursor {
                            Some(c) => {
                                let c = self.plane.project(a, c);
                                if (c - mid).dot(left) < 0.0 { -1.0 } else { 1.0 }
                            }
                            None => 1.0,
                        };
                        let apex = mid + left * (side * h);
                        let Some(points) =
                            three_point_arc(self.plane.normal(), a, b, apex, self.segments)
                        else {
                            return Err(ToolError::NoDirection);
                        };
                        self.place(&points, target)
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

    fn place(&mut self, points: &[DVec3], target: &mut Target) -> Result<(), ToolError> {
        let segments: Vec<_> = points.windows(2).map(|w| (w[0], w[1])).collect();
        draw_segments(target, &segments, "Arc")?;
        self.stage = Stage::Idle;
        self.cursor = None;
        Ok(())
    }
}
