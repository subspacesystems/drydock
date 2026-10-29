// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves: arcs and circles that stay arcs and circles. A curve is drawn as a chain of
//! straight edges, but keeps the circle it came from (center, plane normal, radius, start and
//! end angles), so it can be measured, moved and later re-segmented for export at a chosen
//! chord tolerance.
//!
//! Behaviour: `docs/specs/m7-curves.md`. Curves live in the `Context` (`Context::curves`,
//! `Record::AddCurve` / `RemoveCurve`); `Model::edit_geometry` runs `reconcile` after every
//! geometry edit, so moves, rotations, scales, sticky splits and erasing keep each curve
//! consistent with its vertices, or turn it back into plain edges.

use glam::DVec3;

use crate::topology::{Context, CurveId, EdgeId, Edit, VertexId};

/// Tolerances below this (or not finite) are read as this, in meters (1 µm).
pub const MIN_CHORD_TOL: f64 = 1e-6;

/// `Curve::segments_for` never returns more than this.
pub const MAX_CURVE_SEGMENTS: usize = 65_536;

/// An arc or circle and the vertices it was drawn through.
///
/// The circle: `point_at(a) = center + radius · (cos a · x_axis + sin a · y_axis())`, angles in
/// radians counter-clockwise about `normal`. The chain runs from angle `start` to angle `end`
/// (`start < end`), counter-clockwise. `vertices` is the chain in order; a closed curve (a
/// circle) repeats its first vertex at the end and has `end = start + 2π`.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub center: DVec3,
    /// Unit normal of the circle's plane.
    pub normal: DVec3,
    /// Unit direction of angle 0, perpendicular to `normal`.
    pub x_axis: DVec3,
    pub radius: f64,
    pub start: f64,
    pub end: f64,
    pub vertices: Vec<VertexId>,
}

impl Curve {
    /// True when the chain ends at its first vertex.
    pub fn is_closed(&self) -> bool {
        self.vertices.len() > 1 && self.vertices.first() == self.vertices.last()
    }

    /// `end - start`.
    pub fn sweep(&self) -> f64 {
        self.end - self.start
    }

    /// `normal × x_axis`.
    pub fn y_axis(&self) -> DVec3 {
        self.normal.cross(self.x_axis)
    }

    /// The point of the circle at angle `angle`.
    pub fn point_at(&self, angle: f64) -> DVec3 {
        self.center + (self.x_axis * angle.cos() + self.y_axis() * angle.sin()) * self.radius
    }

    /// The smallest segment count, at least 3 for a closed curve and 1 for an open one, whose
    /// chords stay within `tolerance` of the arc (spec: "Segments for export").
    pub fn segments_for(&self, tolerance: f64) -> usize {
        let _ = tolerance;
        unimplemented!()
    }

    /// `segments_for(tolerance) + 1` points evenly spaced in angle from `point_at(start)` to
    /// `point_at(end)`; a closed curve's last point is a copy of its first.
    pub fn points(&self, tolerance: f64) -> Vec<DVec3> {
        let _ = tolerance;
        unimplemented!()
    }
}

/// True when the curve's numbers are sound and its vertices still lie on it, in order, joined
/// step by step by straight runs of edges (spec: "When a curve fits").
pub fn fits(ctx: &Context, curve: &Curve) -> bool {
    let _ = (ctx, curve);
    unimplemented!()
}

/// The circle through the chain `vertices` (a closed chain repeats its first vertex), with
/// angle `start` at the first vertex; `None` when they don't make a curve that `fits`
/// (spec: "Fitting a curve").
pub fn fit(ctx: &Context, vertices: &[VertexId], start: f64) -> Option<Curve> {
    let _ = (ctx, vertices, start);
    unimplemented!()
}

/// Make a curve through the vertices at `points` (the polyline a drawing tool just added),
/// closed when `closed` or when the first and last points are the same vertex. `None`, and
/// nothing changes, when it can't (spec: "Adding a curve").
pub fn add_curve(
    ctx: &mut Context,
    points: &[DVec3],
    closed: bool,
    log: &mut Edit,
) -> Option<CurveId> {
    let _ = (ctx, points, closed, log);
    unimplemented!()
}

/// The edges of the curve's steps, in chain order (a step split by other geometry gives
/// several edges). Empty for an unknown id or a curve whose steps are broken.
pub fn curve_edges(ctx: &Context, id: CurveId) -> Vec<EdgeId> {
    let _ = (ctx, id);
    unimplemented!()
}

/// The lowest-id curve whose `curve_edges` contain `edge`.
pub fn curve_of_edge(ctx: &Context, edge: EdgeId) -> Option<CurveId> {
    let _ = (ctx, edge);
    unimplemented!()
}

/// Bring every curve back in line with the geometry after an edit whose records are in
/// `log`: keep it, re-fit it, split it into runs or drop it, appending the curve records to
/// `log` (spec: "Keeping curves through edits"). Called by `Model::edit_geometry`.
pub fn reconcile(ctx: &mut Context, log: &mut Edit) {
    let _ = (ctx, log);
    unimplemented!()
}
