// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use drydock_kernel::geom;
use drydock_kernel::sticky::*;
use drydock_kernel::topology::*;
use glam::DVec3;

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The vertex at `q` (within 1e-9), panicking if there is none.
fn vat(ctx: &Context, q: DVec3) -> VertexId {
    ctx.find_vertex(q, 1e-9)
        .unwrap_or_else(|| panic!("no vertex at {q}"))
}

fn edge_at(ctx: &Context, a: DVec3, b: DVec3) -> EdgeId {
    ctx.edge_between(vat(ctx, a), vat(ctx, b))
        .unwrap_or_else(|| panic!("no edge {a} {b}"))
}

fn counts(ctx: &Context) -> (usize, usize, usize) {
    (ctx.vertex_count(), ctx.edge_count(), ctx.face_count())
}

fn assert_valid(ctx: &Context) {
    assert_eq!(ctx.validate(), vec![], "context: {ctx:#?}");
    assert_eq!(ctx.face_overlaps(), vec![], "context: {ctx:#?}");
}

fn assert_undo_redo(before: &Context, after: &Context, log: &Edit) {
    let mut undone = after.clone();
    undone.apply_edit(&log.inverse()).unwrap();
    assert_eq!(&undone, before, "undo");
    let mut redone = before.clone();
    redone.apply_edit(log).unwrap();
    assert_eq!(&redone, after, "redo");
}

/// Draw `a`-`b`, check the context and the undo log, return the chain and the log.
fn draw(ctx: &mut Context, a: DVec3, b: DVec3) -> (EdgeChain, Edit) {
    let before = ctx.clone();
    let mut log = Edit::new();
    let chain = ctx.add_edge(a, b, &mut log).unwrap();
    assert_valid(ctx);
    assert_undo_redo(&before, ctx, &log);
    for (f, _) in &chain.faces {
        assert!(ctx.face(*f).is_some(), "reported face {f:?} doesn't exist");
        assert!(before.face(*f).is_none(), "reported face {f:?} isn't new");
    }
    (chain, log)
}

/// Draw the closed polygon edge by edge; returns the chain of the closing edge.
fn draw_loop(ctx: &mut Context, pts: &[DVec3]) -> EdgeChain {
    let n = pts.len();
    let mut last = None;
    for i in 0..n {
        let (chain, _) = draw(ctx, pts[i], pts[(i + 1) % n]);
        last = Some(chain);
    }
    last.unwrap()
}

fn erase(ctx: &mut Context, e: EdgeId) -> Edit {
    let before = ctx.clone();
    let mut log = Edit::new();
    ctx.erase_edge(e, &mut log).unwrap();
    assert_valid(ctx);
    assert_undo_redo(&before, ctx, &log);
    log
}

fn square(x0: f64, y0: f64, s: f64, z: f64) -> Vec<DVec3> {
    vec![
        p(x0, y0, z),
        p(x0 + s, y0, z),
        p(x0 + s, y0 + s, z),
        p(x0, y0 + s, z),
    ]
}

fn normal(ctx: &Context, f: FaceId) -> DVec3 {
    ctx.face_plane(f).unwrap().normal
}

/// Area of the face (outer loop minus holes).
fn area(ctx: &Context, f: FaceId) -> f64 {
    let face = ctx.face(f).unwrap();
    let mut a = geom::area_vector(&ctx.loop_points(&face.outer)).length();
    for l in &face.inner {
        a -= geom::area_vector(&ctx.loop_points(l)).length();
    }
    a
}

fn total_area(ctx: &Context) -> f64 {
    ctx.faces().map(|(f, _)| area(ctx, f)).sum()
}

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn assert_normal(ctx: &Context, f: FaceId, n: DVec3) {
    assert!(
        normal(ctx, f).distance(n) < 1e-9,
        "face {f:?} normal {} != {n}",
        normal(ctx, f)
    );
}

fn only_face(ctx: &Context) -> FaceId {
    assert_eq!(ctx.face_count(), 1);
    ctx.faces().next().unwrap().0
}

/// The direction (`true` = forward) `f` uses `e` in, if it does.
fn use_dir(ctx: &Context, f: FaceId, e: EdgeId) -> Option<bool> {
    let face = ctx.face(f).unwrap();
    face.outer
        .uses
        .iter()
        .chain(face.inner.iter().flat_map(|l| l.uses.iter()))
        .find(|u| u.edge == e)
        .map(|u| !u.reversed)
}

/// A face built with primitives from the polygon (counter-clockwise seen from the front).
fn polygon_face(ctx: &mut Context, pts: &[DVec3]) -> FaceId {
    let mut log = Edit::new();
    let vs: Vec<_> = pts
        .iter()
        .map(|q| {
            ctx.find_vertex(*q, 1e-9)
                .unwrap_or_else(|| ctx.insert_vertex(*q, &mut log))
        })
        .collect();
    let n = vs.len();
    let mut uses = vec![];
    for i in 0..n {
        let (s, t) = (vs[i], vs[(i + 1) % n]);
        let e = match ctx.edge_between(s, t) {
            Some(e) => e,
            None => ctx.insert_edge(s, t, &mut log).unwrap(),
        };
        let forward = ctx.edge(e).unwrap().ends[0] == s;
        uses.push(EdgeUse {
            edge: e,
            reversed: !forward,
        });
    }
    ctx.insert_face(
        Face {
            outer: Loop { uses },
            inner: vec![],
        },
        &mut log,
    )
    .unwrap()
}

// -- faces from closed loops ------------------------------------------------------------

#[test]
fn square_makes_one_face_on_the_closing_edge() {
    let mut ctx = Context::new();
    let pts = square(0.0, 0.0, 1.0, 0.0);
    for i in 0..3 {
        let (chain, _) = draw(&mut ctx, pts[i], pts[i + 1]);
        assert!(chain.faces.is_empty());
    }
    assert_eq!(ctx.face_count(), 0);
    let (chain, _) = draw(&mut ctx, pts[3], pts[0]);
    assert_eq!(chain.faces.len(), 1);
    let (f, origin) = chain.faces[0];
    assert_eq!(origin, None);
    assert_eq!(counts(&ctx), (4, 4, 1));
    assert_normal(&ctx, f, DVec3::Z);
    assert_close(area(&ctx, f), 1.0);
    let face = ctx.face(f).unwrap();
    assert!(face.inner.is_empty());
    // The loop starts at its lowest-id edge.
    let lowest = face.outer.uses.iter().map(|u| u.edge).min().unwrap();
    assert_eq!(face.outer.uses[0].edge, lowest);
}

#[test]
fn clockwise_square_drawn_backwards_still_faces_up() {
    let mut ctx = Context::new();
    let mut pts = square(0.0, 0.0, 2.0, 1.5);
    pts.reverse();
    draw_loop(&mut ctx, &pts);
    let f = only_face(&ctx);
    assert_normal(&ctx, f, DVec3::Z);
    assert_close(area(&ctx, f), 4.0);
}

#[test]
fn vertical_and_tilted_faces_use_the_canonical_normal() {
    // Plane y = 0 faces +Y.
    let mut ctx = Context::new();
    draw_loop(
        &mut ctx,
        &[
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, 1.0),
            p(1.0, 0.0, 1.0),
            p(1.0, 0.0, 0.0),
        ],
    );
    assert_normal(&ctx, only_face(&ctx), DVec3::Y);

    // Plane x = 2 faces +X.
    let mut ctx = Context::new();
    draw_loop(
        &mut ctx,
        &[p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0), p(2.0, 0.0, 1.0)],
    );
    assert_normal(&ctx, only_face(&ctx), DVec3::X);

    // Plane z = -x tilts up and back: +Z component positive.
    let mut ctx = Context::new();
    draw_loop(
        &mut ctx,
        &[p(1.0, 0.0, -1.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 0.0)],
    );
    let n = normal(&ctx, only_face(&ctx));
    assert!((n - DVec3::new(1.0, 0.0, 1.0).normalize()).length() < 1e-9);
}

#[test]
fn concave_polygon_is_one_face() {
    let mut ctx = Context::new();
    draw_loop(
        &mut ctx,
        &[
            p(0.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(1.0, 2.0, 0.0),
            p(0.0, 2.0, 0.0),
        ],
    );
    let f = only_face(&ctx);
    assert_close(area(&ctx, f), 3.0);
    assert_eq!(ctx.face(f).unwrap().outer.uses.len(), 6);
}

#[test]
fn one_long_segment_can_close_a_loop() {
    // Three sides of a square drawn with a gap, closed by a segment that runs along the
    // bottom past both corners.
    let mut ctx = Context::new();
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    draw(&mut ctx, p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0));
    draw(&mut ctx, p(0.0, 1.0, 0.0), p(0.0, 0.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(-1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    assert_eq!(chain.edges.len(), 3);
    assert_eq!(chain.faces.len(), 1);
    assert_close(area(&ctx, chain.faces[0].0), 1.0);
    assert_eq!(counts(&ctx), (6, 6, 1));
}

#[test]
fn skew_quadrilateral_makes_no_face() {
    let mut ctx = Context::new();
    draw_loop(
        &mut ctx,
        &[
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.5),
            p(0.0, 1.0, 0.0),
        ],
    );
    assert_eq!(counts(&ctx), (4, 4, 0));
}

#[test]
fn two_disjoint_squares_make_two_faces() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    draw_loop(&mut ctx, &square(3.0, 0.0, 1.0, 0.0));
    assert_eq!(counts(&ctx), (8, 8, 2));
    assert_close(total_area(&ctx), 2.0);
}

// -- neighbours and orientation ---------------------------------------------------------

#[test]
fn adjacent_squares_share_an_edge_in_opposite_directions() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let a = only_face(&ctx);
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(2.0, 1.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(chain.faces.len(), 1);
    let (b, origin) = chain.faces[0];
    assert_eq!(origin, None);
    assert!(ctx.face(a).is_some());
    assert_normal(&ctx, b, DVec3::Z);
    let shared = edge_at(&ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    let (da, db) = (use_dir(&ctx, a, shared), use_dir(&ctx, b, shared));
    assert!(da.is_some() && db.is_some());
    assert_ne!(da, db);
}

#[test]
fn neighbour_decides_orientation_over_the_canonical_rule() {
    // A face facing down; a new square next to it faces down too.
    let mut ctx = Context::new();
    let mut pts = square(0.0, 0.0, 1.0, 0.0);
    pts.reverse();
    let a = polygon_face(&mut ctx, &pts);
    assert_normal(&ctx, a, -DVec3::Z);
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(2.0, 1.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(chain.faces.len(), 1);
    assert_normal(&ctx, chain.faces[0].0, -DVec3::Z);
}

#[test]
fn box_drawn_up_from_its_bottom_is_consistently_oriented() {
    let mut ctx = Context::new();
    let bottom = square(0.0, 0.0, 1.0, 0.0);
    let top = square(0.0, 0.0, 1.0, 1.0);
    draw_loop(&mut ctx, &bottom);
    for q in &bottom {
        let (chain, _) = draw(&mut ctx, *q, *q + DVec3::Z);
        assert!(chain.faces.is_empty());
    }
    let mut made = vec![];
    for i in 0..4 {
        let (chain, _) = draw(&mut ctx, top[i], top[(i + 1) % 4]);
        made.push(chain.faces.len());
    }
    // Each top edge closes a side; the last one closes the last side and the top.
    assert_eq!(made, vec![1, 1, 1, 2]);
    assert_eq!(counts(&ctx), (8, 12, 6));
    // Every edge borders two faces that use it in opposite directions.
    for (e, _) in ctx.edges() {
        let fs = ctx.faces_of_edge(e);
        assert_eq!(fs.len(), 2);
        assert_ne!(use_dir(&ctx, fs[0], e), use_dir(&ctx, fs[1], e));
    }
    // The bottom faces up (into the box), so every face faces inward.
    let centre = p(0.5, 0.5, 0.5);
    for (f, _) in ctx.faces() {
        let plane = ctx.face_plane(f).unwrap();
        assert!(plane.signed_distance(centre) > 0.4, "face {f:?} faces out");
    }
}

// -- splitting faces --------------------------------------------------------------------

#[test]
fn diagonal_splits_a_face_in_two() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let f = only_face(&ctx);
    let (chain, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(chain.faces.len(), 2);
    assert!(chain.faces.iter().all(|(_, o)| *o == Some(f)));
    assert!(ctx.face(f).is_none());
    assert_eq!(counts(&ctx), (4, 5, 2));
    for (g, _) in &chain.faces {
        assert_close(area(&ctx, *g), 0.5);
        assert_normal(&ctx, *g, DVec3::Z);
    }
}

#[test]
fn split_pieces_keep_a_downward_face_down() {
    let mut ctx = Context::new();
    let mut pts = square(0.0, 0.0, 2.0, 0.0);
    pts.reverse();
    let f = polygon_face(&mut ctx, &pts);
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 2.0, 0.0));
    assert_eq!(chain.faces.len(), 2);
    for (g, o) in &chain.faces {
        assert_eq!(*o, Some(f));
        assert_normal(&ctx, *g, -DVec3::Z);
        assert_close(area(&ctx, *g), 2.0);
    }
}

#[test]
fn line_overshooting_a_face_splits_it_and_leaves_spurs() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 2.0, 0.0));
    let f = only_face(&ctx);
    let (chain, _) = draw(&mut ctx, p(1.0, -1.0, 0.0), p(1.0, 3.0, 0.0));
    assert_eq!(chain.edges.len(), 3);
    assert_eq!(chain.faces.len(), 2);
    assert!(chain.faces.iter().all(|(_, o)| *o == Some(f)));
    assert_eq!(counts(&ctx), (8, 9, 2));
    assert_close(total_area(&ctx), 4.0);
}

#[test]
fn both_diagonals_make_four_triangles() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let (first, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    let (second, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0));
    assert_eq!(second.edges.len(), 2);
    assert_eq!(second.faces.len(), 4);
    let halves: Vec<FaceId> = first.faces.iter().map(|(g, _)| *g).collect();
    for (_, o) in &second.faces {
        assert!(halves.contains(&o.unwrap()));
    }
    assert_eq!(counts(&ctx), (5, 8, 4));
    for (g, _) in ctx.faces() {
        assert_close(area(&ctx, g), 0.25);
        assert_normal(&ctx, g, DVec3::Z);
    }
}

#[test]
fn spur_into_a_face_leaves_it_alone() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 2.0, 0.0));
    let f = only_face(&ctx);
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert!(chain.faces.is_empty());
    assert_eq!(only_face(&ctx), f);
    assert_eq!(ctx.face(f).unwrap().outer.uses.len(), 5);
    // A loose segment lying on the face changes nothing either.
    let (chain, _) = draw(&mut ctx, p(0.5, 1.5, 0.0), p(1.5, 1.5, 0.0));
    assert!(chain.faces.is_empty());
    assert_eq!(only_face(&ctx), f);
}

#[test]
fn loop_inside_a_face_makes_a_hole_and_a_filled_face() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 4.0, 0.0));
    let f = only_face(&ctx);
    let chain = draw_loop(&mut ctx, &square(1.0, 1.0, 1.0, 0.0));
    assert_eq!(chain.faces.len(), 2);
    assert!(chain.faces.iter().all(|(_, o)| *o == Some(f)));
    let (ring, inner) = (chain.faces[0].0, chain.faces[1].0);
    assert_eq!(ctx.face(ring).unwrap().inner.len(), 1);
    assert_eq!(ctx.face(ring).unwrap().inner[0].uses.len(), 4);
    assert_close(area(&ctx, ring), 15.0);
    assert_close(area(&ctx, inner), 1.0);
    assert_normal(&ctx, ring, DVec3::Z);
    assert_normal(&ctx, inner, DVec3::Z);
    assert_eq!(counts(&ctx), (8, 8, 2));
}

#[test]
fn square_around_a_face_makes_a_face_with_a_hole() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(1.0, 1.0, 1.0, 0.0));
    let inner = only_face(&ctx);
    let chain = draw_loop(&mut ctx, &square(0.0, 0.0, 3.0, 0.0));
    assert_eq!(chain.faces.len(), 1);
    let (ring, origin) = chain.faces[0];
    assert_eq!(origin, None);
    assert!(ctx.face(inner).is_some());
    assert_eq!(ctx.face(ring).unwrap().inner.len(), 1);
    assert_close(area(&ctx, ring), 8.0);
    assert_normal(&ctx, ring, DVec3::Z);
}

#[test]
fn loop_off_the_face_plane_leaves_the_face_alone() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 2.0, 0.0));
    let f = only_face(&ctx);
    let before = ctx.face(f).unwrap().clone();
    // A vertical triangle standing on the face's bottom edge.
    draw(&mut ctx, p(0.5, 0.0, 0.0), p(1.0, 0.0, 1.0));
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 1.0), p(1.5, 0.0, 0.0));
    assert_eq!(chain.faces.len(), 1);
    let (g, origin) = chain.faces[0];
    assert_eq!(origin, None);
    assert_normal(&ctx, g, DVec3::Y);
    assert_close(area(&ctx, g), 0.5);
    // The face only got the two new vertices in its bottom edge.
    assert_eq!(ctx.face(f).unwrap().outer.uses.len(), 6);
    assert_eq!(before.outer.uses.len(), 4);
    assert_close(area(&ctx, f), 4.0);
}

// -- redrawing --------------------------------------------------------------------------

#[test]
fn redrawing_a_face_edge_records_nothing() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let before = ctx.clone();
    let (chain, log) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert!(chain.faces.is_empty());
    assert!(log.is_empty());
    assert_eq!(ctx, before);
}

#[test]
fn redrawing_an_edge_restores_a_missing_face() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let f = only_face(&ctx);
    let mut log = Edit::new();
    ctx.delete_face(f, &mut log).unwrap();
    assert_eq!(ctx.face_count(), 0);
    let (chain, _) = draw(&mut ctx, p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0));
    assert_eq!(chain.faces.len(), 1);
    assert_eq!(chain.faces[0].1, None);
    assert_ne!(chain.faces[0].0, f);
    assert_normal(&ctx, chain.faces[0].0, DVec3::Z);
}

// -- known limit: regions touching themselves -------------------------------------------

#[test]
fn triangle_touching_the_face_edge_waits_for_a_simple_split() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 4.0, 0.0));
    let f = only_face(&ctx);
    // A triangle inside the face with one corner on its bottom edge: the rest of the face
    // would touch itself at that corner, so nothing is split.
    let chain = draw_loop(
        &mut ctx,
        &[p(2.0, 0.0, 0.0), p(3.0, 1.0, 0.0), p(1.0, 1.0, 0.0)],
    );
    assert!(chain.faces.is_empty());
    assert_eq!(only_face(&ctx), f);
    assert_close(area(&ctx, f), 16.0);
    // A segment from the triangle to the left edge makes every piece simple.
    let (chain, _) = draw(&mut ctx, p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0));
    assert_eq!(chain.faces.len(), 3);
    assert!(chain.faces.iter().all(|(_, o)| *o == Some(f)));
    assert_close(total_area(&ctx), 16.0);
    assert_close(area(&ctx, chain.faces[0].0), 15.0 - 1.5);
    // Erasing it again merges two pieces into a region that touches itself: both go.
    let link = edge_at(&ctx, p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0));
    erase(&mut ctx, link);
    let tri = only_face(&ctx);
    assert_close(area(&ctx, tri), 1.0);
}

// -- erase merges coplanar faces --------------------------------------------------------

#[test]
fn erasing_the_shared_edge_merges_two_squares() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let a = only_face(&ctx);
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0));
    draw(&mut ctx, p(2.0, 1.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(ctx.face_count(), 2);
    let shared = edge_at(&ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    erase(&mut ctx, shared);
    // The merged face keeps the lower id; the middle vertices stay in its loop.
    assert_eq!(only_face(&ctx), a);
    assert_eq!(counts(&ctx), (6, 6, 1));
    assert_eq!(ctx.face(a).unwrap().outer.uses.len(), 6);
    assert_close(area(&ctx, a), 2.0);
    assert_normal(&ctx, a, DVec3::Z);
}

#[test]
fn erasing_a_diagonal_restores_the_square() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let (chain, _) = draw(&mut ctx, p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    let lower = chain.faces.iter().map(|(g, _)| *g).min().unwrap();
    erase(&mut ctx, chain.edges[0]);
    assert_eq!(only_face(&ctx), lower);
    assert_eq!(counts(&ctx), (4, 4, 1));
    assert_close(area(&ctx, lower), 1.0);
}

#[test]
fn erasing_a_downward_shared_edge_keeps_the_merged_face_down() {
    let mut ctx = Context::new();
    let mut pts = square(0.0, 0.0, 2.0, 0.0);
    pts.reverse();
    polygon_face(&mut ctx, &pts);
    let (chain, _) = draw(&mut ctx, p(1.0, 0.0, 0.0), p(1.0, 2.0, 0.0));
    erase(&mut ctx, chain.edges[0]);
    let f = only_face(&ctx);
    assert_normal(&ctx, f, -DVec3::Z);
    assert_close(area(&ctx, f), 4.0);
}

#[test]
fn erasing_between_opposite_facing_coplanar_faces_removes_both() {
    let mut ctx = Context::new();
    polygon_face(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let right = [
        p(2.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(2.0, 1.0, 0.0),
    ];
    polygon_face(&mut ctx, &right);
    assert_valid(&ctx);
    let shared = edge_at(&ctx, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    erase(&mut ctx, shared);
    assert_eq!(counts(&ctx), (6, 6, 0));
}

#[test]
fn erasing_an_island_edge_merges_it_back_into_the_ring() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 3.0, 0.0));
    let chain = draw_loop(&mut ctx, &square(1.0, 1.0, 1.0, 0.0));
    let ring = chain.faces[0].0;
    let island_edge = edge_at(&ctx, p(1.0, 1.0, 0.0), p(2.0, 1.0, 0.0));
    erase(&mut ctx, island_edge);
    // The other three island edges stay, loose on the merged face.
    assert_eq!(only_face(&ctx), ring);
    assert!(ctx.face(ring).unwrap().inner.is_empty());
    assert_close(area(&ctx, ring), 9.0);
    assert_eq!(counts(&ctx), (8, 7, 1));
}

#[test]
fn erasing_an_outer_edge_of_two_squares_removes_one() {
    let mut ctx = Context::new();
    draw_loop(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let a = only_face(&ctx);
    draw(&mut ctx, p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    draw(&mut ctx, p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0));
    draw(&mut ctx, p(2.0, 1.0, 0.0), p(1.0, 1.0, 0.0));
    let outer = edge_at(&ctx, p(2.0, 0.0, 0.0), p(2.0, 1.0, 0.0));
    erase(&mut ctx, outer);
    assert_eq!(only_face(&ctx), a);
}

// -- face_overlaps ----------------------------------------------------------------------

#[test]
fn face_overlaps_reports_nested_and_duplicate_faces() {
    // A small face inside a big one, no shared edges.
    let mut ctx = Context::new();
    let big = polygon_face(&mut ctx, &square(0.0, 0.0, 3.0, 0.0));
    let small = polygon_face(&mut ctx, &square(1.0, 1.0, 1.0, 0.0));
    assert_eq!(ctx.face_overlaps(), vec![(big, small)]);

    // Facing doesn't matter.
    let mut ctx = Context::new();
    let big = polygon_face(&mut ctx, &square(0.0, 0.0, 3.0, 0.0));
    let mut pts = square(1.0, 1.0, 1.0, 0.0);
    pts.reverse();
    let small = polygon_face(&mut ctx, &pts);
    assert_eq!(ctx.face_overlaps(), vec![(big, small)]);

    // Two faces on the same loop.
    let mut ctx = Context::new();
    let a = polygon_face(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    let b = polygon_face(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    assert_eq!(ctx.face_overlaps(), vec![(a, b)]);

    // A face sharing two edges with a bigger one it lies inside.
    let mut ctx = Context::new();
    let big = polygon_face(
        &mut ctx,
        &[
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 2.0, 0.0),
            p(0.0, 2.0, 0.0),
            p(0.0, 1.0, 0.0),
        ],
    );
    let corner = polygon_face(
        &mut ctx,
        &[
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
        ],
    );
    assert_eq!(ctx.face_overlaps(), vec![(big, corner)]);
}

#[test]
fn face_overlaps_ignores_neighbours_and_other_planes() {
    let mut ctx = Context::new();
    polygon_face(&mut ctx, &square(0.0, 0.0, 1.0, 0.0));
    polygon_face(&mut ctx, &square(1.0, 0.0, 1.0, 0.0));
    polygon_face(&mut ctx, &square(1.0, 1.0, 1.0, 0.0));
    polygon_face(&mut ctx, &square(0.0, 0.0, 1.0, 1.0));
    polygon_face(&mut ctx, &square(0.25, 0.25, 0.5, 0.5));
    assert_eq!(ctx.face_overlaps(), vec![]);
}

#[test]
fn line_just_past_a_vertex_splits_the_face_into_a_sliver() {
    // A wedge with a 80 degree apex at the origin, cut by x = 1.5e-5: the cut ends are
    // farther than LINEAR_TOL from the apex and from each other, but the cut's midpoint is
    // within LINEAR_TOL of both sides.
    let mut ctx = Context::new();
    let t = 40f64.to_radians().tan();
    let wedge = [
        p(0.0, 0.0, 0.0),
        p(10.0, -10.0 * t, 0.0),
        p(10.0, 10.0 * t, 0.0),
    ];
    let f = polygon_face(&mut ctx, &wedge);
    let before = area(&ctx, f);
    let x = 1.5e-5;
    draw(&mut ctx, p(x, -10.0, 0.0), p(x, 10.0, 0.0));
    assert_eq!(ctx.face_count(), 2);
    assert!((total_area(&ctx) - before).abs() < 1e-6 * before);
}
