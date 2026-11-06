// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Selection logic: window and crossing boxes, and the connected sets that double and triple
//! clicks select. Pure functions over the model; the app keeps the selection.
//!
//! Behaviour: `docs/specs/m2-select-box.md`.

use std::collections::{BTreeSet, VecDeque};

use crate::geom::{self, Containment};
use crate::model::{BodyId, InstanceId, Model};
use crate::topology::{Context, EdgeId, FaceId, Loop};
use glam::{DMat4, DVec2, DVec3};

/// One selectable thing in a body. Ordered edges first, then faces, then instances, each by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Entity {
    Edge(EdgeId),
    Face(FaceId),
    Instance(InstanceId),
}

/// How a selection box picks entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxMode {
    /// Entities entirely inside the box.
    Window,
    /// Entities inside or touching the box.
    Crossing,
}

impl BoxMode {
    /// The mode of a box dragged from `start` to `end` (screen pixels).
    pub fn from_drag(start: DVec2, end: DVec2) -> BoxMode {
        if start.x.is_nan() || end.x.is_nan() || end.x >= start.x {
            BoxMode::Window
        } else {
            BoxMode::Crossing
        }
    }
}

#[derive(Default)]
struct FacePoly {
    outer: Vec<DVec3>,
    holes: Vec<Vec<DVec3>>,
}

#[derive(Default)]
struct Gathered {
    points: Vec<DVec3>,
    segments: Vec<(DVec3, DVec3)>,
    faces: Vec<FacePoly>,
}

fn gathered_point(out: &mut Gathered, p: DVec3, xform: DMat4) -> DVec3 {
    let p = xform.transform_point3(p);
    out.points.push(p);
    p
}

fn gather_edge(ctx: &Context, id: EdgeId, xform: DMat4, out: &mut Gathered) -> bool {
    let Some(edge) = ctx.edge(id) else {
        return false;
    };
    let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
        return false;
    };
    let a = gathered_point(out, a.position, xform);
    let b = gathered_point(out, b.position, xform);
    out.segments.push((a, b));
    true
}

fn gather_loop(ctx: &Context, l: &Loop, xform: DMat4, out: &mut Gathered) -> Option<Vec<DVec3>> {
    let mut points = Vec::with_capacity(l.uses.len());
    for u in &l.uses {
        let v = ctx.use_start(*u)?;
        let p = ctx.vertex(v)?.position;
        points.push(gathered_point(out, p, xform));
        let edge = ctx.edge(u.edge)?;
        let a = ctx.vertex(edge.ends[0])?.position;
        let b = ctx.vertex(edge.ends[1])?.position;
        let a = xform.transform_point3(a);
        let b = xform.transform_point3(b);
        out.segments.push((a, b));
    }
    Some(points)
}

fn gather_face(ctx: &Context, id: FaceId, xform: DMat4, out: &mut Gathered) -> bool {
    let Some(face) = ctx.face(id) else {
        return false;
    };
    let Some(outer) = gather_loop(ctx, &face.outer, xform, out) else {
        return false;
    };
    let mut holes = Vec::with_capacity(face.inner.len());
    for l in &face.inner {
        let Some(hole) = gather_loop(ctx, l, xform, out) else {
            return false;
        };
        holes.push(hole);
    }
    out.faces.push(FacePoly { outer, holes });
    true
}

fn gather_body(
    model: &Model,
    body: BodyId,
    xform: DMat4,
    out: &mut Gathered,
    path: &mut Vec<crate::model::DefinitionId>,
) -> bool {
    let Some(b) = model.body(body) else {
        return false;
    };
    let ctx = b.geometry();
    for (id, _) in ctx.edges() {
        if !gather_edge(ctx, id, xform, out) {
            return false;
        }
    }
    for (id, _) in ctx.faces() {
        if !gather_face(ctx, id, xform, out) {
            return false;
        }
    }
    for (_, inst) in model.visible_instances(body) {
        let d = inst.definition;
        if path.contains(&d) {
            continue;
        }
        path.push(d);
        let combined = xform * inst.transform;
        let ok = gather_body(model, BodyId::Definition(d), combined, out, path);
        path.pop();
        if !ok {
            return false;
        }
    }
    true
}

fn in_box(p: DVec2, lo: DVec2, hi: DVec2) -> bool {
    p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
}

fn segment_touches_box(a: DVec2, b: DVec2, lo: DVec2, hi: DVec2) -> bool {
    let d = b - a;
    let mut tmin = 0.0f64;
    let mut tmax = 1.0f64;
    for (p, d, l, h) in [(a.x, d.x, lo.x, hi.x), (a.y, d.y, lo.y, hi.y)] {
        if d == 0.0 {
            if p < l || p > h {
                return false;
            }
        } else {
            let mut t1 = (l - p) / d;
            let mut t2 = (h - p) / d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return false;
            }
        }
    }
    true
}

fn project_loop(l: &[DVec3], project: &dyn Fn(DVec3) -> Option<DVec2>) -> Option<Vec<DVec2>> {
    l.iter().map(|&p| project(p)).collect()
}

fn selected(
    g: &Gathered,
    project: &dyn Fn(DVec3) -> Option<DVec2>,
    lo: DVec2,
    hi: DVec2,
    mode: BoxMode,
) -> bool {
    if g.points.is_empty() {
        return false;
    }
    let mut projected = Vec::with_capacity(g.points.len());
    for &p in &g.points {
        let Some(q) = project(p) else {
            return false;
        };
        if !q.is_finite() {
            return false;
        }
        projected.push(q);
    }

    match mode {
        BoxMode::Window => projected.iter().all(|q| in_box(*q, lo, hi)),
        BoxMode::Crossing => {
            for &(a, b) in &g.segments {
                let (Some(a), Some(b)) = (project(a), project(b)) else {
                    return false;
                };
                if !a.is_finite() || !b.is_finite() {
                    return false;
                }
                if segment_touches_box(a, b, lo, hi) {
                    return true;
                }
            }

            let center = (lo + hi) * 0.5;
            for face in &g.faces {
                let Some(outer) = project_loop(&face.outer, project) else {
                    return false;
                };
                if outer.iter().any(|p| !p.is_finite()) {
                    return false;
                }
                if outer.len() < 3
                    || geom::point_in_polygon_2d(center, &outer, geom::LINEAR_TOL)
                        == Containment::Outside
                {
                    continue;
                }
                let mut in_hole = false;
                for hole in &face.holes {
                    let Some(hole) = project_loop(hole, project) else {
                        return false;
                    };
                    if hole.iter().any(|p| !p.is_finite()) {
                        return false;
                    }
                    if hole.len() >= 3
                        && geom::point_in_polygon_2d(center, &hole, geom::LINEAR_TOL)
                            == Containment::Inside
                    {
                        in_hole = true;
                        break;
                    }
                }
                if !in_hole {
                    return true;
                }
            }
            false
        }
    }
}

/// The entities of `body` that the screen box with opposite corners `a` and `b` selects.
/// `project` maps a point in `body` coordinates to screen pixels, `None` when it can't be
/// seen. Sorted, no duplicates.
pub fn box_select(
    model: &Model,
    body: BodyId,
    project: &dyn Fn(DVec3) -> Option<DVec2>,
    a: DVec2,
    b: DVec2,
    mode: BoxMode,
) -> Vec<Entity> {
    if !a.is_finite() || !b.is_finite() {
        return Vec::new();
    }
    let Some(target) = model.body(body) else {
        return Vec::new();
    };
    let lo = a.min(b);
    let hi = a.max(b);
    let ctx = target.geometry();
    let mut out = Vec::new();

    for (id, _) in ctx.edges() {
        let mut g = Gathered::default();
        if gather_edge(ctx, id, DMat4::IDENTITY, &mut g) && selected(&g, project, lo, hi, mode) {
            out.push(Entity::Edge(id));
        }
    }
    for (id, _) in ctx.faces() {
        let mut g = Gathered::default();
        if gather_face(ctx, id, DMat4::IDENTITY, &mut g) && selected(&g, project, lo, hi, mode) {
            out.push(Entity::Face(id));
        }
    }
    for (id, inst) in model.visible_instances(body) {
        let mut g = Gathered::default();
        let mut path = vec![inst.definition];
        if gather_body(
            model,
            BodyId::Definition(inst.definition),
            inst.transform,
            &mut g,
            &mut path,
        ) && selected(&g, project, lo, hi, mode)
        {
            out.push(Entity::Instance(id));
        }
    }

    out.sort();
    out.dedup();
    out
}

/// What a double click on `entity` selects. Sorted, no duplicates.
pub fn bounded(ctx: &Context, entity: Entity) -> Vec<Entity> {
    let mut out = BTreeSet::new();
    match entity {
        Entity::Instance(_) => {
            out.insert(entity);
        }
        Entity::Edge(id) => {
            if ctx.edge(id).is_none() {
                return Vec::new();
            }
            out.insert(entity);
            for f in ctx.faces_of_edge(id) {
                out.insert(Entity::Face(f));
            }
        }
        Entity::Face(id) => {
            let Some(face) = ctx.face(id) else {
                return Vec::new();
            };
            out.insert(entity);
            for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                for u in &l.uses {
                    if ctx.edge(u.edge).is_some() {
                        out.insert(Entity::Edge(u.edge));
                    }
                }
            }
        }
    }
    out.into_iter().collect()
}

/// What a triple click on `entity` selects: everything connected to it. Sorted, no duplicates.
pub fn connected(ctx: &Context, entity: Entity) -> Vec<Entity> {
    let mut visited: BTreeSet<Entity> = BTreeSet::new();
    let mut queue: VecDeque<Entity> = VecDeque::new();
    match entity {
        Entity::Instance(_) => return vec![entity],
        Entity::Edge(id) => {
            if ctx.edge(id).is_none() {
                return Vec::new();
            }
        }
        Entity::Face(id) => {
            if ctx.face(id).is_none() {
                return Vec::new();
            }
        }
    }
    visited.insert(entity);
    queue.push_back(entity);

    while let Some(node) = queue.pop_front() {
        match node {
            Entity::Edge(id) => {
                if let Some(edge) = ctx.edge(id) {
                    for v in edge.ends {
                        for e in ctx.edges_of_vertex(v) {
                            let next = Entity::Edge(e);
                            if visited.insert(next) {
                                queue.push_back(next);
                            }
                        }
                    }
                }
                for f in ctx.faces_of_edge(id) {
                    let next = Entity::Face(f);
                    if visited.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            Entity::Face(id) => {
                if let Some(face) = ctx.face(id) {
                    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                        for u in &l.uses {
                            if ctx.edge(u.edge).is_some() {
                                let next = Entity::Edge(u.edge);
                                if visited.insert(next) {
                                    queue.push_back(next);
                                }
                            }
                        }
                    }
                }
            }
            Entity::Instance(_) => {}
        }
    }
    visited.into_iter().collect()
}

fn gather_instance_segments(
    model: &Model,
    definition: crate::model::DefinitionId,
    xform: DMat4,
    out: &mut Vec<(DVec3, DVec3)>,
    path: &mut Vec<crate::model::DefinitionId>,
) {
    let Some(b) = model.body(BodyId::Definition(definition)) else {
        return;
    };
    let ctx = b.geometry();
    for (_, edge) in ctx.edges() {
        let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
            continue;
        };
        out.push((
            xform.transform_point3(a.position),
            xform.transform_point3(b.position),
        ));
    }
    for (_, inst) in model.visible_instances(BodyId::Definition(definition)) {
        if path.contains(&inst.definition) {
            continue;
        }
        path.push(inst.definition);
        gather_instance_segments(model, inst.definition, xform * inst.transform, out, path);
        path.pop();
    }
}

/// The edges of instance `id` of `body`, as segments in `body` coordinates: every edge of the
/// definition's geometry and, recursively, of the instances inside it, each with the composed
/// transforms. In no particular order; empty when the body or instance
/// doesn't exist. The app draws the selection highlight from this.
///
/// Behaviour: `docs/specs/m2-pick-imported.md`.
pub fn instance_segments(model: &Model, body: BodyId, id: InstanceId) -> Vec<(DVec3, DVec3)> {
    let Some(instance) = model.body(body).and_then(|b| b.instance(id)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut path = vec![instance.definition];
    gather_instance_segments(
        model,
        instance.definition,
        instance.transform,
        &mut out,
        &mut path,
    );
    out
}
