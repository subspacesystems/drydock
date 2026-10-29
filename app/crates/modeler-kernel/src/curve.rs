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

use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};

use glam::DVec3;

use crate::geom::LINEAR_TOL;
use crate::topology::{Context, CurveId, EdgeId, Edit, Record, VertexId};

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
        let min = if self.is_closed() { 3 } else { 1 };
        let sweep = self.sweep();
        if !self.radius.is_finite() || self.radius <= 0.0 || !sweep.is_finite() || sweep <= 0.0 {
            return min;
        }
        let tolerance = if tolerance.is_finite() && tolerance >= MIN_CHORD_TOL {
            tolerance
        } else {
            MIN_CHORD_TOL
        };
        let sagitta = |n: usize| 2.0 * self.radius * (sweep / (4.0 * n as f64)).sin().powi(2);
        // sagitta = r (1 - cos(sweep / (2n))); solve for the first guess with acos.
        let cos = 1.0 - tolerance / self.radius;
        let guess = if cos <= -1.0 {
            min as f64
        } else {
            let angle = cos.clamp(-1.0, 1.0).acos();
            if angle > 0.0 && angle.is_finite() {
                (sweep / (2.0 * angle)).ceil()
            } else {
                MAX_CURVE_SEGMENTS as f64
            }
        };
        let mut n = if guess.is_finite() {
            (guess.max(min as f64).min(MAX_CURVE_SEGMENTS as f64)) as usize
        } else {
            MAX_CURVE_SEGMENTS
        };
        n = n.clamp(min, MAX_CURVE_SEGMENTS);
        while n > min && sagitta(n - 1) <= tolerance {
            n -= 1;
        }
        while n < MAX_CURVE_SEGMENTS && sagitta(n) > tolerance {
            n += 1;
        }
        n
    }

    /// `segments_for(tolerance) + 1` points evenly spaced in angle from `point_at(start)` to
    /// `point_at(end)`; a closed curve's last point is a copy of its first.
    pub fn points(&self, tolerance: f64) -> Vec<DVec3> {
        let n = self.segments_for(tolerance);
        let mut points = Vec::with_capacity(n + 1);
        for k in 0..=n {
            let angle = self.start + self.sweep() * k as f64 / n as f64;
            points.push(self.point_at(angle));
        }
        if self.is_closed() {
            points[n] = points[0];
        } else {
            points[n] = self.point_at(self.end);
        }
        points
    }
}

fn other_end(edge: &crate::topology::Edge, vertex: VertexId) -> Option<VertexId> {
    if edge.ends[0] == vertex {
        Some(edge.ends[1])
    } else if edge.ends[1] == vertex {
        Some(edge.ends[0])
    } else {
        None
    }
}

/// The edges of one step `a` to `b`, in order, walking a straight run when the chord has
/// been split by other geometry. `None` when the step is broken.
fn step_edges(ctx: &Context, a: VertexId, b: VertexId) -> Option<Vec<EdgeId>> {
    let pa = ctx.vertex(a)?.position;
    let pb = ctx.vertex(b)?.position;
    if !pa.is_finite() || !pb.is_finite() {
        return None;
    }
    let along = pb - pa;
    let length = along.length();
    if length == 0.0 {
        return None;
    }
    let dir = along / length;
    let mut current = a;
    let mut current_d = 0.0;
    let mut edges = Vec::new();
    let limit = ctx.vertex_count() + 2;
    for _ in 0..limit {
        let at = ctx.edges_of_vertex(current);
        // The edge that reaches the step's end wins.
        for &edge in &at {
            if let Some(data) = ctx.edge(edge)
                && other_end(data, current) == Some(b)
            {
                edges.push(edge);
                return Some(edges);
            }
        }
        // Otherwise continue along the chord to the nearest vertex beyond `current`.
        let mut best: Option<(f64, EdgeId, VertexId)> = None;
        for &edge in &at {
            let Some(data) = ctx.edge(edge) else {
                continue;
            };
            let Some(next) = other_end(data, current) else {
                continue;
            };
            if next == b {
                continue;
            }
            let Some(vertex) = ctx.vertex(next) else {
                continue;
            };
            let p = vertex.position;
            if crate::geom::point_segment_distance(p, pa, pb) > LINEAR_TOL {
                continue;
            }
            let d = (p - pa).dot(dir);
            if d <= current_d {
                continue;
            }
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, edge, next));
            }
        }
        let (d, edge, next) = best?;
        edges.push(edge);
        current = next;
        current_d = d;
    }
    None
}

fn angle_at(center: DVec3, x_axis: DVec3, y_axis: DVec3, p: DVec3) -> f64 {
    let d = p - center;
    d.dot(y_axis).atan2(d.dot(x_axis))
}

/// True when the curve's numbers are sound and its vertices still lie on it, in order, joined
/// step by step by straight runs of edges (spec: "When a curve fits").
pub fn fits(ctx: &Context, curve: &Curve) -> bool {
    if !curve.center.is_finite()
        || !curve.normal.is_finite()
        || !curve.x_axis.is_finite()
        || !curve.radius.is_finite()
        || !curve.start.is_finite()
        || !curve.end.is_finite()
    {
        return false;
    }
    if curve.radius <= LINEAR_TOL {
        return false;
    }
    if (curve.normal.length() - 1.0).abs() > 1e-9
        || (curve.x_axis.length() - 1.0).abs() > 1e-9
        || curve.normal.dot(curve.x_axis).abs() > 1e-9
    {
        return false;
    }

    let closed = curve.is_closed();
    let distinct = if closed {
        &curve.vertices[..curve.vertices.len() - 1]
    } else {
        &curve.vertices[..]
    };
    if distinct.len() < 3 {
        return false;
    }
    let mut seen = BTreeSet::new();
    for id in distinct {
        if !seen.insert(*id) {
            return false;
        }
    }

    let sweep = curve.sweep();
    if closed {
        if (sweep - TAU).abs() > 1e-9 {
            return false;
        }
    } else if !(sweep > 0.0 && sweep < TAU) {
        return false;
    }

    let mut points = Vec::with_capacity(curve.vertices.len());
    for id in &curve.vertices {
        let Some(vertex) = ctx.vertex(*id) else {
            return false;
        };
        let p = vertex.position;
        let d = p - curve.center;
        let h = d.dot(curve.normal);
        let rho = (d - curve.normal * h).length();
        if h.hypot(rho - curve.radius) > LINEAR_TOL {
            return false;
        }
        points.push(p);
    }

    let first = points[0];
    let last = *points.last().expect("curve has vertices");
    if first.distance(curve.point_at(curve.start)) > LINEAR_TOL
        || last.distance(curve.point_at(curve.end)) > LINEAR_TOL
    {
        return false;
    }

    let y_axis = curve.y_axis();
    let mut turns = 0.0;
    for pair in points.windows(2) {
        let a = angle_at(curve.center, curve.x_axis, y_axis, pair[0]);
        let b = angle_at(curve.center, curve.x_axis, y_axis, pair[1]);
        let turn = (b - a).rem_euclid(TAU);
        if !(turn > 0.0 && turn < PI) {
            return false;
        }
        turns += turn;
    }
    if (turns - sweep).abs() > PI {
        return false;
    }

    for pair in curve.vertices.windows(2) {
        if step_edges(ctx, pair[0], pair[1]).is_none() {
            return false;
        }
    }
    true
}

/// The circle through the chain `vertices` (a closed chain repeats its first vertex), with
/// angle `start` at the first vertex; `None` when they don't make a curve that `fits`
/// (spec: "Fitting a curve").
pub fn fit(ctx: &Context, vertices: &[VertexId], start: f64) -> Option<Curve> {
    if !start.is_finite() || vertices.len() < 2 {
        return None;
    }
    let closed = vertices.first() == vertices.last();
    let m = if closed {
        vertices.len() - 1
    } else {
        vertices.len()
    };
    if m < 3 {
        return None;
    }

    let mut points = Vec::with_capacity(m);
    for id in &vertices[..m] {
        points.push(ctx.vertex(*id)?.position);
    }

    let (ia, ib, ic) = if closed {
        (0, m / 3, 2 * m / 3)
    } else {
        (0, (m - 1) / 2, m - 1)
    };
    let a = points[ia];
    let b = points[ib];
    let c = points[ic];
    let u = b - a;
    let v = c - a;
    let w = u.cross(v);
    let w2 = w.length_squared();
    if !w2.is_finite() || w2 == 0.0 {
        return None;
    }
    let center =
        a + (u.length_squared() * v.cross(w) + v.length_squared() * w.cross(u)) / (2.0 * w2);
    if !center.is_finite() {
        return None;
    }
    let mut normal = w / w2.sqrt();
    if !normal.is_finite() {
        return None;
    }

    let steps = if closed { m } else { m - 1 };
    let mut area = DVec3::ZERO;
    for i in 0..steps {
        let p = points[i] - center;
        let q = points[(i + 1) % m] - center;
        area += p.cross(q);
    }
    if area.dot(normal) < 0.0 {
        normal = -normal;
    }

    let radius = (points[0] - center).length();
    if !radius.is_finite() || radius == 0.0 {
        return None;
    }
    let d = points[0] - center;
    let d = d - normal * d.dot(normal);
    let d_len = d.length();
    if !d_len.is_finite() || d_len == 0.0 {
        return None;
    }
    let d = d / d_len;
    let x_axis = d * start.cos() - normal.cross(d) * start.sin();
    let x_len = x_axis.length();
    if !x_len.is_finite() || x_len == 0.0 {
        return None;
    }
    let x_axis = x_axis / x_len;

    let end = if closed {
        start + TAU
    } else {
        let y_axis = normal.cross(x_axis);
        let mut turns = 0.0;
        for pair in points.windows(2) {
            let a = angle_at(center, x_axis, y_axis, pair[0]);
            let b = angle_at(center, x_axis, y_axis, pair[1]);
            turns += (b - a).rem_euclid(TAU);
        }
        start + turns
    };

    let curve = Curve {
        center,
        normal,
        x_axis,
        radius,
        start,
        end,
        vertices: vertices.to_vec(),
    };
    fits(ctx, &curve).then_some(curve)
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
    let mut vertices = Vec::with_capacity(points.len() + 1);
    for p in points {
        vertices.push(ctx.find_vertex(*p, LINEAR_TOL)?);
    }
    if closed && vertices.first() != vertices.last() {
        vertices.push(*vertices.first()?);
    }
    let curve = fit(ctx, &vertices, 0.0)?;
    for pair in curve.vertices.windows(2) {
        let edges = step_edges(ctx, pair[0], pair[1])?;
        for edge in edges {
            if curve_of_edge(ctx, edge).is_some() {
                return None;
            }
        }
    }
    Some(ctx.insert_curve(curve, log))
}

/// The edges of the curve's steps, in chain order (a step split by other geometry gives
/// several edges). Empty for an unknown id or a curve whose steps are broken.
pub fn curve_edges(ctx: &Context, id: CurveId) -> Vec<EdgeId> {
    let Some(curve) = ctx.curve(id) else {
        return Vec::new();
    };
    let mut edges = Vec::new();
    for pair in curve.vertices.windows(2) {
        match step_edges(ctx, pair[0], pair[1]) {
            Some(mut step) => edges.append(&mut step),
            None => return Vec::new(),
        }
    }
    edges
}

/// The lowest-id curve whose `curve_edges` contain `edge`.
pub fn curve_of_edge(ctx: &Context, edge: EdgeId) -> Option<CurveId> {
    ctx.curves()
        .find(|(id, _)| curve_edges(ctx, *id).contains(&edge))
        .map(|(id, _)| id)
}

fn add_with_id(ctx: &mut Context, id: CurveId, curve: Curve, log: &mut Edit) {
    let record = Record::AddCurve(id, curve);
    if ctx.apply(&record).is_ok() {
        log.records.push(record);
    }
}

/// Bring every curve back in line with the geometry after an edit whose records are in
/// `log`: keep it, re-fit it, split it into runs or drop it, appending the curve records to
/// `log` (spec: "Keeping curves through edits"). Called by `Model::edit_geometry`.
pub fn reconcile(ctx: &mut Context, log: &mut Edit) {
    let geometry_changed = log.records.iter().any(|record| {
        matches!(
            record,
            Record::MoveVertex { .. } | Record::RemoveVertex(..) | Record::RemoveEdge(..)
        )
    });
    if !geometry_changed {
        return;
    }

    let ids: Vec<CurveId> = ctx.curves().map(|(id, _)| id).collect();
    for id in ids {
        let Some(old) = ctx.curve(id).cloned() else {
            continue;
        };
        if fits(ctx, &old) {
            continue;
        }

        let steps: Vec<Option<Vec<EdgeId>>> = old
            .vertices
            .windows(2)
            .map(|pair| step_edges(ctx, pair[0], pair[1]))
            .collect();
        let all_intact = steps.iter().all(Option::is_some);
        if all_intact {
            match fit(ctx, &old.vertices, old.start) {
                Some(new) => {
                    let _ = ctx.delete_curve(id, log);
                    add_with_id(ctx, id, new, log);
                }
                None => {
                    let _ = ctx.delete_curve(id, log);
                }
            }
            continue;
        }

        let _ = ctx.delete_curve(id, log);
        let step_count = steps.len();
        let intact: Vec<bool> = steps.iter().map(Option::is_some).collect();
        let closed = old.is_closed();

        let mut runs: Vec<(usize, usize)> = Vec::new(); // (first step, step count)
        if closed {
            let Some(first_broken) = intact.iter().position(|good| !good) else {
                continue;
            };
            let mut k = 1;
            while k <= step_count {
                let index = (first_broken + k) % step_count;
                if intact[index] && !intact[(index + step_count - 1) % step_count] {
                    let mut length = 0;
                    while length < step_count && intact[(index + length) % step_count] {
                        length += 1;
                    }
                    runs.push((index, length));
                    k += length;
                } else {
                    k += 1;
                }
            }
        } else {
            let mut i = 0;
            while i < step_count {
                if intact[i] {
                    let start = i;
                    while i < step_count && intact[i] {
                        i += 1;
                    }
                    runs.push((start, i - start));
                } else {
                    i += 1;
                }
            }
        }
        runs.sort_by_key(|(start, _)| *start);

        let y_axis = old.y_axis();
        let mut first_added = true;
        for (start, length) in runs {
            if length < 2 {
                continue;
            }
            let run: Vec<VertexId> = if closed {
                let n = old.vertices.len() - 1;
                (0..=length)
                    .map(|k| old.vertices[(start + k) % n])
                    .collect()
            } else {
                old.vertices[start..=start + length].to_vec()
            };

            let mut turns = 0.0;
            for i in start..start + length {
                let pair = if closed {
                    let n = old.vertices.len() - 1;
                    (old.vertices[i % n], old.vertices[(i + 1) % n])
                } else {
                    (old.vertices[i], old.vertices[i + 1])
                };
                let (Some(a), Some(b)) = (ctx.vertex(pair.0), ctx.vertex(pair.1)) else {
                    continue;
                };
                let a = angle_at(old.center, old.x_axis, y_axis, a.position);
                let b = angle_at(old.center, old.x_axis, y_axis, b.position);
                turns += (b - a).rem_euclid(TAU);
            }
            let Some(first_point) = ctx.vertex(run[0]) else {
                continue;
            };
            let first_angle = angle_at(old.center, old.x_axis, y_axis, first_point.position);
            let run_start = old.start + (first_angle - old.start).rem_euclid(TAU);
            let run_end = run_start + turns;

            let candidate = Curve {
                center: old.center,
                normal: old.normal,
                x_axis: old.x_axis,
                radius: old.radius,
                start: run_start,
                end: run_end,
                vertices: run.clone(),
            };
            let curve = if fits(ctx, &candidate) {
                Some(candidate)
            } else {
                fit(ctx, &run, run_start)
            };
            let Some(curve) = curve else {
                continue;
            };
            if first_added {
                add_with_id(ctx, id, curve, log);
                first_added = false;
            } else {
                ctx.insert_curve(curve, log);
            }
        }
    }
}
