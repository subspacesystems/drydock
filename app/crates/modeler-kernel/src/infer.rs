// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference: where the cursor's point lands while a drawing tool runs, what it snapped to,
//! and what entity is under the cursor.
//!
//! Behaviour: `docs/specs/m2-inference.md`. Works on one context, in its own coordinates;
//! instances are not entered.

use std::collections::BTreeSet;

use crate::geom::{self, Containment};
use crate::select::Entity;
use crate::topology::{Context, EdgeId, FaceId, VertexId};
use glam::{DVec2, DVec3};

const TOL: f64 = geom::LINEAR_TOL;

/// The cursor, the view and what the running tool knows. Points are in the context's
/// coordinates.
pub struct Probe<'a> {
    /// Cursor, screen pixels.
    pub cursor: DVec2,
    /// Start of the ray through the cursor (the eye for a perspective view).
    pub origin: DVec3,
    /// Direction of the ray through the cursor. Need not be unit length.
    pub dir: DVec3,
    /// True for a parallel view: every pixel's ray has direction `dir`.
    pub parallel: bool,
    /// Maps a point to screen pixels; `None` when it can't be seen.
    pub project: &'a dyn Fn(DVec3) -> Option<DVec2>,
    /// Snap radius, pixels.
    pub radius: f64,
    /// The tool's last placed point, for axis inference and the direction lock.
    pub anchor: Option<DVec3>,
    /// Axis directions in red, green, blue order (unit length, orthogonal).
    pub axes: [DVec3; 3],
    /// Extra points that snap as centers (arc and circle centers known to the tool).
    pub centers: &'a [DVec3],
    /// Direction lock: the result stays on the line through `anchor` along this direction.
    pub lock: Option<DVec3>,
}

/// What the inferred point snapped to, highest priority first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snap {
    Endpoint(VertexId),
    Midpoint(EdgeId),
    /// Index into `Probe::centers`.
    Center(usize),
    OnEdge(EdgeId),
    /// Index into `Probe::axes`.
    OnAxis(usize),
    OnFace(FaceId),
    /// On the ground plane, or the horizontal plane through the anchor.
    Ground,
    /// On the lock line, nothing else snapped.
    Along,
}

/// The point a tool should use, and the hint to draw for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inference {
    pub point: DVec3,
    pub snap: Snap,
    /// Index into `Probe::axes` when `point - anchor` runs along that axis.
    pub axis: Option<usize>,
}

/// The nearest face the ray from `origin` along `dir` passes through, and the distance to
/// it along the unit ray.
pub fn ray_face(ctx: &Context, origin: DVec3, dir: DVec3) -> Option<(FaceId, f64)> {
    let d = unit(dir)?;
    let mut best: Option<(FaceId, f64)> = None;
    for (id, _) in ctx.faces() {
        let Some(t) = face_t(ctx, id, origin, d) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((best_id, best_t)) => t < best_t || (t == best_t && id < best_id),
        };
        if better {
            best = Some((id, t));
        }
    }
    best
}

/// The inferred point for the cursor.
pub fn infer(ctx: &Context, probe: &Probe) -> Option<Inference> {
    let d = valid_probe(probe)?;

    if let (Some(anchor), Some(a)) = (probe.anchor, probe.lock.and_then(unit)) {
        if let Some(hit) = point_categories(ctx, probe, d) {
            let point = anchor + a * ((hit.point - anchor).dot(a));
            return Some(inference(probe, hit.snap, point));
        }
        let point = match line_param(anchor, a, probe.origin, d) {
            Some(s) if d.cross(a).length() >= 1e-9 => anchor + a * s,
            _ => anchor,
        };
        return Some(inference(probe, Snap::Along, point));
    }

    if let Some(hit) = point_categories(ctx, probe, d)
        .or_else(|| on_edge_best(ctx, probe, d))
        .or_else(|| on_axis_best(ctx, probe, d))
    {
        return Some(inference(probe, hit.snap, hit.point));
    }

    if let Some((face, t)) = ray_face(ctx, probe.origin, d) {
        let point = probe.origin + d * t;
        return Some(inference(probe, Snap::OnFace(face), point));
    }

    if let Some(point) = ground_point(probe, d) {
        return Some(inference(probe, Snap::Ground, point));
    }

    None
}

/// The entity under the cursor: a visible edge within the snap radius, else the face the
/// ray hits first.
pub fn pick(ctx: &Context, probe: &Probe) -> Option<Entity> {
    let d = valid_probe(probe)?;
    if let Some(Hit {
        snap: Snap::OnEdge(edge),
        ..
    }) = on_edge_best(ctx, probe, d)
    {
        return Some(Entity::Edge(edge));
    }
    ray_face(ctx, probe.origin, d).map(|(face, _)| Entity::Face(face))
}

// -- input -------------------------------------------------------------------------

fn unit(v: DVec3) -> Option<DVec3> {
    if !v.is_finite() {
        return None;
    }
    let len = v.length();
    if len > 0.0 && len.is_finite() {
        Some(v / len)
    } else {
        None
    }
}

fn valid_probe(probe: &Probe) -> Option<DVec3> {
    if !probe.origin.is_finite() || !probe.cursor.is_finite() {
        return None;
    }
    unit(probe.dir)
}

fn depth(probe: &Probe, p: DVec3, d: DVec3) -> f64 {
    (p - probe.origin).dot(d)
}

fn screen_distance(probe: &Probe, p: DVec3) -> Option<f64> {
    let s = (probe.project)(p)?;
    if !s.is_finite() {
        return None;
    }
    Some(s.distance(probe.cursor))
}

fn in_range(probe: &Probe, p: DVec3) -> Option<f64> {
    let d = screen_distance(probe, p)?;
    if d <= probe.radius { Some(d) } else { None }
}

// -- ray/face ----------------------------------------------------------------------

/// Distance along the unit ray to `face`, when the ray hits its interior or boundary.
fn face_t(ctx: &Context, face_id: FaceId, origin: DVec3, d: DVec3) -> Option<f64> {
    let plane = ctx.face_plane(face_id)?;
    let denom = d.dot(plane.normal);
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = -plane.signed_distance(origin) / denom;
    if !t.is_finite() || t <= 1e-9 {
        return None;
    }

    let face = ctx.face(face_id)?;
    let hit = origin + d * t;
    let hit2 = geom::to_plane_2d(&[hit], plane.normal)[0];

    let outer = geom::to_plane_2d(&ctx.loop_points(&face.outer), plane.normal);
    if !matches!(
        geom::point_in_polygon_2d(hit2, &outer, 1e-9),
        Containment::Inside | Containment::Boundary
    ) {
        return None;
    }

    for hole in &face.inner {
        let points = geom::to_plane_2d(&ctx.loop_points(hole), plane.normal);
        if geom::point_in_polygon_2d(hit2, &points, 1e-9) == Containment::Inside {
            return None;
        }
    }

    Some(t)
}

// -- hidden points -----------------------------------------------------------------

fn faces_touching_vertex(ctx: &Context, vertex: VertexId) -> BTreeSet<FaceId> {
    let mut faces = BTreeSet::new();
    for edge in ctx.edges_of_vertex(vertex) {
        for face in ctx.faces_of_edge(edge) {
            faces.insert(face);
        }
    }
    faces
}

fn faces_touching_edge(ctx: &Context, edge: EdgeId) -> BTreeSet<FaceId> {
    ctx.faces_of_edge(edge).into_iter().collect()
}

fn is_hidden(
    ctx: &Context,
    probe: &Probe,
    d: DVec3,
    p: DVec3,
    touching: &BTreeSet<FaceId>,
) -> bool {
    let s = if probe.parallel {
        p - d * depth(probe, p, d)
    } else {
        probe.origin
    };
    let diff = p - s;
    let length = diff.length();
    if length <= TOL {
        return false;
    }
    let Some(view) = unit(diff) else {
        return false;
    };
    let limit = length - TOL;
    for (id, _) in ctx.faces() {
        if touching.contains(&id) {
            continue;
        }
        if let Some(t) = face_t(ctx, id, s, view)
            && t < limit
        {
            return true;
        }
    }
    false
}

// -- candidates --------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Hit {
    snap: Snap,
    point: DVec3,
    screen: f64,
    depth: f64,
    id: u64,
}

fn better(a: &Hit, b: &Hit) -> bool {
    if a.screen != b.screen {
        a.screen < b.screen
    } else if a.depth != b.depth {
        a.depth < b.depth
    } else {
        a.id < b.id
    }
}

fn consider(best: &mut Option<Hit>, hit: Hit) {
    if best.as_ref().is_none_or(|b| better(&hit, b)) {
        *best = Some(hit);
    }
}

fn endpoint_best(ctx: &Context, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    for (id, vertex) in ctx.vertices() {
        let p = vertex.position;
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        let touching = faces_touching_vertex(ctx, id);
        if is_hidden(ctx, probe, d, p, &touching) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::Endpoint(id),
                point: p,
                screen,
                depth: dep,
                id: id.0,
            },
        );
    }
    best
}

fn midpoint_best(ctx: &Context, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    for (id, edge) in ctx.edges() {
        let (Some(a), Some(b)) = (
            ctx.vertex(edge.ends[0]).map(|v| v.position),
            ctx.vertex(edge.ends[1]).map(|v| v.position),
        ) else {
            continue;
        };
        let p = (a + b) / 2.0;
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        let touching = faces_touching_edge(ctx, id);
        if is_hidden(ctx, probe, d, p, &touching) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::Midpoint(id),
                point: p,
                screen,
                depth: dep,
                id: id.0,
            },
        );
    }
    best
}

fn center_best(ctx: &Context, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    let empty = BTreeSet::new();
    for (i, &p) in probe.centers.iter().enumerate() {
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        if is_hidden(ctx, probe, d, p, &empty) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::Center(i),
                point: p,
                screen,
                depth: dep,
                id: i as u64,
            },
        );
    }
    best
}

fn on_edge_best(ctx: &Context, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    for (id, edge) in ctx.edges() {
        let (Some(a), Some(b)) = (
            ctx.vertex(edge.ends[0]).map(|v| v.position),
            ctx.vertex(edge.ends[1]).map(|v| v.position),
        ) else {
            continue;
        };
        let Some(p) = segment_point_nearest_ray(a, b, probe.origin, d) else {
            continue;
        };
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        let touching = faces_touching_edge(ctx, id);
        if is_hidden(ctx, probe, d, p, &touching) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::OnEdge(id),
                point: p,
                screen,
                depth: dep,
                id: id.0,
            },
        );
    }
    best
}

fn on_axis_best(ctx: &Context, probe: &Probe, d: DVec3) -> Option<Hit> {
    let anchor = probe.anchor?;
    let mut best = None;
    let empty = BTreeSet::new();
    for (i, &axis) in probe.axes.iter().enumerate() {
        if d.cross(axis).length() < 1e-9 {
            continue;
        }
        let Some(s) = line_param(anchor, axis, probe.origin, d) else {
            continue;
        };
        let p = anchor + axis * s;
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        if is_hidden(ctx, probe, d, p, &empty) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::OnAxis(i),
                point: p,
                screen,
                depth: dep,
                id: i as u64,
            },
        );
    }
    best
}

fn point_categories(ctx: &Context, probe: &Probe, d: DVec3) -> Option<Hit> {
    endpoint_best(ctx, probe, d)
        .or_else(|| midpoint_best(ctx, probe, d))
        .or_else(|| center_best(ctx, probe, d))
}

// -- ground and axis ---------------------------------------------------------------

fn ground_point(probe: &Probe, d: DVec3) -> Option<DVec3> {
    let base = probe.anchor.unwrap_or(DVec3::ZERO);
    let normal = probe.axes[2];
    let denom = d.dot(normal);
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = -normal.dot(probe.origin - base) / denom;
    if t >= 0.0 {
        Some(probe.origin + d * t)
    } else {
        None
    }
}

fn inference(probe: &Probe, snap: Snap, point: DVec3) -> Inference {
    let axis = match snap {
        Snap::OnAxis(i) => Some(i),
        _ => axis_for(probe, point),
    };
    Inference { point, snap, axis }
}

fn axis_for(probe: &Probe, point: DVec3) -> Option<usize> {
    let anchor = probe.anchor?;
    let delta = point - anchor;
    let length = delta.length();
    if !length.is_finite() || length <= TOL {
        return None;
    }
    let u = delta / length;
    for (i, &axis) in probe.axes.iter().enumerate() {
        if u.dot(axis).abs() >= 1.0 - 1e-9 {
            return Some(i);
        }
    }
    None
}

// -- lines and segments ------------------------------------------------------------

/// Parameter `s` on the line `a + s*u` at the point closest to the line `b + t*v`.
fn line_param(a: DVec3, u: DVec3, b: DVec3, v: DVec3) -> Option<f64> {
    let w = a - b;
    let uu = u.dot(u);
    let uv = u.dot(v);
    let vv = v.dot(v);
    let uw = u.dot(w);
    let vw = v.dot(w);
    let denom = uu * vv - uv * uv;
    if !denom.is_finite() || denom == 0.0 {
        return None;
    }
    let s = (uv * vw - vv * uw) / denom;
    if s.is_finite() { Some(s) } else { None }
}

fn segment_point_nearest_ray(a: DVec3, b: DVec3, origin: DVec3, d: DVec3) -> Option<DVec3> {
    let u = b - a;
    let length = u.length();
    if length == 0.0 || !length.is_finite() {
        return Some(a);
    }
    if d.cross(u).length() <= 1e-12 * length {
        let da = (a - origin).dot(d);
        let db = (b - origin).dot(d);
        return Some(if da <= db { a } else { b });
    }
    let s = line_param(a, u, origin, d)?;
    Some(a + u * s.clamp(0.0, 1.0))
}
