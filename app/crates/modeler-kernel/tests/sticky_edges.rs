// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::DVec3;
use modeler_kernel::geom::{self, LINEAR_TOL};
use modeler_kernel::sticky::*;
use modeler_kernel::topology::*;

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn pos(ctx: &Context, v: VertexId) -> DVec3 {
    ctx.vertex(v).unwrap().position
}

/// The vertex at `q` (within 1e-9), panicking if there is none.
fn vat(ctx: &Context, q: DVec3) -> VertexId {
    ctx.find_vertex(q, 1e-9)
        .unwrap_or_else(|| panic!("no vertex at {q}"))
}

fn has_edge(ctx: &Context, a: DVec3, b: DVec3) -> bool {
    match (ctx.find_vertex(a, 1e-9), ctx.find_vertex(b, 1e-9)) {
        (Some(va), Some(vb)) => ctx.edge_between(va, vb).is_some(),
        _ => false,
    }
}

fn counts(ctx: &Context) -> (usize, usize, usize) {
    (ctx.vertex_count(), ctx.edge_count(), ctx.face_count())
}

fn assert_valid(ctx: &Context) {
    assert_eq!(ctx.validate(), vec![], "context: {ctx:#?}");
}

/// `log` (the records of one call, starting from `before`) undoes to `before` and redoes to
/// `after`.
fn assert_undo_redo(before: &Context, after: &Context, log: &Edit) {
    let mut undone = after.clone();
    undone.apply_edit(&log.inverse()).unwrap();
    assert_eq!(&undone, before, "undo");
    let mut redone = before.clone();
    redone.apply_edit(log).unwrap();
    assert_eq!(&redone, after, "redo");
}

/// The chain is consistent, runs from `a` to `b` and stays on the segment.
fn assert_chain(ctx: &Context, chain: &EdgeChain, a: DVec3, b: DVec3) {
    assert_eq!(chain.vertices.len(), chain.edges.len() + 1);
    assert!(!chain.edges.is_empty());
    for (i, e) in chain.edges.iter().enumerate() {
        let ends = ctx.edge(*e).unwrap().ends;
        let (u, v) = (chain.vertices[i], chain.vertices[i + 1]);
        assert!(
            ends == [u, v] || ends == [v, u],
            "edge {i} doesn't join its vertices"
        );
    }
    assert!(pos(ctx, chain.vertices[0]).distance(a) <= LINEAR_TOL);
    assert!(pos(ctx, *chain.vertices.last().unwrap()).distance(b) <= LINEAR_TOL);
    let dir = b - a;
    let mut last = f64::NEG_INFINITY;
    for v in &chain.vertices {
        let q = pos(ctx, *v);
        assert!(geom::point_segment_distance(q, a, b) <= LINEAR_TOL);
        let t = (q - a).dot(dir);
        assert!(t > last, "chain vertices out of order");
        last = t;
    }
}

/// Run `add_edge` on a clone, check validity, chain and undo/redo, and return the result.
fn draw(ctx: &mut Context, a: DVec3, b: DVec3) -> (EdgeChain, Edit) {
    let before = ctx.clone();
    let mut log = Edit::new();
    let chain = ctx.add_edge(a, b, &mut log).unwrap();
    assert_valid(ctx);
    assert_chain(ctx, &chain, a, b);
    assert_undo_redo(&before, ctx, &log);
    (chain, log)
}

/// A face built with primitives from the polygon (counter-clockwise seen from the front).
/// Edge `i` joins point `i` to point `i + 1`; when `flip` is set every edge is stored
/// backwards and used reversed.
fn polygon_face(ctx: &mut Context, pts: &[DVec3], flip: bool) -> (FaceId, Vec<EdgeId>) {
    let mut log = Edit::new();
    let vs: Vec<_> = pts
        .iter()
        .map(|q| ctx.insert_vertex(*q, &mut log))
        .collect();
    let n = vs.len();
    let mut es = vec![];
    let mut uses = vec![];
    for i in 0..n {
        let (s, t) = (vs[i], vs[(i + 1) % n]);
        if flip {
            let e = ctx.insert_edge(t, s, &mut log).unwrap();
            es.push(e);
            uses.push(EdgeUse::backward(e));
        } else {
            let e = ctx.insert_edge(s, t, &mut log).unwrap();
            es.push(e);
            uses.push(EdgeUse::forward(e));
        }
    }
    let f = ctx
        .insert_face(
            Face {
                outer: Loop { uses },
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    (f, es)
}

fn square(x0: f64, y0: f64, s: f64, z: f64) -> Vec<DVec3> {
    vec![
        p(x0, y0, z),
        p(x0 + s, y0, z),
        p(x0 + s, y0 + s, z),
        p(x0, y0 + s, z),
    ]
}

/// The face's outer loop points, in loop order.
fn outer_points(ctx: &Context, f: FaceId) -> Vec<DVec3> {
    ctx.loop_points(&ctx.face(f).unwrap().outer)
}

/// True when `pts` (a closed loop) visits `q`, and `q` is between `before` and `after`.
fn loop_has_between(pts: &[DVec3], before: DVec3, q: DVec3, after: DVec3) -> bool {
    let n = pts.len();
    (0..n).any(|i| {
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        pts[i].distance(q) < 1e-9
            && ((prev.distance(before) < 1e-9 && next.distance(after) < 1e-9)
                || (prev.distance(after) < 1e-9 && next.distance(before) < 1e-9))
    })
}

// -- add_edge: basics ----------------------------------------------------------------

#[test]
fn first_edge_in_empty_context() {
    let mut ctx = Context::new();
    let (chain, log) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 2.0, 3.0));
    assert_eq!(counts(&ctx), (2, 1, 0));
    assert_eq!(chain.edges.len(), 1);
    assert_eq!(pos(&ctx, chain.vertices[0]), p(0.0, 0.0, 0.0));
    assert_eq!(pos(&ctx, chain.vertices[1]), p(1.0, 2.0, 3.0));
    assert_eq!(
        ctx.edge(chain.edges[0]).unwrap().ends,
        [chain.vertices[0], chain.vertices[1]]
    );
    assert_eq!(log.records.len(), 3);
}

#[test]
fn log_is_appended_to() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    ctx.add_edge(p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), &mut log)
        .unwrap();
    let n = log.records.len();
    ctx.add_edge(p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), &mut log)
        .unwrap();
    assert!(log.records.len() > n);
    ctx.apply_edit(&log.inverse()).unwrap();
    assert_eq!(ctx, Context::new());
}

#[test]
fn degenerate_and_non_finite_change_nothing() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    ctx.add_edge(p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), &mut log)
        .unwrap();
    let before = ctx.clone();
    let n = log.records.len();
    let cases = [
        (p(2.0, 2.0, 2.0), p(2.0, 2.0, 2.0), StickyError::Degenerate),
        (
            p(2.0, 2.0, 2.0),
            p(2.0, 2.0, 2.0 + 5e-6),
            StickyError::Degenerate,
        ),
        (
            p(f64::NAN, 0.0, 0.0),
            p(1.0, 1.0, 1.0),
            StickyError::NotFinite,
        ),
        (
            p(0.0, 0.0, 0.0),
            p(1.0, f64::INFINITY, 0.0),
            StickyError::NotFinite,
        ),
        // 12 um apart (more than the tolerance), but both snap to the vertex at (1, 0, 0).
        (
            p(1.0 + 6e-6, 0.0, 0.0),
            p(1.0 - 6e-6, 0.0, 0.0),
            StickyError::Degenerate,
        ),
    ];
    for (a, b, err) in cases {
        assert_eq!(ctx.add_edge(a, b, &mut log), Err(err.clone()), "{a} {b}");
        assert_eq!(ctx, before);
        assert_eq!(log.records.len(), n);
    }
}

#[test]
fn degenerate_after_snapping_to_an_edge_rolls_back() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    ctx.add_edge(p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0), &mut log)
        .unwrap();
    let before = ctx.clone();
    let n = log.records.len();
    // `a` splits the edge at (1, 0, 0); `b` then snaps to that new vertex.
    let r = ctx.add_edge(p(1.0, 6e-6, 0.0), p(1.0 + 6e-6, -6e-6, 0.0), &mut log);
    assert_eq!(r, Err(StickyError::Degenerate));
    assert_eq!(ctx, before);
    assert_eq!(log.records.len(), n);
}

#[test]
fn shared_end_vertex() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(counts(&ctx), (3, 2, 0));
    assert_eq!(chain.vertices[0], vat(&ctx, p(1.0, 0.0, 0.0)));
}

#[test]
fn end_snaps_to_vertex_within_tolerance() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0 + 5e-6, 3e-6, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(counts(&ctx), (3, 2, 0));
    // The existing vertex doesn't move.
    assert_eq!(pos(&ctx, chain.vertices[0]), p(1.0, 0.0, 0.0));
}

#[test]
fn end_beyond_tolerance_is_a_new_vertex() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0 + 3e-5, 0.0, 0.0), p(1.0 + 3e-5, 1.0, 0.0));
    assert_eq!(counts(&ctx), (4, 2, 0));
    assert_eq!(pos(&ctx, chain.vertices[0]), p(1.0 + 3e-5, 0.0, 0.0));
}

#[test]
fn same_edge_again_records_nothing() {
    let mut ctx = Context::new();
    let (first, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    let before = ctx.clone();
    let (again, log) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert!(log.is_empty());
    assert_eq!(ctx, before);
    assert_eq!(again, first);
    let (rev, log) = draw(&mut ctx, p(1.0, 1.0, 0.0), p(0.0, 0.0, 0.0));
    assert!(log.is_empty());
    assert_eq!(rev.edges, first.edges);
    assert_eq!(rev.vertices, vec![first.vertices[1], first.vertices[0]]);
}

// -- add_edge: crossings and T-junctions -------------------------------------------------

#[test]
fn crossing_splits_both() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, -1.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(counts(&ctx), (5, 4, 0));
    assert_eq!(chain.edges.len(), 2);
    let mid = vat(&ctx, p(1.0, 0.0, 0.0));
    assert_eq!(chain.vertices[1], mid);
    assert_eq!(ctx.edges_of_vertex(mid).len(), 4);
    assert!(has_edge(&ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)));
    assert!(has_edge(&ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0)));
}

#[test]
fn crossing_on_a_tilted_plane() {
    let mut ctx = Context::new();
    let o = p(0.3, -0.2, 0.7);
    let u = DVec3::new(1.0, 2.0, 2.0) / 3.0;
    let v = DVec3::new(2.0, 1.0, -2.0) / 3.0;
    draw(&mut ctx, o, o + u * 4.0);
    let (chain, _) = draw(&mut ctx, o + u + v, o + u * 3.0 - v);
    assert_eq!(counts(&ctx), (5, 4, 0));
    assert_eq!(chain.edges.len(), 2);
    assert!(pos(&ctx, chain.vertices[1]).distance(o + u * 2.0) < 1e-9);
}

#[test]
fn skew_edges_stay_apart() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(1.0, -1.0, 1.0), p(1.0, 1.0, 1.0));
    assert_eq!(counts(&ctx), (4, 2, 0));
}

#[test]
fn nearly_crossing_skew_edges_split_on_the_old_edge() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, -1.0, 4e-6), p(1.0, 1.0, 4e-6));
    assert_eq!(counts(&ctx), (5, 4, 0));
    // The crossing vertex is on the old edge.
    assert!(pos(&ctx, chain.vertices[1]).distance(p(1.0, 0.0, 0.0)) < 1e-9);
}

#[test]
fn t_junction_from_new_end() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    assert_eq!(chain.edges.len(), 1);
    assert_eq!(chain.vertices[0], vat(&ctx, p(1.0, 0.0, 0.0)));
}

#[test]
fn t_junction_end_near_edge_lands_on_it() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, 1.0, 0.0), p(1.25, 7e-6, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    let end = pos(&ctx, chain.vertices[1]);
    assert!(end.distance(p(1.25, 0.0, 0.0)) < 1e-9, "{end}");
}

#[test]
fn snaps_to_the_closest_of_two_edges() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(0.0, 1.5e-5, 0.0), p(2.0, 1.5e-5, 0.0));
    // Two parallel edges 15 um apart. The point is 6 um from the first and 9 um from the
    // second: both are within tolerance, and the closer one is split.
    let (chain, _) = draw(&mut ctx, p(1.0, 6e-6, 0.0), p(1.0, -1.0, 0.0));
    assert!(pos(&ctx, chain.vertices[0]).distance(p(1.0, 0.0, 0.0)) < 1e-9);
    assert_eq!(counts(&ctx), (6, 4, 0));
}

#[test]
fn t_junction_at_old_end() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    assert_eq!(chain.edges.len(), 2);
    assert_eq!(chain.vertices[1], vat(&ctx, p(1.0, 0.0, 0.0)));
}

#[test]
fn passes_through_a_vertex_within_tolerance() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(1.0, 6e-6, 0.0), p(1.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    assert_eq!(pos(&ctx, chain.vertices[1]), p(1.0, 6e-6, 0.0));
}

#[test]
fn tic_tac_toe_grid() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 3.0, 0.0));
    draw(&mut ctx, p(2.0, 0.0, 0.0), p(2.0, 3.0, 0.0));
    let (c1, _) = draw(&mut ctx, p(0.0, 1.0, 0.0), p(3.0, 1.0, 0.0));
    let (c2, _) = draw(&mut ctx, p(3.0, 2.0, 0.0), p(0.0, 2.0, 0.0));
    assert_eq!(c1.edges.len(), 3);
    assert_eq!(c2.edges.len(), 3);
    // The centre square closes, so it becomes a face (m1-sticky-faces).
    assert_eq!(counts(&ctx), (12, 12, 1));
    assert_eq!(c2.faces.len(), 1);
    for (x, y) in [(1.0, 1.0), (2.0, 1.0), (1.0, 2.0), (2.0, 2.0)] {
        assert_eq!(ctx.edges_of_vertex(vat(&ctx, p(x, y, 0.0))).len(), 4);
    }
}

#[test]
fn star_through_one_point() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(-1.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    draw(&mut ctx, p(0.0, -1.0, 0.0), p(0.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(-1.0, -1.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(chain.edges.len(), 2);
    draw(&mut ctx, p(0.0, 0.0, -1.0), p(0.0, 0.0, 1.0));
    let o = vat(&ctx, p(0.0, 0.0, 0.0));
    assert_eq!(ctx.edges_of_vertex(o).len(), 8);
    assert_eq!(counts(&ctx), (9, 8, 0));
}

// -- add_edge: collinear overlaps -------------------------------------------------------

#[test]
fn partial_overlap() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(3.0, 0.0, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    assert_eq!(chain.edges.len(), 2);
    for x in [0.0, 1.0, 2.0] {
        assert!(has_edge(&ctx, p(x, 0.0, 0.0), p(x + 1.0, 0.0, 0.0)));
    }
}

#[test]
fn overlap_inside_an_edge() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.5, 0.0, 0.0), p(0.5, 0.0, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    assert_eq!(chain.edges.len(), 1);
    assert_eq!(pos(&ctx, chain.vertices[0]), p(1.5, 0.0, 0.0));
    assert_eq!(pos(&ctx, chain.vertices[1]), p(0.5, 0.0, 0.0));
}

#[test]
fn covering_an_edge_reuses_it() {
    let mut ctx = Context::new();
    let (old, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(3.0, 0.0, 0.0), p(0.0, 0.0, 0.0));
    assert_eq!(counts(&ctx), (4, 3, 0));
    assert_eq!(chain.edges.len(), 3);
    assert_eq!(chain.edges[1], old.edges[0]);
}

#[test]
fn overlap_across_several_edges_and_a_gap() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(3.0, 0.0, 0.0), p(4.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(0.5, 0.0, 0.0), p(3.5, 0.0, 0.0));
    // Vertices 0, 0.5, 1, 2, 3, 3.5, 4; edges between each consecutive pair.
    assert_eq!(counts(&ctx), (7, 6, 0));
    assert_eq!(chain.edges.len(), 4);
}

#[test]
fn overlap_and_crossing_together() {
    let mut ctx = Context::new();
    draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(3.0, -1.0, 0.0), p(3.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(4.0, 0.0, 0.0));
    // 0, 1, 2, 3 (crossing), 4 on the x axis plus the vertical's two ends.
    assert_eq!(counts(&ctx), (7, 6, 0));
    assert_eq!(chain.edges.len(), 3);
}

// -- add_edge: faces keep their shape ---------------------------------------------------

fn t_into_face_edge(flip: bool) {
    let mut ctx = Context::new();
    let (f, _) = polygon_face(&mut ctx, &square(0.0, 0.0, 2.0, 0.0), flip);
    assert_valid(&ctx);
    let area = geom::area_vector(&outer_points(&ctx, f));
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, -1.0, 0.0));
    assert_eq!(counts(&ctx), (6, 6, 1));
    let face = ctx.face(f).unwrap();
    assert_eq!(face.outer.uses.len(), 5);
    let pts = outer_points(&ctx, f);
    assert!(loop_has_between(
        &pts,
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(2.0, 0.0, 0.0)
    ));
    assert!((geom::area_vector(&pts) - area).length() < 1e-12);
}

#[test]
fn t_into_face_edge_keeps_face() {
    t_into_face_edge(false);
}

#[test]
fn t_into_reversed_face_edge_keeps_face() {
    t_into_face_edge(true);
}

#[test]
fn edge_through_shared_face_edge_splits_both_faces_loops() {
    let mut ctx = Context::new();
    // Two squares sharing the edge x = 1, built by hand.
    let mut log = Edit::new();
    let pts = [
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(2.0, 0.0, 0.0),
        p(2.0, 1.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
    ];
    let v: Vec<_> = pts
        .iter()
        .map(|q| ctx.insert_vertex(*q, &mut log))
        .collect();
    let mut e =
        |a: usize, b: usize, ctx: &mut Context| ctx.insert_edge(v[a], v[b], &mut log).unwrap();
    let e01 = e(0, 1, &mut ctx);
    let e12 = e(1, 2, &mut ctx);
    let e23 = e(2, 3, &mut ctx);
    let e34 = e(3, 4, &mut ctx);
    let e45 = e(4, 5, &mut ctx);
    let e50 = e(5, 0, &mut ctx);
    let e14 = e(1, 4, &mut ctx);
    let mut log = Edit::new();
    let left = ctx
        .insert_face(
            Face {
                outer: Loop {
                    uses: vec![
                        EdgeUse::forward(e01),
                        EdgeUse::forward(e14),
                        EdgeUse::forward(e45),
                        EdgeUse::forward(e50),
                    ],
                },
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    let right = ctx
        .insert_face(
            Face {
                outer: Loop {
                    uses: vec![
                        EdgeUse::forward(e12),
                        EdgeUse::forward(e23),
                        EdgeUse::forward(e34),
                        EdgeUse::backward(e14),
                    ],
                },
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    assert_valid(&ctx);
    // A vertical edge piercing the shared edge at its middle.
    draw(&mut ctx, p(1.0, 0.5, -1.0), p(1.0, 0.5, 1.0));
    assert_eq!(counts(&ctx), (9, 10, 2));
    let mid = p(1.0, 0.5, 0.0);
    for f in [left, right] {
        let pts = outer_points(&ctx, f);
        assert_eq!(pts.len(), 5);
        assert!(loop_has_between(
            &pts,
            p(1.0, 0.0, 0.0),
            mid,
            p(1.0, 1.0, 0.0)
        ));
    }
    assert!(ctx.edge(e14).is_none());
}

#[test]
fn edge_through_face_interior_leaves_it_alone() {
    let mut ctx = Context::new();
    let (f, _) = polygon_face(&mut ctx, &square(0.0, 0.0, 2.0, 0.0), false);
    let before = ctx.face(f).unwrap().clone();
    draw(&mut ctx, p(1.0, 1.0, -1.0), p(1.0, 1.0, 1.0));
    assert_eq!(counts(&ctx), (6, 5, 1));
    assert_eq!(ctx.face(f).unwrap(), &before);
}

#[test]
fn edge_along_face_edge_splits_it_in_the_loop() {
    let mut ctx = Context::new();
    let (f, _) = polygon_face(&mut ctx, &square(0.0, 0.0, 3.0, 0.0), true);
    // Runs along the middle third of the bottom edge.
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    assert_eq!(counts(&ctx), (6, 6, 1));
    let pts = outer_points(&ctx, f);
    assert_eq!(pts.len(), 6);
    assert!(loop_has_between(
        &pts,
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(2.0, 0.0, 0.0)
    ));
    assert!(loop_has_between(
        &pts,
        p(1.0, 0.0, 0.0),
        p(2.0, 0.0, 0.0),
        p(3.0, 0.0, 0.0)
    ));
}

// -- split_edge -------------------------------------------------------------------------

#[test]
fn split_edge_in_the_middle() {
    let mut ctx = Context::new();
    let (c, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let before = ctx.clone();
    let mut log = Edit::new();
    // The closest point on the edge to (0.5, 1, 1) is (0.5, 0, 0).
    let m = ctx
        .split_edge(c.edges[0], p(0.5, 1.0, 1.0), &mut log)
        .unwrap();
    assert_eq!(pos(&ctx, m), p(0.5, 0.0, 0.0));
    assert_eq!(counts(&ctx), (3, 2, 0));
    assert!(ctx.edge(c.edges[0]).is_none());
    assert_eq!(ctx.edges_of_vertex(m).len(), 2);
    // Piece 1 keeps the old edge's first end, piece 2 its second.
    let es = ctx.edges_of_vertex(m);
    let ends: Vec<_> = es.iter().map(|e| ctx.edge(*e).unwrap().ends).collect();
    assert!(ends.contains(&[c.vertices[0], m]));
    assert!(ends.contains(&[m, c.vertices[1]]));
    assert_valid(&ctx);
    assert_undo_redo(&before, &ctx, &log);
}

#[test]
fn split_edge_at_an_end_changes_nothing() {
    let mut ctx = Context::new();
    let (c, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let before = ctx.clone();
    let mut log = Edit::new();
    let e = c.edges[0];
    assert_eq!(
        ctx.split_edge(e, p(2.0 - 5e-6, 0.0, 0.0), &mut log),
        Ok(c.vertices[1])
    );
    assert_eq!(
        ctx.split_edge(e, p(-3.0, 0.0, 0.0), &mut log),
        Ok(c.vertices[0])
    );
    assert_eq!(
        ctx.split_edge(e, p(5.0, 1.0, 0.0), &mut log),
        Ok(c.vertices[1])
    );
    assert!(log.is_empty());
    assert_eq!(ctx, before);
}

#[test]
fn split_edge_errors() {
    let mut ctx = Context::new();
    let (c, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    let before = ctx.clone();
    let mut log = Edit::new();
    assert_eq!(
        ctx.split_edge(EdgeId(99), p(1.0, 0.0, 0.0), &mut log),
        Err(StickyError::MissingEdge(EdgeId(99)))
    );
    assert_eq!(
        ctx.split_edge(c.edges[0], p(f64::NAN, 0.0, 0.0), &mut log),
        Err(StickyError::NotFinite)
    );
    assert!(log.is_empty());
    assert_eq!(ctx, before);
}

#[test]
fn split_edge_updates_face_loops_in_place() {
    for flip in [false, true] {
        let mut ctx = Context::new();
        let (f, es) = polygon_face(&mut ctx, &square(0.0, 0.0, 2.0, 0.0), flip);
        let before = ctx.clone();
        let mut log = Edit::new();
        // Edge 1 runs (2,0)-(2,2).
        let m = ctx.split_edge(es[1], p(2.0, 0.5, 0.0), &mut log).unwrap();
        assert_valid(&ctx);
        let face = ctx.face(f).unwrap();
        assert_eq!(face.outer.uses.len(), 5);
        // The pieces sit where the old use was: uses 1 and 2.
        let vs = ctx.loop_vertices(&face.outer);
        assert_eq!(vs[2], m);
        assert_eq!(face.outer.uses[0].edge, es[0]);
        assert_eq!(face.outer.uses[3].edge, es[2]);
        assert_eq!(face.outer.uses[1].reversed, flip);
        assert_eq!(face.outer.uses[2].reversed, flip);
        assert_undo_redo(&before, &ctx, &log);
    }
}

// -- erase_edge -------------------------------------------------------------------------

fn erase(ctx: &mut Context, e: EdgeId) -> Edit {
    let before = ctx.clone();
    let mut log = Edit::new();
    ctx.erase_edge(e, &mut log).unwrap();
    assert_valid(ctx);
    assert_undo_redo(&before, ctx, &log);
    log
}

#[test]
fn erase_only_edge_empties_context() {
    let mut ctx = Context::new();
    let (c, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    erase(&mut ctx, c.edges[0]);
    assert_eq!(ctx, Context::new());
}

#[test]
fn erase_keeps_shared_vertex_and_collinear_neighbours() {
    let mut ctx = Context::new();
    let (c, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(3.0, 0.0, 0.0));
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    draw(&mut ctx, p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0));
    assert!(ctx.edge(c.edges[0]).is_none());
    let v1 = vat(&ctx, p(1.0, 0.0, 0.0));
    let v2 = vat(&ctx, p(2.0, 0.0, 0.0));
    let middle = ctx.edge_between(v1, v2).unwrap();
    erase(&mut ctx, middle);
    assert_eq!(counts(&ctx), (6, 4, 0));
    let up = ctx.edge_between(v1, vat(&ctx, p(1.0, 1.0, 0.0))).unwrap();
    erase(&mut ctx, up);
    // (1,0,0) still joins two collinear edges; they are not merged.
    assert_eq!(counts(&ctx), (5, 3, 0));
    assert!(has_edge(&ctx, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)));
    assert_eq!(ctx.edges_of_vertex(v1).len(), 1);
}

#[test]
fn erase_missing_edge() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    assert_eq!(
        ctx.erase_edge(EdgeId(3), &mut log),
        Err(StickyError::MissingEdge(EdgeId(3)))
    );
    assert!(log.is_empty());
}

#[test]
fn erase_face_edge_removes_face() {
    let mut ctx = Context::new();
    let (f, es) = polygon_face(&mut ctx, &square(0.0, 0.0, 1.0, 0.0), false);
    erase(&mut ctx, es[2]);
    assert!(ctx.face(f).is_none());
    assert_eq!(counts(&ctx), (4, 3, 0));
}

#[test]
fn erase_box_edge_removes_both_faces() {
    // Two faces of a box meeting at a right angle along the edge (0,0,0)-(1,0,0).
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let v: Vec<_> = [
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
        p(1.0, 0.0, 1.0),
        p(0.0, 0.0, 1.0),
    ]
    .iter()
    .map(|q| ctx.insert_vertex(*q, &mut log))
    .collect();
    let mut e =
        |a: usize, b: usize, ctx: &mut Context| ctx.insert_edge(v[a], v[b], &mut log).unwrap();
    let e01 = e(0, 1, &mut ctx);
    let e12 = e(1, 2, &mut ctx);
    let e23 = e(2, 3, &mut ctx);
    let e30 = e(3, 0, &mut ctx);
    let e14 = e(1, 4, &mut ctx);
    let e45 = e(4, 5, &mut ctx);
    let e50 = e(5, 0, &mut ctx);
    let mut log = Edit::new();
    // Bottom faces down (clockwise seen from above), front faces -y.
    let bottom = Face {
        outer: Loop {
            uses: vec![
                EdgeUse::forward(e01),
                EdgeUse::forward(e12),
                EdgeUse::forward(e23),
                EdgeUse::forward(e30),
            ]
            .into_iter()
            .rev()
            .map(|u| EdgeUse {
                edge: u.edge,
                reversed: true,
            })
            .collect(),
        },
        inner: vec![],
    };
    let front = Face {
        outer: Loop {
            uses: vec![
                EdgeUse::forward(e01),
                EdgeUse::forward(e14),
                EdgeUse::forward(e45),
                EdgeUse::forward(e50),
            ],
        },
        inner: vec![],
    };
    let fb = ctx.insert_face(bottom, &mut log).unwrap();
    let ff = ctx.insert_face(front, &mut log).unwrap();
    assert_valid(&ctx);
    erase(&mut ctx, e01);
    assert!(ctx.face(fb).is_none());
    assert!(ctx.face(ff).is_none());
    assert_eq!(counts(&ctx), (6, 6, 0));
}

/// A 3x3 square with a 1x1 hole, and a face filling the hole.
fn face_with_filled_hole(ctx: &mut Context) -> (FaceId, FaceId, Vec<EdgeId>) {
    let (outer, _) = polygon_face(ctx, &square(0.0, 0.0, 3.0, 0.0), false);
    let (fill, hole_edges) = polygon_face(ctx, &square(1.0, 1.0, 1.0, 0.0), false);
    let hole = Loop {
        uses: ctx
            .face(fill)
            .unwrap()
            .outer
            .uses
            .iter()
            .rev()
            .map(|u| EdgeUse {
                edge: u.edge,
                reversed: !u.reversed,
            })
            .collect(),
    };
    let mut face = ctx.face(outer).unwrap().clone();
    let old = face.clone();
    face.inner.push(hole);
    ctx.apply(&Record::RemoveFace(outer, old)).unwrap();
    ctx.apply(&Record::AddFace(outer, face)).unwrap();
    assert_valid(ctx);
    (outer, fill, hole_edges)
}

#[test]
fn erase_hole_edge_closes_the_hole() {
    let mut ctx = Context::new();
    let (outer, fill, hole_edges) = face_with_filled_hole(&mut ctx);
    let outer_loop = ctx.face(outer).unwrap().outer.clone();
    erase(&mut ctx, hole_edges[0]);
    assert!(ctx.face(fill).is_none());
    let face = ctx.face(outer).unwrap();
    assert!(face.inner.is_empty());
    assert_eq!(face.outer, outer_loop);
    // The other three hole edges stay, loose inside the face.
    assert_eq!(counts(&ctx), (8, 7, 1));
}

#[test]
fn edge_into_hole_edge_splits_the_inner_loop() {
    let mut ctx = Context::new();
    let (outer, fill, _) = face_with_filled_hole(&mut ctx);
    draw(&mut ctx, p(1.5, 1.0, 0.0), p(1.5, 1.0, 1.0));
    assert_eq!(counts(&ctx), (10, 10, 2));
    let face = ctx.face(outer).unwrap();
    assert_eq!(face.inner[0].uses.len(), 5);
    assert_eq!(ctx.face(fill).unwrap().outer.uses.len(), 5);
}
