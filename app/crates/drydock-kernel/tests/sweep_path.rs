// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sweep paths for Follow Me (`docs/specs/m9-sweep-path.md`).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use drydock_kernel::curve;
use drydock_kernel::sweep::*;
use drydock_kernel::topology::*;
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

#[track_caller]
fn assert_near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn vertex(ctx: &mut Context, p: DVec3) -> VertexId {
    ctx.insert_vertex(p, &mut Edit::new())
}

fn edge(ctx: &mut Context, a: VertexId, b: VertexId) -> EdgeId {
    ctx.insert_edge(a, b, &mut Edit::new()).unwrap()
}

/// Vertices at `points` (inserted in order) and an edge between each neighbour, inserted in
/// order, each running forward; closed adds the edge from the last back to the first.
fn chain(ctx: &mut Context, points: &[DVec3], closed: bool) -> (Vec<VertexId>, Vec<EdgeId>) {
    let vs: Vec<VertexId> = points.iter().map(|p| vertex(ctx, *p)).collect();
    let mut es: Vec<EdgeId> = vs.windows(2).map(|w| edge(ctx, w[0], w[1])).collect();
    if closed {
        es.push(edge(ctx, *vs.last().unwrap(), vs[0]));
    }
    (vs, es)
}

fn positions(ctx: &Context, vs: &[VertexId]) -> Vec<DVec3> {
    vs.iter()
        .map(|id| ctx.vertex(*id).unwrap().position)
        .collect()
}

/// `n + 1` points on the arc of radius 1 about the origin in the XY plane from angle `from`
/// to angle `to`.
fn arc_points(from: f64, to: f64, n: usize) -> Vec<DVec3> {
    (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f64 / n as f64;
            v(a.cos(), a.sin(), 0.0)
        })
        .collect()
}

/// An open arc made a curve: (vertices, edges, curve id).
fn arc(ctx: &mut Context, from: f64, to: f64, n: usize) -> (Vec<VertexId>, Vec<EdgeId>) {
    let points = arc_points(from, to, n);
    let (vs, es) = chain(ctx, &points, false);
    curve::add_curve(ctx, &points, false, &mut Edit::new()).expect("arc becomes a curve");
    (vs, es)
}

/// A unit square face in z = 0, with its outer loop given starting at corner `start`
/// (counter-clockwise seen from +Z). Returns the face, the corner vertices in counter-clockwise
/// order from (0,0), and the edges where `edges[i]` joins corner i and corner i + 1.
fn square_face(ctx: &mut Context, start: usize) -> (FaceId, Vec<VertexId>, Vec<EdgeId>) {
    let corners = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let (vs, es) = chain(ctx, &corners, true);
    let uses = (0..4)
        .map(|i| EdgeUse::forward(es[(start + i) % 4]))
        .collect();
    let face = Face {
        outer: Loop { uses },
        inner: Vec::new(),
    };
    let f = ctx.insert_face(face, &mut Edit::new()).unwrap();
    (f, vs, es)
}

#[track_caller]
fn assert_path(ctx: &Context, path: &SweepPath, vs: &[VertexId], es: &[EdgeId], closed: bool) {
    assert_eq!(path.vertices, vs, "vertices");
    assert_eq!(path.edges, es, "edges");
    assert_eq!(path.closed, closed, "closed");
    assert_eq!(path.points, positions(ctx, vs), "points");
}

// ---- Path from edges: open chains ----

#[test]
fn one_edge_is_an_open_path() {
    let mut ctx = Context::new();
    let (vs, es) = chain(&mut ctx, &[v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0)], false);
    let path = path_from_edges(&ctx, &es).unwrap();
    assert_path(&ctx, &path, &vs, &es, false);
    assert_near(path.length(), 2.0);
}

#[test]
fn chain_in_order_starts_at_its_lower_end() {
    let mut ctx = Context::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(1.0, 1.0, 1.0),
    ];
    let (vs, es) = chain(&mut ctx, &pts, false);
    let path = path_from_edges(&ctx, &es).unwrap();
    assert_path(&ctx, &path, &vs, &es, false);
    assert_near(path.length(), 3.0);
}

#[test]
fn edge_order_in_the_input_does_not_matter() {
    let mut ctx = Context::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(1.0, 1.0, 1.0),
    ];
    let (vs, es) = chain(&mut ctx, &pts, false);
    let path = path_from_edges(&ctx, &[es[2], es[0], es[1]]).unwrap();
    assert_path(&ctx, &path, &vs, &es, false);
}

#[test]
fn repeated_edges_in_the_input_count_once() {
    let mut ctx = Context::new();
    let pts = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)];
    let (vs, es) = chain(&mut ctx, &pts, false);
    let path = path_from_edges(&ctx, &[es[1], es[0], es[1], es[0]]).unwrap();
    assert_path(&ctx, &path, &vs, &es, false);
}

#[test]
fn open_path_starts_at_the_lower_id_end_wherever_it_lies() {
    // Vertex ids 0..4 placed so that the path runs c - a - b - d: its ends are c and d, and
    // c has the lower id.
    let mut ctx = Context::new();
    let a = vertex(&mut ctx, v(1.0, 0.0, 0.0));
    let b = vertex(&mut ctx, v(2.0, 0.0, 0.0));
    let c = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let d = vertex(&mut ctx, v(3.0, 0.0, 0.0));
    let ab = edge(&mut ctx, a, b);
    let db = edge(&mut ctx, d, b);
    let ac = edge(&mut ctx, a, c);
    let path = path_from_edges(&ctx, &[db, ab, ac]).unwrap();
    assert_path(&ctx, &path, &[c, a, b, d], &[ac, ab, db], false);
    assert_near(path.length(), 3.0);
}

#[test]
fn stored_edge_direction_does_not_matter() {
    let mut ctx = Context::new();
    let a = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let b = vertex(&mut ctx, v(1.0, 0.0, 0.0));
    let c = vertex(&mut ctx, v(1.0, 2.0, 0.0));
    let ba = edge(&mut ctx, b, a);
    let cb = edge(&mut ctx, c, b);
    let path = path_from_edges(&ctx, &[cb, ba]).unwrap();
    assert_path(&ctx, &path, &[a, b, c], &[ba, cb], false);
    assert_near(path.length(), 3.0);
}

#[test]
fn edges_not_given_are_not_part_of_the_path() {
    let mut ctx = Context::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let (vs, es) = chain(&mut ctx, &pts, true);
    let path = path_from_edges(&ctx, &[es[1], es[2]]).unwrap();
    assert_path(&ctx, &path, &vs[1..4], &es[1..3], false);
}

// ---- Path from edges: closed chains ----

#[test]
fn closed_square_is_a_closed_path() {
    let mut ctx = Context::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let (vs, es) = chain(&mut ctx, &pts, true);
    let path = path_from_edges(&ctx, &[es[3], es[1], es[0], es[2]]).unwrap();
    // From the lowest vertex along its lower-id edge (es[0] to vs[1]).
    assert_path(&ctx, &path, &vs, &es, true);
    assert_eq!(path.vertices.len(), path.edges.len());
    assert_near(path.length(), 4.0);
}

#[test]
fn closed_path_heads_along_the_lower_id_edge_of_its_lowest_vertex() {
    let mut ctx = Context::new();
    let a = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let b = vertex(&mut ctx, v(1.0, 0.0, 0.0));
    let c = vertex(&mut ctx, v(1.0, 1.0, 0.0));
    let d = vertex(&mut ctx, v(0.0, 1.0, 0.0));
    // The edge from a to d gets the lowest id.
    let da = edge(&mut ctx, d, a);
    let ab = edge(&mut ctx, a, b);
    let bc = edge(&mut ctx, b, c);
    let cd = edge(&mut ctx, c, d);
    let path = path_from_edges(&ctx, &[ab, bc, cd, da]).unwrap();
    assert_path(&ctx, &path, &[a, d, c, b], &[da, cd, bc, ab], true);
}

#[test]
fn triangle_is_the_smallest_closed_path() {
    let mut ctx = Context::new();
    let pts = [v(0.0, 0.0, 0.0), v(3.0, 0.0, 0.0), v(3.0, 4.0, 0.0)];
    let (vs, es) = chain(&mut ctx, &pts, true);
    let path = path_from_edges(&ctx, &es).unwrap();
    assert_path(&ctx, &path, &vs, &es, true);
    assert_near(path.length(), 12.0);
}

#[test]
fn closed_path_from_a_face_s_edges_matches_its_perimeter() {
    let mut ctx = Context::new();
    let (f, _, es) = square_face(&mut ctx, 2);
    let from_edges = path_from_edges(&ctx, &es).unwrap();
    let around = face_perimeter(&ctx, f).unwrap();
    assert!(from_edges == around || from_edges == around.reversed());
}

// ---- Path from edges: errors ----

#[test]
fn no_edges_is_an_error() {
    let ctx = Context::new();
    assert_eq!(path_from_edges(&ctx, &[]), Err(PathError::Empty));
}

#[test]
fn unknown_edge_is_reported_first_in_input_order() {
    let mut ctx = Context::new();
    let (_, es) = chain(&mut ctx, &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)], false);
    let result = path_from_edges(&ctx, &[es[0], EdgeId(900), EdgeId(500)]);
    assert_eq!(result, Err(PathError::UnknownEdge(EdgeId(900))));
}

#[test]
fn unknown_edge_comes_before_other_errors() {
    let mut ctx = Context::new();
    let (_, a) = chain(&mut ctx, &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)], false);
    let (_, b) = chain(&mut ctx, &[v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0)], false);
    let result = path_from_edges(&ctx, &[a[0], b[0], EdgeId(77)]);
    assert_eq!(result, Err(PathError::UnknownEdge(EdgeId(77))));
}

#[test]
fn three_edges_at_a_vertex_are_a_branch() {
    let mut ctx = Context::new();
    let hub = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let ends: Vec<VertexId> = [v(1.0, 0.0, 0.0), v(-1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)]
        .iter()
        .map(|p| vertex(&mut ctx, *p))
        .collect();
    let es: Vec<EdgeId> = ends.iter().map(|e| edge(&mut ctx, *e, hub)).collect();
    assert_eq!(path_from_edges(&ctx, &es), Err(PathError::Branch(hub)));
}

#[test]
fn branch_names_the_lowest_branching_vertex() {
    // An H: two hubs, each with three edges; the second hub has the lower id.
    let mut ctx = Context::new();
    let low = vertex(&mut ctx, v(1.0, 0.0, 0.0));
    let high = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let mut es = vec![edge(&mut ctx, high, low)];
    for p in [v(-1.0, 1.0, 0.0), v(-1.0, -1.0, 0.0)] {
        let o = vertex(&mut ctx, p);
        es.push(edge(&mut ctx, high, o));
    }
    for p in [v(2.0, 1.0, 0.0), v(2.0, -1.0, 0.0)] {
        let o = vertex(&mut ctx, p);
        es.push(edge(&mut ctx, o, low));
    }
    assert_eq!(path_from_edges(&ctx, &es), Err(PathError::Branch(low)));
}

#[test]
fn a_branch_is_only_counted_among_the_given_edges() {
    // The hub has three edges in the context, but only two are given.
    let mut ctx = Context::new();
    let hub = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let a = vertex(&mut ctx, v(1.0, 0.0, 0.0));
    let b = vertex(&mut ctx, v(-1.0, 0.0, 0.0));
    let c = vertex(&mut ctx, v(0.0, 1.0, 0.0));
    let ha = edge(&mut ctx, hub, a);
    let hb = edge(&mut ctx, hub, b);
    edge(&mut ctx, hub, c);
    let path = path_from_edges(&ctx, &[hb, ha]).unwrap();
    assert_path(&ctx, &path, &[a, hub, b], &[ha, hb], false);
}

#[test]
fn separate_pieces_are_a_gap() {
    let mut ctx = Context::new();
    let (_, a) = chain(&mut ctx, &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)], false);
    let (_, b) = chain(
        &mut ctx,
        &[v(0.0, 2.0, 0.0), v(1.0, 2.0, 0.0), v(1.0, 3.0, 0.0)],
        false,
    );
    let given = [a[0], b[0], b[1]];
    assert_eq!(
        path_from_edges(&ctx, &given),
        Err(PathError::Gap { pieces: 2 })
    );
}

#[test]
fn gap_counts_every_piece() {
    let mut ctx = Context::new();
    let mut given = Vec::new();
    for i in 0..3 {
        let y = i as f64;
        let (_, es) = chain(&mut ctx, &[v(0.0, y, 0.0), v(1.0, y, 0.0)], false);
        given.extend(es);
    }
    assert_eq!(
        path_from_edges(&ctx, &given),
        Err(PathError::Gap { pieces: 3 })
    );
}

#[test]
fn a_closed_loop_and_a_separate_edge_are_a_gap() {
    let mut ctx = Context::new();
    let pts = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)];
    let (_, mut given) = chain(&mut ctx, &pts, true);
    let (_, b) = chain(&mut ctx, &[v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0)], false);
    given.extend(b);
    assert_eq!(
        path_from_edges(&ctx, &given),
        Err(PathError::Gap { pieces: 2 })
    );
}

#[test]
fn edges_meeting_only_in_space_are_a_gap() {
    // Two edges whose ends lie at the same point but are different vertices.
    let mut ctx = Context::new();
    let (_, a) = chain(&mut ctx, &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)], false);
    let (_, b) = chain(&mut ctx, &[v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)], false);
    assert_eq!(
        path_from_edges(&ctx, &[a[0], b[0]]),
        Err(PathError::Gap { pieces: 2 })
    );
}

#[test]
fn branch_comes_before_gap() {
    let mut ctx = Context::new();
    let hub = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let mut given = Vec::new();
    for p in [v(1.0, 0.0, 0.0), v(-1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)] {
        let o = vertex(&mut ctx, p);
        given.push(edge(&mut ctx, hub, o));
    }
    let (_, far) = chain(&mut ctx, &[v(9.0, 0.0, 0.0), v(9.0, 1.0, 0.0)], false);
    given.extend(far);
    assert_eq!(path_from_edges(&ctx, &given), Err(PathError::Branch(hub)));
}

// ---- Path from edges: curves ----

#[test]
fn one_edge_of_an_arc_brings_the_whole_arc() {
    let mut ctx = Context::new();
    let (vs, es) = arc(&mut ctx, 0.0, FRAC_PI_2, 6);
    let path = path_from_edges(&ctx, &[es[3]]).unwrap();
    assert_path(&ctx, &path, &vs, &es, false);
    let chord = 2.0 * (FRAC_PI_2 / 12.0).sin();
    assert_near(path.length(), 6.0 * chord);
}

#[test]
fn one_edge_of_a_circle_brings_the_whole_circle() {
    let mut ctx = Context::new();
    let points = arc_points(0.0, TAU, 12);
    // The last point repeats the first: make the closed chain of the first 12.
    let (vs, es) = chain(&mut ctx, &points[..12], true);
    curve::add_curve(&mut ctx, &points[..12], true, &mut Edit::new()).expect("circle curve");
    let path = path_from_edges(&ctx, &[es[7]]).unwrap();
    assert_path(&ctx, &path, &vs, &es, true);
    assert_near(path.length(), 12.0 * 2.0 * (TAU / 24.0).sin());
}

#[test]
fn arc_and_lines_make_one_chain() {
    // A line into the arc's start and a line out of its end; given: both lines and one step.
    let mut ctx = Context::new();
    let (avs, aes) = arc(&mut ctx, 0.0, FRAC_PI_2, 4);
    let before = vertex(&mut ctx, v(1.0, -1.0, 0.0));
    let after = vertex(&mut ctx, v(-1.0, 1.0, 0.0));
    let l1 = edge(&mut ctx, before, avs[0]);
    let l2 = edge(&mut ctx, avs[4], after);
    let path = path_from_edges(&ctx, &[l2, aes[1], l1]).unwrap();
    // Ends: `before` (id 5) and `after` (id 6): starts at `before`.
    let mut vs = vec![before];
    vs.extend(&avs);
    vs.push(after);
    let mut es = vec![l1];
    es.extend(&aes);
    es.push(l2);
    assert_path(&ctx, &path, &vs, &es, false);
}

#[test]
fn a_line_touching_an_arc_does_not_bring_the_arc() {
    let mut ctx = Context::new();
    let (avs, _) = arc(&mut ctx, 0.0, FRAC_PI_2, 4);
    let out = vertex(&mut ctx, v(2.0, 0.0, 0.0));
    let line = edge(&mut ctx, avs[0], out);
    let path = path_from_edges(&ctx, &[line]).unwrap();
    assert_path(&ctx, &path, &[avs[0], out], &[line], false);
}

#[test]
fn an_arc_s_steps_can_make_a_branch() {
    // A line from the arc's middle vertex; given: the line and one step of the arc.
    let mut ctx = Context::new();
    let (avs, aes) = arc(&mut ctx, 0.0, FRAC_PI_2, 4);
    let out = vertex(&mut ctx, v(3.0, 3.0, 0.0));
    let line = edge(&mut ctx, avs[2], out);
    assert_eq!(
        path_from_edges(&ctx, &[line, aes[0]]),
        Err(PathError::Branch(avs[2]))
    );
}

#[test]
fn two_arcs_each_given_by_one_step() {
    // Two arcs meeting end to end make one path when one step of each is given.
    let mut ctx = Context::new();
    let (avs, aes) = arc(&mut ctx, 0.0, FRAC_PI_2, 3);
    // The second arc bends the other way, about (0, 2, 0), starting at the first's end.
    let pts: Vec<DVec3> = arc_points(-FRAC_PI_2, -PI, 3)
        .iter()
        .map(|p| v(-p.x, p.y + 2.0, 0.0))
        .collect();
    // pts[0] is (0, 1, 0), the first arc's end vertex.
    let mut bvs = vec![avs[3]];
    for p in &pts[1..] {
        bvs.push(vertex(&mut ctx, *p));
    }
    let bes: Vec<EdgeId> = bvs.windows(2).map(|w| edge(&mut ctx, w[0], w[1])).collect();
    curve::add_curve(&mut ctx, &pts, false, &mut Edit::new()).expect("second arc");
    let path = path_from_edges(&ctx, &[bes[2], aes[1]]).unwrap();
    let mut vs = avs.clone();
    vs.extend(&bvs[1..]);
    let mut es = aes.clone();
    es.extend(&bes);
    assert_path(&ctx, &path, &vs, &es, false);
}

// ---- Path around a face ----

#[test]
fn face_perimeter_follows_the_outer_loop_from_its_lowest_vertex() {
    let mut ctx = Context::new();
    // The loop is given starting at corner 2; the path starts at corner 0 anyway.
    let (f, vs, es) = square_face(&mut ctx, 2);
    let path = face_perimeter(&ctx, f).unwrap();
    assert_path(&ctx, &path, &vs, &es, true);
    assert_near(path.length(), 4.0);
}

#[test]
fn face_perimeter_keeps_the_loop_direction() {
    // A square whose loop runs corner 0 -> 3 -> 2 -> 1 (its front faces -Z), using each edge
    // backward.
    let mut ctx = Context::new();
    let corners = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let (vs, es) = chain(&mut ctx, &corners, true);
    let uses = [3, 2, 1, 0]
        .iter()
        .map(|i| EdgeUse::backward(es[*i]))
        .collect();
    let face = Face {
        outer: Loop { uses },
        inner: Vec::new(),
    };
    let f = ctx.insert_face(face, &mut Edit::new()).unwrap();
    let path = face_perimeter(&ctx, f).unwrap();
    assert_path(
        &ctx,
        &path,
        &[vs[0], vs[3], vs[2], vs[1]],
        &[es[3], es[2], es[1], es[0]],
        true,
    );
}

#[test]
fn face_perimeter_starts_at_the_lowest_vertex_wherever_it_lies() {
    // Corner ids: the corner at (1, 1) is inserted first.
    let mut ctx = Context::new();
    let c2 = vertex(&mut ctx, v(1.0, 1.0, 0.0));
    let c0 = vertex(&mut ctx, v(0.0, 0.0, 0.0));
    let c1 = vertex(&mut ctx, v(1.0, 0.0, 0.0));
    let c3 = vertex(&mut ctx, v(0.0, 1.0, 0.0));
    let e01 = edge(&mut ctx, c0, c1);
    let e12 = edge(&mut ctx, c1, c2);
    let e23 = edge(&mut ctx, c2, c3);
    let e30 = edge(&mut ctx, c3, c0);
    let uses = [e01, e12, e23, e30]
        .iter()
        .map(|e| EdgeUse::forward(*e))
        .collect();
    let face = Face {
        outer: Loop { uses },
        inner: Vec::new(),
    };
    let f = ctx.insert_face(face, &mut Edit::new()).unwrap();
    let path = face_perimeter(&ctx, f).unwrap();
    assert_path(&ctx, &path, &[c2, c3, c0, c1], &[e23, e30, e01, e12], true);
}

#[test]
fn face_perimeter_ignores_holes() {
    let mut ctx = Context::new();
    let outer = [
        v(0.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 4.0, 0.0),
        v(0.0, 4.0, 0.0),
    ];
    let (vs, es) = chain(&mut ctx, &outer, true);
    // The hole runs clockwise seen from +Z.
    let hole = [
        v(1.0, 1.0, 0.0),
        v(1.0, 3.0, 0.0),
        v(3.0, 3.0, 0.0),
        v(3.0, 1.0, 0.0),
    ];
    let (_, hes) = chain(&mut ctx, &hole, true);
    let face = Face {
        outer: Loop {
            uses: es.iter().map(|e| EdgeUse::forward(*e)).collect(),
        },
        inner: vec![Loop {
            uses: hes.iter().map(|e| EdgeUse::forward(*e)).collect(),
        }],
    };
    let f = ctx.insert_face(face, &mut Edit::new()).unwrap();
    let path = face_perimeter(&ctx, f).unwrap();
    assert_path(&ctx, &path, &vs, &es, true);
    assert_near(path.length(), 16.0);
}

#[test]
fn face_perimeter_of_a_circle_face() {
    let mut ctx = Context::new();
    let points = arc_points(0.0, TAU, 16);
    let (vs, es) = chain(&mut ctx, &points[..16], true);
    curve::add_curve(&mut ctx, &points[..16], true, &mut Edit::new()).expect("circle curve");
    let face = Face {
        outer: Loop {
            uses: es.iter().map(|e| EdgeUse::forward(*e)).collect(),
        },
        inner: Vec::new(),
    };
    let f = ctx.insert_face(face, &mut Edit::new()).unwrap();
    let path = face_perimeter(&ctx, f).unwrap();
    assert_path(&ctx, &path, &vs, &es, true);
}

#[test]
fn face_perimeter_of_an_unknown_face_is_an_error() {
    let ctx = Context::new();
    assert_eq!(
        face_perimeter(&ctx, FaceId(3)),
        Err(PathError::UnknownFace(FaceId(3)))
    );
}

// ---- Reversing ----

#[test]
fn reversed_open_path_runs_from_the_other_end() {
    let mut ctx = Context::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(1.0, 1.0, 1.0),
    ];
    let (vs, es) = chain(&mut ctx, &pts, false);
    let back = path_from_edges(&ctx, &es).unwrap().reversed();
    let rvs: Vec<VertexId> = vs.iter().rev().copied().collect();
    let res: Vec<EdgeId> = es.iter().rev().copied().collect();
    assert_path(&ctx, &back, &rvs, &res, false);
    assert_near(back.length(), 3.0);
}

#[test]
fn reversed_closed_path_keeps_its_first_vertex() {
    let mut ctx = Context::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let (vs, es) = chain(&mut ctx, &pts, true);
    let back = path_from_edges(&ctx, &es).unwrap().reversed();
    assert_path(
        &ctx,
        &back,
        &[vs[0], vs[3], vs[2], vs[1]],
        &[es[3], es[2], es[1], es[0]],
        true,
    );
}

#[test]
fn reversing_twice_gives_the_same_path() {
    let mut ctx = Context::new();
    let (f, _, _) = square_face(&mut ctx, 1);
    let path = face_perimeter(&ctx, f).unwrap();
    assert_eq!(path.reversed().reversed(), path);
}
