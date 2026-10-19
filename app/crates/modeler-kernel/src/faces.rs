// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Coplanar face finding: planar arrangement of edges, regions, and face overlap tests.
//!
//! Behaviour: `docs/specs/m1-sticky-faces.md`. Everything here is private to the kernel.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DVec2, DVec3};

use crate::geom::{self, Containment, LINEAR_TOL, Plane};
use crate::topology::{Context, EdgeId, EdgeUse, Face, FaceId, Loop, VertexId};

/// One directed step of a boundary walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    pub edge: EdgeId,
    pub from: VertexId,
    pub to: VertexId,
}

/// One bounded region of a plane: an outline walk plus its hole walks.
#[derive(Clone, Debug)]
pub(crate) struct Region {
    pub outline: Vec<Step>,
    pub holes: Vec<Vec<Step>>,
    pub edges: BTreeSet<EdgeId>,
    pub lowest_edge: EdgeId,
    pub simple: bool,
    outline_points: Vec<DVec2>,
    hole_points: Vec<Vec<DVec2>>,
}

fn unit(n: DVec3) -> Option<DVec3> {
    let len = n.length();
    if len.is_finite() && len > 1e-12 {
        Some(n / len)
    } else {
        None
    }
}

/// The unit normal of a plane in canonical orientation: the first non-zero component of
/// `z`, `y`, `x` is positive.
pub(crate) fn canonical_normal(normal: DVec3) -> Option<DVec3> {
    let mut n = unit(normal)?;
    let first = if n.z.abs() > 1e-9 {
        n.z
    } else if n.y.abs() > 1e-9 {
        n.y
    } else if n.x.abs() > 1e-9 {
        n.x
    } else {
        return None;
    };
    if first < 0.0 {
        n = -n;
    }
    Some(n)
}

pub(crate) fn canonical_plane(point: DVec3, normal: DVec3) -> Option<Plane> {
    Some(Plane {
        point,
        normal: canonical_normal(normal)?,
    })
}

/// Two planes are the same when their unit normals are parallel and a point of one lies
/// within tolerance of the other.
pub(crate) fn same_plane(a: &Plane, b: &Plane) -> bool {
    let (Some(na), Some(nb)) = (unit(a.normal), unit(b.normal)) else {
        return false;
    };
    na.dot(nb).abs() >= 1.0 - 1e-9 && na.dot(b.point - a.point).abs() <= LINEAR_TOL
}

/// The plane of a face, derived from its outer loop.
pub(crate) fn face_plane_of(ctx: &Context, face: &Face) -> Option<Plane> {
    geom::fit_plane(&ctx.loop_points(&face.outer))
}

pub(crate) fn face_edges(face: &Face) -> BTreeSet<EdgeId> {
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| l.uses.iter().map(|u| u.edge))
        .collect()
}

fn outer_edges(face: &Face) -> BTreeSet<EdgeId> {
    face.outer.uses.iter().map(|u| u.edge).collect()
}

fn edge_midpoint(ctx: &Context, edge: EdgeId) -> Option<DVec3> {
    let e = ctx.edge(edge)?;
    let a = ctx.vertex(e.ends[0])?.position;
    let b = ctx.vertex(e.ends[1])?.position;
    Some((a + b) * 0.5)
}

/// True when `p` is strictly inside the face: inside the outer loop and outside every
/// hole (neither inside nor on the boundary).
pub(crate) fn point_inside_face(ctx: &Context, face: &Face, p: DVec3) -> bool {
    let Some(plane) = face_plane_of(ctx, face) else {
        return false;
    };
    let n = plane.normal;
    let (u, v) = geom::plane_basis(n);
    let pp = DVec2::new(p.dot(u), p.dot(v));
    let outer = geom::to_plane_2d(&ctx.loop_points(&face.outer), n);
    if geom::point_in_polygon_2d(pp, &outer, LINEAR_TOL) != Containment::Inside {
        return false;
    }
    for l in &face.inner {
        let hole = geom::to_plane_2d(&ctx.loop_points(l), n);
        if geom::point_in_polygon_2d(pp, &hole, LINEAR_TOL) != Containment::Outside {
            return false;
        }
    }
    true
}

fn use_direction(ctx: &Context, use_: EdgeUse) -> Option<DVec3> {
    let e = ctx.edge(use_.edge)?;
    let a = ctx.vertex(e.ends[0])?.position;
    let b = ctx.vertex(e.ends[1])?.position;
    Some(if use_.reversed { a - b } else { b - a })
}

pub(crate) fn face_use_direction(ctx: &Context, face: &Face, edge: EdgeId) -> Option<DVec3> {
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| l.uses.iter())
        .find(|u| u.edge == edge)
        .and_then(|u| use_direction(ctx, *u))
}

pub(crate) fn step_direction(ctx: &Context, step: &Step) -> DVec3 {
    let from = ctx
        .vertex(step.from)
        .map(|v| v.position)
        .unwrap_or(DVec3::ZERO);
    let to = ctx
        .vertex(step.to)
        .map(|v| v.position)
        .unwrap_or(DVec3::ZERO);
    to - from
}

fn step_midpoint(ctx: &Context, step: &Step) -> DVec3 {
    let from = ctx
        .vertex(step.from)
        .map(|v| v.position)
        .unwrap_or(DVec3::ZERO);
    let to = ctx
        .vertex(step.to)
        .map(|v| v.position)
        .unwrap_or(DVec3::ZERO);
    (from + to) * 0.5
}

pub(crate) fn to_2d(normal: DVec3, p: DVec3) -> DVec2 {
    let (u, v) = geom::plane_basis(normal);
    DVec2::new(p.dot(u), p.dot(v))
}

pub(crate) fn region_contains_2d(region: &Region, p: DVec2) -> bool {
    geom::point_in_polygon_2d(p, &region.outline_points, LINEAR_TOL) == Containment::Inside
        && region
            .hole_points
            .iter()
            .all(|h| geom::point_in_polygon_2d(p, h, LINEAR_TOL) == Containment::Outside)
}

/// True when the region lies inside `face`, using the rules of the spec.
pub(crate) fn region_inside_face(
    ctx: &Context,
    region: &Region,
    face: &Face,
    plane_normal: DVec3,
) -> bool {
    let f_edges = face_edges(face);
    if let Some(step) = region.outline.iter().find(|s| !f_edges.contains(&s.edge)) {
        return point_inside_face(ctx, face, step_midpoint(ctx, step));
    }
    let Some(step) = region.outline.first() else {
        return false;
    };
    let (Some(face_dir), Some(region_dir)) = (
        face_use_direction(ctx, face, step.edge),
        Some(step_direction(ctx, step)),
    ) else {
        return false;
    };
    let Some(face_plane) = face_plane_of(ctx, face) else {
        return false;
    };
    let same_facing = face_plane.normal.dot(plane_normal) > 0.0;
    let same_direction = face_dir.dot(region_dir) > 0.0;
    if same_facing {
        same_direction
    } else {
        !same_direction
    }
}

fn walk_is_simple(steps: &[Step]) -> bool {
    if steps.len() < 3 {
        return false;
    }
    let mut seen = BTreeSet::new();
    steps.iter().all(|s| seen.insert(s.from))
}

fn walk_loop(ctx: &Context, steps: &[Step]) -> Option<Loop> {
    let mut uses = Vec::with_capacity(steps.len());
    for step in steps {
        let edge = ctx.edge(step.edge)?;
        let reversed = if edge.ends[0] == step.from && edge.ends[1] == step.to {
            false
        } else if edge.ends[0] == step.to && edge.ends[1] == step.from {
            true
        } else {
            return None;
        };
        uses.push(EdgeUse {
            edge: step.edge,
            reversed,
        });
    }
    Some(Loop { uses })
}

fn reverse_loop(mut loop_: Loop) -> Loop {
    loop_.uses.reverse();
    for use_ in &mut loop_.uses {
        use_.reversed = !use_.reversed;
    }
    loop_
}

fn rotate_loop(loop_: &mut Loop) {
    if let Some(pos) = loop_
        .uses
        .iter()
        .enumerate()
        .min_by_key(|(_, u)| u.edge.0)
        .map(|(i, _)| i)
    {
        loop_.uses.rotate_left(pos);
    }
}

/// Build the face for a region. The unflipped face fronts `+n`; `flip` reverses every loop
/// so it fronts `-n`. Loops start at their lowest-id edge and holes are ordered by it.
pub(crate) fn region_face(ctx: &Context, region: &Region, flip: bool) -> Option<Face> {
    let mut outer = walk_loop(ctx, &region.outline)?;
    let mut holes = region
        .holes
        .iter()
        .map(|h| walk_loop(ctx, h))
        .collect::<Option<Vec<_>>>()?;
    if flip {
        outer = reverse_loop(outer);
        holes = holes.into_iter().map(reverse_loop).collect();
    }
    rotate_loop(&mut outer);
    for hole in &mut holes {
        rotate_loop(hole);
    }
    holes.sort_by_key(|h| h.uses.first().map(|u| u.edge.0).unwrap_or(u64::MAX));
    Some(Face {
        outer,
        inner: holes,
    })
}

fn find_bridges(adj: &BTreeMap<VertexId, Vec<(EdgeId, VertexId)>>) -> BTreeSet<EdgeId> {
    fn dfs(
        u: VertexId,
        parent: Option<EdgeId>,
        adj: &BTreeMap<VertexId, Vec<(EdgeId, VertexId)>>,
        tin: &mut BTreeMap<VertexId, usize>,
        low: &mut BTreeMap<VertexId, usize>,
        bridges: &mut BTreeSet<EdgeId>,
        timer: &mut usize,
    ) {
        tin.insert(u, *timer);
        low.insert(u, *timer);
        *timer += 1;
        if let Some(neighbours) = adj.get(&u) {
            for &(edge, v) in neighbours {
                if Some(edge) == parent {
                    continue;
                }
                if tin.contains_key(&v) {
                    let tv = tin[&v];
                    if let Some(l) = low.get_mut(&u) {
                        *l = (*l).min(tv);
                    }
                } else {
                    dfs(v, Some(edge), adj, tin, low, bridges, timer);
                    let lv = low[&v];
                    if let Some(l) = low.get_mut(&u) {
                        *l = (*l).min(lv);
                    }
                    if lv > tin[&u] {
                        bridges.insert(edge);
                    }
                }
            }
        }
    }

    let mut tin = BTreeMap::new();
    let mut low = BTreeMap::new();
    let mut bridges = BTreeSet::new();
    let mut timer = 0;
    for &start in adj.keys() {
        if !tin.contains_key(&start) {
            dfs(
                start,
                None,
                adj,
                &mut tin,
                &mut low,
                &mut bridges,
                &mut timer,
            );
        }
    }
    bridges
}

/// The arrangement of all edges lying in `plane`: its bounded regions, each with holes.
pub(crate) fn arrangement(ctx: &Context, plane: &Plane) -> Vec<Region> {
    let Some(normal) = canonical_normal(plane.normal) else {
        return Vec::new();
    };
    let (u_axis, v_axis) = geom::plane_basis(normal);
    let project = |p: DVec3| DVec2::new(p.dot(u_axis), p.dot(v_axis));

    let mut geometry: BTreeMap<EdgeId, (VertexId, VertexId, DVec2, DVec2)> = BTreeMap::new();
    for (id, edge) in ctx.edges() {
        let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
            continue;
        };
        if plane.signed_distance(a.position).abs() > LINEAR_TOL
            || plane.signed_distance(b.position).abs() > LINEAR_TOL
        {
            continue;
        }
        geometry.insert(
            id,
            (
                edge.ends[0],
                edge.ends[1],
                project(a.position),
                project(b.position),
            ),
        );
    }
    if geometry.is_empty() {
        return Vec::new();
    }

    let mut adj: BTreeMap<VertexId, Vec<(EdgeId, VertexId)>> = BTreeMap::new();
    for (id, (a, b, _, _)) in &geometry {
        adj.entry(*a).or_default().push((*id, *b));
        adj.entry(*b).or_default().push((*id, *a));
    }
    let bridges = find_bridges(&adj);

    let mut turned: BTreeMap<VertexId, Vec<(EdgeId, VertexId, f64)>> = BTreeMap::new();
    for (id, (a, b, a2, b2)) in &geometry {
        if bridges.contains(id) {
            continue;
        }
        let ab = *b2 - *a2;
        let ba = *a2 - *b2;
        turned
            .entry(*a)
            .or_default()
            .push((*id, *b, ab.y.atan2(ab.x)));
        turned
            .entry(*b)
            .or_default()
            .push((*id, *a, ba.y.atan2(ba.x)));
    }
    for list in turned.values_mut() {
        list.sort_by(|x, y| x.2.total_cmp(&y.2));
    }
    if turned.is_empty() {
        return Vec::new();
    }

    let half_step = |edge: EdgeId, forward: bool| -> Option<(VertexId, VertexId)> {
        let (a, b, _, _) = geometry.get(&edge)?;
        Some(if forward { (*a, *b) } else { (*b, *a) })
    };
    let next_half = |edge: EdgeId, at: VertexId| -> Option<(EdgeId, bool)> {
        let list = turned.get(&at)?;
        let pos = list.iter().position(|(e, _, _)| *e == edge)?;
        let prev = (pos + list.len() - 1) % list.len();
        let (next_edge, next_vertex, _) = list[prev];
        let (a, _, _, _) = geometry.get(&next_edge)?;
        let forward = *a == at && *a != next_vertex;
        Some((next_edge, forward))
    };

    let mut visited: BTreeSet<(EdgeId, bool)> = BTreeSet::new();
    struct RawWalk {
        steps: Vec<Step>,
        points: Vec<DVec2>,
        area: f64,
        component: usize,
    }
    let mut walks: Vec<RawWalk> = Vec::new();
    for &edge in geometry.keys() {
        for forward in [true, false] {
            if visited.contains(&(edge, forward)) {
                continue;
            }
            let Some((mut from, mut to)) = half_step(edge, forward) else {
                continue;
            };
            let start = (edge, forward);
            let mut cur = start;
            let mut steps = Vec::new();
            loop {
                visited.insert(cur);
                steps.push(Step {
                    edge: cur.0,
                    from,
                    to,
                });
                let Some(next) = next_half(cur.0, to) else {
                    break;
                };
                if next == start {
                    break;
                }
                if visited.contains(&next) {
                    break;
                }
                let Some((nf, nt)) = half_step(next.0, next.1) else {
                    break;
                };
                cur = next;
                from = nf;
                to = nt;
            }
            let points: Vec<DVec2> = steps
                .iter()
                .map(|s| {
                    project(
                        ctx.vertex(s.from)
                            .map(|v| v.position)
                            .unwrap_or(DVec3::ZERO),
                    )
                })
                .collect();
            let area = geom::signed_area_2d(&points);
            walks.push(RawWalk {
                steps,
                points,
                area,
                component: 0,
            });
        }
    }

    // Connected components of the graph with the bridges removed.
    let mut component: BTreeMap<VertexId, usize> = BTreeMap::new();
    let mut next_component = 0;
    for &start in turned.keys() {
        if component.contains_key(&start) {
            continue;
        }
        let id = next_component;
        next_component += 1;
        component.insert(start, id);
        let mut stack = vec![start];
        while let Some(v) = stack.pop() {
            if let Some(list) = turned.get(&v) {
                for &(_, w, _) in list {
                    if let std::collections::btree_map::Entry::Vacant(entry) = component.entry(w) {
                        entry.insert(id);
                        stack.push(w);
                    }
                }
            }
        }
    }
    for walk in &mut walks {
        walk.component = walk
            .steps
            .first()
            .and_then(|s| component.get(&s.from).copied())
            .unwrap_or(0);
    }

    let mut regions: Vec<Region> = Vec::new();
    let mut outline_info: Vec<(usize, usize, Vec<DVec2>, f64)> = Vec::new();
    type Contour = (Vec<Step>, Vec<DVec2>, usize, BTreeSet<EdgeId>);
    let mut contour_info: Vec<Contour> = Vec::new();
    for walk in walks {
        if walk.area > 0.0 {
            let edges: BTreeSet<EdgeId> = walk.steps.iter().map(|s| s.edge).collect();
            let lowest_edge = edges.iter().next().copied().unwrap_or(EdgeId(u64::MAX));
            let index = regions.len();
            regions.push(Region {
                outline: walk.steps,
                holes: Vec::new(),
                edges,
                lowest_edge,
                simple: false,
                outline_points: walk.points.clone(),
                hole_points: Vec::new(),
            });
            outline_info.push((index, walk.component, walk.points, walk.area.abs()));
        } else if walk.area < 0.0 {
            let edges: BTreeSet<EdgeId> = walk.steps.iter().map(|s| s.edge).collect();
            contour_info.push((walk.steps, walk.points, walk.component, edges));
        }
    }

    for (steps, points, comp, edges) in contour_info {
        let Some(first) = points.first() else {
            continue;
        };
        let mut best: Option<(f64, usize)> = None;
        for (index, outline_comp, outline_points, area) in &outline_info {
            if *outline_comp == comp {
                continue;
            }
            if geom::point_in_polygon_2d(*first, outline_points, LINEAR_TOL) != Containment::Inside
            {
                continue;
            }
            if best.is_none_or(|(best_area, _)| *area < best_area) {
                best = Some((*area, *index));
            }
        }
        if let Some((_, index)) = best {
            regions[index].holes.push(steps);
            regions[index].hole_points.push(points);
            regions[index].edges.extend(edges);
        }
    }

    for region in &mut regions {
        region.lowest_edge = region
            .edges
            .iter()
            .next()
            .copied()
            .unwrap_or(EdgeId(u64::MAX));
        region.simple =
            walk_is_simple(&region.outline) && region.holes.iter().all(|h| walk_is_simple(h));
    }

    regions
}

fn loop_edge_inside_face(ctx: &Context, from: &Face, into: &Face) -> bool {
    std::iter::once(&from.outer)
        .chain(from.inner.iter())
        .flat_map(|l| l.uses.iter())
        .any(|u| edge_midpoint(ctx, u.edge).is_some_and(|p| point_inside_face(ctx, into, p)))
}

/// Pairs of coplanar faces whose interiors overlap.
pub(crate) fn face_overlaps(ctx: &Context) -> Vec<(FaceId, FaceId)> {
    let ids: Vec<FaceId> = ctx.faces().map(|(id, _)| id).collect();
    let mut out = Vec::new();
    for i in 0..ids.len() {
        for j in i + 1..ids.len() {
            let (a, b) = (ids[i], ids[j]);
            let (Some(fa), Some(fb)) = (ctx.face(a), ctx.face(b)) else {
                continue;
            };
            let (Some(pa), Some(pb)) = (face_plane_of(ctx, fa), face_plane_of(ctx, fb)) else {
                continue;
            };
            if !same_plane(&pa, &pb) {
                continue;
            }
            let same_outline = outer_edges(fa) == outer_edges(fb);
            if same_outline
                || loop_edge_inside_face(ctx, fa, fb)
                || loop_edge_inside_face(ctx, fb, fa)
            {
                out.push((a, b));
            }
        }
    }
    out
}
