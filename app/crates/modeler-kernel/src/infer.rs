// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference: where the cursor's point lands while a drawing tool runs, what it snapped to,
//! and what entity is under the cursor.
//!
//! Behaviour: `docs/specs/m2-inference.md`. Works on one context, in its own coordinates;
//! instances are not entered.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::editpath::EditPath;
use crate::geom::{self, Containment};
use crate::model::{BodyId, DefinitionId, InstanceId, Model};
use crate::select::Entity;
use crate::topology::{Context, EdgeId, FaceId, VertexId};
use glam::{DMat4, DVec2, DVec3};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    Root,
    Instance(InstanceId),
    Rest,
}

impl Owner {
    fn rank(self) -> (u8, u64) {
        match self {
            Owner::Root => (0, 0),
            Owner::Instance(id) => (1, id.0),
            Owner::Rest => (2, 0),
        }
    }
}

#[derive(Clone, Copy)]
struct Hit {
    snap: Snap,
    point: DVec3,
    screen: f64,
    depth: f64,
    owner: Owner,
    context: u64,
    id: u64,
}

fn better(a: &Hit, b: &Hit) -> bool {
    if a.screen != b.screen {
        a.screen < b.screen
    } else if a.depth != b.depth {
        a.depth < b.depth
    } else if a.owner != b.owner {
        a.owner.rank() < b.owner.rank()
    } else if a.context != b.context {
        a.context < b.context
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
                owner: Owner::Root,
                context: 0,
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
                owner: Owner::Root,
                context: 0,
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
                owner: Owner::Root,
                context: 0,
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
                owner: Owner::Root,
                context: 0,
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
                owner: Owner::Root,
                context: 0,
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

// -- flattened geometry ------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FaceKey {
    context: u64,
    face: FaceId,
}

struct FlatVertex {
    id: VertexId,
    pos: DVec3,
    owner: Owner,
    context: u64,
    touching: BTreeSet<FaceKey>,
}

struct FlatEdge {
    id: EdgeId,
    a: DVec3,
    b: DVec3,
    owner: Owner,
    context: u64,
    touching: BTreeSet<FaceKey>,
}

struct FlatFace {
    id: FaceId,
    owner: Owner,
    context: u64,
    plane: geom::Plane,
    outer: Vec<DVec3>,
    inner: Vec<Vec<DVec3>>,
}

#[derive(Default)]
struct Flat {
    vertices: Vec<FlatVertex>,
    edges: Vec<FlatEdge>,
    faces: Vec<FlatFace>,
    next_context: u64,
}

fn flatten(model: &Model) -> Flat {
    let mut flat = Flat {
        next_context: 1,
        ..Flat::default()
    };
    gather_flat_body(
        model,
        BodyId::Root,
        DMat4::IDENTITY,
        Owner::Root,
        0,
        &mut Vec::new(),
        &mut flat,
    );
    flat
}

struct CachedFlat {
    key: (u64, u64),
    flat: Rc<Flat>,
}

thread_local! {
    static FLAT_CACHE: RefCell<Option<CachedFlat>> = const { RefCell::new(None) };
}

fn cached_flat(model: &Model) -> Rc<Flat> {
    let key = model.cache_key();
    FLAT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.as_ref()
            && cached.key == key
        {
            return Rc::clone(&cached.flat);
        }
        let flat = Rc::new(flatten(model));
        *cache = Some(CachedFlat {
            key,
            flat: Rc::clone(&flat),
        });
        flat
    })
}

fn gather_own_geometry(
    model: &Model,
    body: BodyId,
    xform: DMat4,
    owner: Owner,
    context: u64,
    flat: &mut Flat,
) {
    let Some(b) = model.body(body) else {
        return;
    };
    let ctx = b.geometry();

    for (id, face) in ctx.faces() {
        let outer: Vec<DVec3> = ctx
            .loop_points(&face.outer)
            .into_iter()
            .map(|p| xform.transform_point3(p))
            .collect();
        let Some(plane) = geom::fit_plane(&outer) else {
            continue;
        };
        let inner: Vec<Vec<DVec3>> = face
            .inner
            .iter()
            .map(|l| {
                ctx.loop_points(l)
                    .into_iter()
                    .map(|p| xform.transform_point3(p))
                    .collect()
            })
            .collect();
        flat.faces.push(FlatFace {
            id,
            owner,
            context,
            plane,
            outer,
            inner,
        });
    }

    for (id, edge) in ctx.edges() {
        let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
            continue;
        };
        let touching = ctx
            .faces_of_edge(id)
            .into_iter()
            .map(|face| FaceKey { context, face })
            .collect();
        flat.edges.push(FlatEdge {
            id,
            a: xform.transform_point3(a.position),
            b: xform.transform_point3(b.position),
            owner,
            context,
            touching,
        });
    }

    for (id, vertex) in ctx.vertices() {
        let mut touching = BTreeSet::new();
        for edge in ctx.edges_of_vertex(id) {
            for face in ctx.faces_of_edge(edge) {
                touching.insert(FaceKey { context, face });
            }
        }
        flat.vertices.push(FlatVertex {
            id,
            pos: xform.transform_point3(vertex.position),
            owner,
            context,
            touching,
        });
    }
}

fn gather_flat_body(
    model: &Model,
    body: BodyId,
    xform: DMat4,
    owner: Owner,
    context: u64,
    path: &mut Vec<DefinitionId>,
    flat: &mut Flat,
) {
    gather_own_geometry(model, body, xform, owner, context, flat);
    let Some(b) = model.body(body) else {
        return;
    };

    for (id, inst) in b.instances() {
        if path.contains(&inst.definition) {
            continue;
        }
        path.push(inst.definition);
        let child_owner = match owner {
            Owner::Root => Owner::Instance(id),
            Owner::Instance(root) => Owner::Instance(root),
            Owner::Rest => Owner::Rest,
        };
        let child_context = flat.next_context;
        flat.next_context += 1;
        gather_flat_body(
            model,
            BodyId::Definition(inst.definition),
            xform * inst.transform,
            child_owner,
            child_context,
            path,
            flat,
        );
        path.pop();
    }
}

fn gather_flat_rest(
    model: &Model,
    body: BodyId,
    xform: DMat4,
    context: u64,
    path: &mut Vec<DefinitionId>,
    flat: &mut Flat,
) {
    gather_own_geometry(model, body, xform, Owner::Rest, context, flat);
    let Some(b) = model.body(body) else {
        return;
    };
    for (_, inst) in b.instances() {
        if path.contains(&inst.definition) {
            continue;
        }
        path.push(inst.definition);
        let child_context = flat.next_context;
        flat.next_context += 1;
        gather_flat_rest(
            model,
            BodyId::Definition(inst.definition),
            xform * inst.transform,
            child_context,
            path,
            flat,
        );
        path.pop();
    }
}

fn flatten_in(model: &Model, path: &EditPath, rest: bool) -> Option<Flat> {
    let open_body = path.body(model)?;
    let open_xform = path.transform(model)?;
    let mut flat = Flat {
        next_context: 1,
        ..Flat::default()
    };
    let mut def_path = Vec::new();
    gather_flat_body(
        model,
        open_body,
        DMat4::IDENTITY,
        Owner::Root,
        0,
        &mut def_path,
        &mut flat,
    );
    if rest {
        let to_local = open_xform.inverse();
        if !to_local.is_finite() {
            return None;
        }
        let mut body = BodyId::Root;
        let mut world = DMat4::IDENTITY;
        for &step in &path.steps {
            let context = flat.next_context;
            flat.next_context += 1;
            gather_own_geometry(
                model,
                body,
                to_local * world,
                Owner::Rest,
                context,
                &mut flat,
            );

            let b = model.body(body)?;
            for (id, inst) in b.instances() {
                if id == step {
                    continue;
                }
                let child_context = flat.next_context;
                flat.next_context += 1;
                gather_flat_rest(
                    model,
                    BodyId::Definition(inst.definition),
                    to_local * world * inst.transform,
                    child_context,
                    &mut Vec::new(),
                    &mut flat,
                );
            }
            let instance = b.instance(step)?;
            world *= instance.transform;
            body = BodyId::Definition(instance.definition);
        }
    }
    Some(flat)
}

struct CachedFlatIn {
    key: (u64, u64, Vec<InstanceId>, bool),
    flat: Rc<Flat>,
}

thread_local! {
    static FLAT_IN_CACHE: RefCell<Option<CachedFlatIn>> = const { RefCell::new(None) };
}

fn cached_flat_in(model: &Model, path: &EditPath, rest: bool) -> Option<Rc<Flat>> {
    let (id, revision) = model.cache_key();
    let key = (id, revision, path.steps.clone(), rest);
    FLAT_IN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.as_ref()
            && cached.key == key
        {
            return Some(Rc::clone(&cached.flat));
        }
        let flat = Rc::new(flatten_in(model, path, rest)?);
        *cache = Some(CachedFlatIn {
            key,
            flat: Rc::clone(&flat),
        });
        Some(flat)
    })
}

fn flat_face_t(face: &FlatFace, origin: DVec3, d: DVec3) -> Option<f64> {
    let plane = face.plane;
    let denom = d.dot(plane.normal);
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = -plane.signed_distance(origin) / denom;
    if !t.is_finite() || t <= 1e-9 {
        return None;
    }

    let hit = origin + d * t;
    let hit2 = geom::to_plane_2d(&[hit], plane.normal)[0];
    let outer = geom::to_plane_2d(&face.outer, plane.normal);
    if !matches!(
        geom::point_in_polygon_2d(hit2, &outer, 1e-9),
        Containment::Inside | Containment::Boundary
    ) {
        return None;
    }
    for hole in &face.inner {
        let points = geom::to_plane_2d(hole, plane.normal);
        if geom::point_in_polygon_2d(hit2, &points, 1e-9) == Containment::Inside {
            return None;
        }
    }
    Some(t)
}

fn flat_ray_face(flat: &Flat, origin: DVec3, dir: DVec3) -> Option<(&FlatFace, f64)> {
    let d = unit(dir)?;
    let mut best: Option<(&FlatFace, f64)> = None;
    for face in &flat.faces {
        let Some(t) = flat_face_t(face, origin, d) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((best_face, best_t)) => {
                t < best_t
                    || (t == best_t
                        && (face.owner, face.context, face.id)
                            < (best_face.owner, best_face.context, best_face.id))
            }
        };
        if better {
            best = Some((face, t));
        }
    }
    best
}

fn flat_is_hidden(
    flat: &Flat,
    probe: &Probe,
    d: DVec3,
    p: DVec3,
    touching: &BTreeSet<FaceKey>,
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
    for face in &flat.faces {
        if touching.contains(&FaceKey {
            context: face.context,
            face: face.id,
        }) {
            continue;
        }
        if let Some(t) = flat_face_t(face, s, view)
            && t < limit
        {
            return true;
        }
    }
    false
}

fn flat_endpoint_best(flat: &Flat, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    for vertex in &flat.vertices {
        let p = vertex.pos;
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        if flat_is_hidden(flat, probe, d, p, &vertex.touching) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::Endpoint(vertex.id),
                point: p,
                screen,
                depth: dep,
                owner: vertex.owner,
                context: vertex.context,
                id: vertex.id.0,
            },
        );
    }
    best
}

fn flat_midpoint_best(flat: &Flat, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    for edge in &flat.edges {
        let p = (edge.a + edge.b) / 2.0;
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        if flat_is_hidden(flat, probe, d, p, &edge.touching) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::Midpoint(edge.id),
                point: p,
                screen,
                depth: dep,
                owner: edge.owner,
                context: edge.context,
                id: edge.id.0,
            },
        );
    }
    best
}

fn flat_center_best(flat: &Flat, probe: &Probe, d: DVec3) -> Option<Hit> {
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
        if flat_is_hidden(flat, probe, d, p, &empty) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::Center(i),
                point: p,
                screen,
                depth: dep,
                owner: Owner::Root,
                context: 0,
                id: i as u64,
            },
        );
    }
    best
}

fn flat_on_edge_best(flat: &Flat, probe: &Probe, d: DVec3) -> Option<Hit> {
    let mut best = None;
    for edge in &flat.edges {
        let Some(p) = segment_point_nearest_ray(edge.a, edge.b, probe.origin, d) else {
            continue;
        };
        let dep = depth(probe, p, d);
        if dep.is_nan() || dep <= 0.0 {
            continue;
        }
        let Some(screen) = in_range(probe, p) else {
            continue;
        };
        if flat_is_hidden(flat, probe, d, p, &edge.touching) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::OnEdge(edge.id),
                point: p,
                screen,
                depth: dep,
                owner: edge.owner,
                context: edge.context,
                id: edge.id.0,
            },
        );
    }
    best
}

fn flat_on_axis_best(flat: &Flat, probe: &Probe, d: DVec3) -> Option<Hit> {
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
        if flat_is_hidden(flat, probe, d, p, &empty) {
            continue;
        }
        consider(
            &mut best,
            Hit {
                snap: Snap::OnAxis(i),
                point: p,
                screen,
                depth: dep,
                owner: Owner::Root,
                context: 0,
                id: i as u64,
            },
        );
    }
    best
}

fn flat_point_categories(flat: &Flat, probe: &Probe, d: DVec3) -> Option<Hit> {
    flat_endpoint_best(flat, probe, d)
        .or_else(|| flat_midpoint_best(flat, probe, d))
        .or_else(|| flat_center_best(flat, probe, d))
}

/// Like [`pick`], for the root body of `model` including the geometry inside instances (at any
/// depth, with their transforms). A hit inside an instance is reported as the root-level
/// instance that holds it, `Entity::Instance`.
///
/// Behaviour: `docs/specs/m2-pick-imported.md`.
pub fn pick_model(model: &Model, probe: &Probe) -> Option<Entity> {
    if model.root().instances().next().is_none() {
        return pick(model.root().geometry(), probe);
    }
    let d = valid_probe(probe)?;
    let flat = cached_flat(model);
    if let Some(Hit {
        snap: Snap::OnEdge(edge),
        owner,
        ..
    }) = flat_on_edge_best(&flat, probe, d)
    {
        return Some(match owner {
            Owner::Root => Entity::Edge(edge),
            Owner::Instance(id) => Entity::Instance(id),
            Owner::Rest => return None,
        });
    }
    flat_ray_face(&flat, probe.origin, d).map(|(face, _)| match face.owner {
        Owner::Root => Entity::Face(face.id),
        Owner::Instance(id) => Entity::Instance(id),
        Owner::Rest => Entity::Face(face.id),
    })
}

/// Like [`infer`], for the root body of `model` including the geometry inside instances:
/// endpoints, midpoints, points on edges and on faces there snap like root geometry. Ids in
/// `Snap` for geometry inside an instance are the ids in the definition's own context; callers
/// use the kind and the point only.
///
/// Behaviour: `docs/specs/m2-pick-imported.md`.
pub fn infer_model(model: &Model, probe: &Probe) -> Option<Inference> {
    if model.root().instances().next().is_none() {
        return infer(model.root().geometry(), probe);
    }
    let d = valid_probe(probe)?;
    let flat = cached_flat(model);

    if let (Some(anchor), Some(a)) = (probe.anchor, probe.lock.and_then(unit)) {
        if let Some(hit) = flat_point_categories(&flat, probe, d) {
            let point = anchor + a * ((hit.point - anchor).dot(a));
            return Some(inference(probe, hit.snap, point));
        }
        let point = match line_param(anchor, a, probe.origin, d) {
            Some(s) if d.cross(a).length() >= 1e-9 => anchor + a * s,
            _ => anchor,
        };
        return Some(inference(probe, Snap::Along, point));
    }

    if let Some(hit) = flat_point_categories(&flat, probe, d)
        .or_else(|| flat_on_edge_best(&flat, probe, d))
        .or_else(|| flat_on_axis_best(&flat, probe, d))
    {
        return Some(inference(probe, hit.snap, hit.point));
    }

    if let Some((face, t)) = flat_ray_face(&flat, probe.origin, d) {
        let point = probe.origin + d * t;
        return Some(inference(probe, Snap::OnFace(face.id), point));
    }

    if let Some(point) = ground_point(probe, d) {
        return Some(inference(probe, Snap::Ground, point));
    }

    None
}

/// Like [`pick_model`], for the body open at `path` (`docs/specs/m4-edit-path.md`). The probe
/// is in the open body's coordinates. The open body's edges and faces are reported as
/// themselves and geometry inside its instances as the instance of the open body that holds
/// it. With `rest`, the rest of the model takes part too, and a hit on it gives `None` (a
/// click outside the open instance). `None` when the path doesn't resolve or its transform
/// can't be inverted.
pub fn pick_in(model: &Model, path: &EditPath, rest: bool, probe: &Probe) -> Option<Entity> {
    if path.is_root() {
        return pick_model(model, probe);
    }
    open_transform(model, path)?;
    let d = valid_probe(probe)?;
    let flat = cached_flat_in(model, path, rest)?;
    if let Some(Hit {
        snap: Snap::OnEdge(edge),
        owner,
        ..
    }) = flat_on_edge_best(&flat, probe, d)
    {
        return match owner {
            Owner::Root => Some(Entity::Edge(edge)),
            Owner::Instance(id) => Some(Entity::Instance(id)),
            Owner::Rest => None,
        };
    }
    flat_ray_face(&flat, probe.origin, d).and_then(|(face, _)| match face.owner {
        Owner::Root => Some(Entity::Face(face.id)),
        Owner::Instance(id) => Some(Entity::Instance(id)),
        Owner::Rest => None,
    })
}

/// Like [`infer_model`], for the body open at `path` (`docs/specs/m4-edit-path.md`). The probe
/// and the result are in the open body's coordinates. Snaps to the open body's geometry and
/// the geometry inside its instances and, with `rest`, to every other geometry of the model
/// placed where it shows. `None` when the path doesn't resolve or its transform can't be
/// inverted.
pub fn infer_in(model: &Model, path: &EditPath, rest: bool, probe: &Probe) -> Option<Inference> {
    if path.is_root() {
        return infer_model(model, probe);
    }
    open_transform(model, path)?;
    let d = valid_probe(probe)?;
    let flat = cached_flat_in(model, path, rest)?;

    if let (Some(anchor), Some(a)) = (probe.anchor, probe.lock.and_then(unit)) {
        if let Some(hit) = flat_point_categories(&flat, probe, d) {
            let point = anchor + a * ((hit.point - anchor).dot(a));
            return Some(inference(probe, hit.snap, point));
        }
        let point = match line_param(anchor, a, probe.origin, d) {
            Some(s) if d.cross(a).length() >= 1e-9 => anchor + a * s,
            _ => anchor,
        };
        return Some(inference(probe, Snap::Along, point));
    }

    if let Some(hit) = flat_point_categories(&flat, probe, d)
        .or_else(|| flat_on_edge_best(&flat, probe, d))
        .or_else(|| flat_on_axis_best(&flat, probe, d))
    {
        return Some(inference(probe, hit.snap, hit.point));
    }

    if let Some((face, t)) = flat_ray_face(&flat, probe.origin, d) {
        let point = probe.origin + d * t;
        return Some(inference(probe, Snap::OnFace(face.id), point));
    }

    if let Some(point) = ground_point(probe, d) {
        return Some(inference(probe, Snap::Ground, point));
    }

    None
}

/// The open body and its world transform, unless the path doesn't resolve or the transform
/// can't be used to map between the open body and world coordinates.
fn open_transform(model: &Model, path: &EditPath) -> Option<(BodyId, DMat4)> {
    let body = path.body(model)?;
    let transform = path.transform(model)?;
    if !transform.to_cols_array().iter().all(|v| v.is_finite())
        || transform.determinant().abs() < 1e-12
    {
        return None;
    }
    Some((body, transform))
}
