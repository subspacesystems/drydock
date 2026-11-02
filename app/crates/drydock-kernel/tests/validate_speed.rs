// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `Context::validate` reports exactly what the all-pairs checks report, in the same order,
//! and stays fast on large contexts (`docs/specs/m4-fix-copy-speed.md`, part 1).
//!
//! `all_pairs` below is the old implementation, kept here as the reference.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use drydock_kernel::geom::{self, SegmentHit};
use drydock_kernel::topology::*;
use glam::DVec3;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The all-pairs reference: what `validate` returned before this task.
fn all_pairs(ctx: &Context) -> Vec<Violation> {
    let mut out = Vec::new();
    for (id, _) in ctx.vertices() {
        if ctx.edges_of_vertex(id).is_empty() {
            out.push(Violation::StrayVertex(id));
        }
    }
    let verts: Vec<(VertexId, DVec3)> = ctx.vertices().map(|(id, v)| (id, v.position)).collect();
    for i in 0..verts.len() {
        for j in i + 1..verts.len() {
            if verts[i].1.distance(verts[j].1) <= geom::LINEAR_TOL {
                out.push(Violation::DuplicateVertex(verts[i].0, verts[j].0));
            }
        }
    }
    let edges: Vec<(EdgeId, Edge)> = ctx.edges().map(|(id, e)| (id, *e)).collect();
    let mut duplicate_pairs = BTreeSet::new();
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            let (a, b) = (edges[i].1, edges[j].1);
            if (a.ends[0] == b.ends[0] && a.ends[1] == b.ends[1])
                || (a.ends[0] == b.ends[1] && a.ends[1] == b.ends[0])
            {
                duplicate_pairs.insert((i, j));
                out.push(Violation::DuplicateEdge(edges[i].0, edges[j].0));
            }
        }
    }
    let pos = |id: VertexId| ctx.vertex(id).map(|v| v.position);
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            if duplicate_pairs.contains(&(i, j)) {
                continue;
            }
            let (ea, eb) = (edges[i].1, edges[j].1);
            let (Some(a0), Some(a1), Some(b0), Some(b1)) = (
                pos(ea.ends[0]),
                pos(ea.ends[1]),
                pos(eb.ends[0]),
                pos(eb.ends[1]),
            ) else {
                continue;
            };
            let shared: Vec<VertexId> = ea
                .ends
                .iter()
                .copied()
                .filter(|v| eb.ends.contains(v))
                .collect();
            let hit = geom::segment_intersection(a0, a1, b0, b1, geom::LINEAR_TOL);
            let bad = match shared.as_slice() {
                [only] => {
                    let expected_a = if ea.ends[0] == *only { 0.0 } else { 1.0 };
                    let expected_b = if eb.ends[0] == *only { 0.0 } else { 1.0 };
                    match hit {
                        SegmentHit::None => false,
                        SegmentHit::Point { ta, tb } => !(ta == expected_a && tb == expected_b),
                        SegmentHit::Overlap { .. } => true,
                    }
                }
                [] => !matches!(hit, SegmentHit::None),
                _ => false,
            };
            if bad {
                out.push(Violation::EdgesIntersect(edges[i].0, edges[j].0));
            }
        }
    }
    let faces: Vec<(FaceId, Face)> = ctx.faces().map(|(id, f)| (id, f.clone())).collect();
    for (id, _) in &faces {
        if let Some(violation) = ctx.face_violation(*id) {
            out.push(violation);
        }
    }
    let sets: Vec<BTreeSet<EdgeId>> = faces
        .iter()
        .map(|(_, f)| f.outer.uses.iter().map(|u| u.edge).collect())
        .collect();
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            if sets[i] == sets[j] {
                out.push(Violation::DuplicateFace(faces[i].0, faces[j].0));
            }
        }
    }
    out
}

/// A closed loop through `vs`, inserting the edges it needs (reusing none).
fn ring(ctx: &mut Context, vs: &[VertexId], log: &mut Edit) -> Loop {
    let mut uses = Vec::new();
    for i in 0..vs.len() {
        let e = ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], log).unwrap();
        uses.push(EdgeUse::forward(e));
    }
    Loop { uses }
}

/// An `n` × `n` grid of unit squares in z = 0 sharing their edges: a valid context with
/// (n+1)² vertices, 2n(n+1) edges and n² faces.
fn grid(n: usize) -> Context {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let mut vs = Vec::new();
    for y in 0..=n {
        for x in 0..=n {
            vs.push(ctx.insert_vertex(v(x as f64, y as f64, 0.0), &mut log));
        }
    }
    let at = |x: usize, y: usize| y * (n + 1) + x;
    let mut horizontal = Vec::new();
    for y in 0..=n {
        for x in 0..n {
            horizontal.push(
                ctx.insert_edge(vs[at(x, y)], vs[at(x + 1, y)], &mut log)
                    .unwrap(),
            );
        }
    }
    let mut vertical = Vec::new();
    for y in 0..n {
        for x in 0..=n {
            vertical.push(
                ctx.insert_edge(vs[at(x, y)], vs[at(x, y + 1)], &mut log)
                    .unwrap(),
            );
        }
    }
    for y in 0..n {
        for x in 0..n {
            let bottom = horizontal[y * n + x];
            let top = horizontal[(y + 1) * n + x];
            let left = vertical[y * (n + 1) + x];
            let right = vertical[y * (n + 1) + x + 1];
            ctx.insert_face(
                Face {
                    outer: Loop {
                        uses: vec![
                            EdgeUse::forward(bottom),
                            EdgeUse::forward(right),
                            EdgeUse::backward(top),
                            EdgeUse::backward(left),
                        ],
                    },
                    inner: Vec::new(),
                },
                &mut log,
            )
            .unwrap();
        }
    }
    ctx
}

fn assert_matches_reference(ctx: &Context) {
    assert_eq!(ctx.validate(), all_pairs(ctx));
}

#[test]
fn empty_context_is_valid() {
    assert!(Context::new().validate().is_empty());
}

#[test]
fn a_valid_grid_has_no_violations() {
    let ctx = grid(6);
    assert!(ctx.validate().is_empty());
    assert_matches_reference(&ctx);
}

#[test]
fn stray_and_duplicate_vertices_are_reported_in_id_order() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0.0, 0.0, 0.0), &mut log);
    let b = ctx.insert_vertex(v(1.0, 0.0, 0.0), &mut log);
    let c = ctx.insert_vertex(v(0.0, 0.0, 0.4 * geom::LINEAR_TOL), &mut log);
    let d = ctx.insert_vertex(v(1.0, 0.0, 0.0), &mut log);
    let _far = ctx.insert_vertex(v(0.0, 3.0 * geom::LINEAR_TOL, 0.0), &mut log);
    ctx.insert_edge(a, b, &mut log).unwrap();
    let got = ctx.validate();
    assert!(got.contains(&Violation::DuplicateVertex(a, c)));
    assert!(got.contains(&Violation::DuplicateVertex(b, d)));
    assert!(got.contains(&Violation::StrayVertex(c)));
    assert_matches_reference(&ctx);
}

#[test]
fn duplicates_across_a_cell_boundary_are_found() {
    // Points just either side of coordinates a spatial grid might use as cell walls.
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let tol = geom::LINEAR_TOL;
    for wall in [0.0, tol, 2.0 * tol, 1.0, 0.5, -0.25, 1000.0] {
        let p = ctx.insert_vertex(v(wall - 0.45 * tol, wall, -wall), &mut log);
        let q = ctx.insert_vertex(v(wall + 0.45 * tol, wall, -wall), &mut log);
        let r = ctx.insert_vertex(v(wall, wall + 0.6 * tol, -wall - 0.6 * tol), &mut log);
        ctx.insert_edge(p, q, &mut log).ok();
        ctx.insert_edge(q, r, &mut log).ok();
    }
    assert_matches_reference(&ctx);
    assert!(
        ctx.validate()
            .iter()
            .filter(|v| matches!(v, Violation::DuplicateVertex(..)))
            .count()
            >= 7
    );
}

#[test]
fn duplicate_and_crossing_edges_match_the_reference() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0.0, 0.0, 0.0), &mut log);
    let b = ctx.insert_vertex(v(2.0, 0.0, 0.0), &mut log);
    let c = ctx.insert_vertex(v(1.0, -1.0, 0.0), &mut log);
    let d = ctx.insert_vertex(v(1.0, 1.0, 0.0), &mut log);
    let e = ctx.insert_vertex(v(1.0, 0.0, 0.0), &mut log);
    let f = ctx.insert_vertex(v(3.0, 0.0, 0.0), &mut log);
    let g = ctx.insert_vertex(v(5.0, 5.0, 5.0), &mut log);
    let ab = ctx.insert_edge(a, b, &mut log).unwrap();
    let ba = ctx.insert_edge(b, a, &mut log).unwrap();
    let cd = ctx.insert_edge(c, d, &mut log).unwrap();
    let ef = ctx.insert_edge(e, f, &mut log).unwrap();
    let _bg = ctx.insert_edge(b, g, &mut log).unwrap();
    let got = ctx.validate();
    assert!(got.contains(&Violation::DuplicateEdge(ab, ba)));
    assert!(got.contains(&Violation::EdgesIntersect(ab, cd)));
    assert!(got.contains(&Violation::EdgesIntersect(ab, ef)));
    assert!(!got.contains(&Violation::EdgesIntersect(ab, ba)));
    assert_matches_reference(&ctx);
}

#[test]
fn edges_that_nearly_touch_end_to_middle_are_found() {
    // A T-junction where the stem stops within the tolerance of the bar but off its ends.
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0.0, 0.0, 0.0), &mut log);
    let b = ctx.insert_vertex(v(10.0, 0.0, 0.0), &mut log);
    let c = ctx.insert_vertex(v(4.0, 0.5 * geom::LINEAR_TOL, 0.0), &mut log);
    let d = ctx.insert_vertex(v(4.0, 7.0, 0.0), &mut log);
    let bar = ctx.insert_edge(a, b, &mut log).unwrap();
    let stem = ctx.insert_edge(c, d, &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![Violation::EdgesIntersect(bar, stem)]);
    assert_matches_reference(&ctx);
}

#[test]
fn duplicate_faces_and_face_checks_match_the_reference() {
    let mut ctx = grid(2);
    let mut log = Edit::new();
    let first = ctx.faces().next().map(|(_, f)| f.clone()).unwrap();
    let mut reversed = first.clone();
    reversed.outer.uses.reverse();
    for u in &mut reversed.outer.uses {
        u.reversed = !u.reversed;
    }
    let again = ctx.insert_face(first, &mut log).unwrap();
    let flipped = ctx.insert_face(reversed, &mut log).unwrap();
    // A face that isn't planar.
    let p: Vec<VertexId> = [
        v(10.0, 0.0, 0.0),
        v(11.0, 0.0, 0.0),
        v(11.0, 1.0, 0.5),
        v(10.0, 1.0, 0.0),
    ]
    .into_iter()
    .map(|q| ctx.insert_vertex(q, &mut log))
    .collect();
    let outer = ring(&mut ctx, &p, &mut log);
    let bent = ctx
        .insert_face(
            Face {
                outer,
                inner: Vec::new(),
            },
            &mut log,
        )
        .unwrap();
    let got = ctx.validate();
    assert!(
        got.iter()
            .any(|x| matches!(x, Violation::DuplicateFace(_, b) if *b == again))
    );
    assert!(
        got.iter()
            .any(|x| matches!(x, Violation::DuplicateFace(_, b) if *b == flipped))
    );
    assert!(got.contains(&Violation::NonPlanarFace(bent)));
    assert_matches_reference(&ctx);
}

#[test]
fn edges_with_missing_vertices_are_skipped_like_the_reference() {
    // Deleting a vertex an edge uses is refused, so only well-formed contexts reach here;
    // this pins down that edges between far-apart points and very long edges still pair.
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(-1e6, 0.0, 0.0), &mut log);
    let b = ctx.insert_vertex(v(1e6, 0.0, 0.0), &mut log);
    let c = ctx.insert_vertex(v(0.0, -1e6, 1.0), &mut log);
    let d = ctx.insert_vertex(v(0.0, 1e6, -1.0), &mut log);
    let ab = ctx.insert_edge(a, b, &mut log).unwrap();
    let cd = ctx.insert_edge(c, d, &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![Violation::EdgesIntersect(ab, cd)]);
    assert_matches_reference(&ctx);
}

#[test]
fn non_finite_positions_match_the_reference() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0.0, 0.0, 0.0), &mut log);
    let b = ctx.insert_vertex(v(f64::NAN, 0.0, 0.0), &mut log);
    let c = ctx.insert_vertex(v(f64::INFINITY, 1.0, 0.0), &mut log);
    let d = ctx.insert_vertex(v(1.0, 1.0, 0.0), &mut log);
    let e = ctx.insert_vertex(v(0.5, -1.0, 0.0), &mut log);
    let f = ctx.insert_vertex(v(0.5, 2.0, 0.0), &mut log);
    ctx.insert_edge(a, b, &mut log).unwrap();
    ctx.insert_edge(c, d, &mut log).unwrap();
    ctx.insert_edge(e, f, &mut log).unwrap();
    ctx.insert_edge(a, d, &mut log).unwrap();
    assert_matches_reference(&ctx);
}

/// Runs `validate` on another thread and asserts it finds nothing within `limit`. The
/// all-pairs version takes many minutes on these contexts; the thread is left behind.
fn assert_valid_within(ctx: Context, limit: Duration) {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let start = Instant::now();
        let found = ctx.validate();
        let _ = send.send((found, start.elapsed()));
    });
    match receive.recv_timeout(limit) {
        Ok((found, elapsed)) => {
            assert!(found.is_empty(), "{:?}", &found[..found.len().min(5)]);
            eprintln!("validate took {elapsed:?}");
        }
        Err(_) => panic!("validate took longer than {limit:?}"),
    }
}

#[test]
fn a_large_grid_validates_quickly() {
    // 10 201 vertices, 20 200 edges, 10 000 faces: about 2·10⁸ edge pairs all-pairs.
    assert_valid_within(grid(100), Duration::from_secs(20));
}

#[test]
fn long_parallel_edges_validate_quickly() {
    // 6000 edges spanning the same x range, spread in y and z: every pair overlaps along x.
    let mut ctx = Context::new();
    let mut log = Edit::new();
    for i in 0..6000 {
        let y = (i % 100) as f64 * 0.1;
        let z = (i / 100) as f64 * 0.1;
        let a = ctx.insert_vertex(v(0.0, y, z), &mut log);
        let b = ctx.insert_vertex(v(50.0, y, z), &mut log);
        ctx.insert_edge(a, b, &mut log).unwrap();
    }
    assert_valid_within(ctx, Duration::from_secs(20));
}

#[test]
fn a_large_crossing_free_grid_of_parallel_edge_families_validates_quickly() {
    // Two families of long edges in separate planes (z = 0 along x, z = 1 along y), so
    // every x edge's box overlaps every y edge's box in x and y but not in z.
    let mut ctx = Context::new();
    let mut log = Edit::new();
    for i in 0..3000 {
        let t = i as f64 * 0.01;
        let a = ctx.insert_vertex(v(0.0, t, 0.0), &mut log);
        let b = ctx.insert_vertex(v(30.0, t, 0.0), &mut log);
        ctx.insert_edge(a, b, &mut log).unwrap();
        let c = ctx.insert_vertex(v(t, 0.0, 1.0), &mut log);
        let d = ctx.insert_vertex(v(t, 30.0, 1.0), &mut log);
        ctx.insert_edge(c, d, &mut log).unwrap();
    }
    assert_valid_within(ctx, Duration::from_secs(20));
}

/// Random contexts on a coarse lattice, some points nudged by about the tolerance, with
/// random edges (some duplicated or reversed) and random three- and four-edge faces.
fn random_context() -> impl Strategy<Value = Context> {
    let point = (0u8..4, 0u8..4, 0u8..3, 0u8..6, 0u8..6);
    let points = prop::collection::vec(point, 2..40);
    let edges = prop::collection::vec(
        (any::<prop::sample::Index>(), any::<prop::sample::Index>()),
        0..60,
    );
    let faces = prop::collection::vec(
        prop::collection::vec(any::<prop::sample::Index>(), 3..5),
        0..8,
    );
    (points, edges, faces).prop_map(|(points, edges, faces)| {
        let tol = geom::LINEAR_TOL;
        let nudge = |k: u8| match k {
            0 => 0.0,
            1 => 0.3 * tol,
            2 => -0.49 * tol,
            3 => 0.99 * tol,
            4 => 1.5 * tol,
            _ => 0.5,
        };
        let mut ctx = Context::new();
        let mut log = Edit::new();
        let vs: Vec<VertexId> = points
            .iter()
            .map(|&(x, y, z, nx, ny)| {
                let p = v(
                    f64::from(x) * 0.5 + nudge(nx),
                    f64::from(y) * 0.5 + nudge(ny),
                    f64::from(z) * 0.5,
                );
                ctx.insert_vertex(p, &mut log)
            })
            .collect();
        let mut made = Vec::new();
        for (a, b) in edges {
            let (a, b) = (vs[a.index(vs.len())], vs[b.index(vs.len())]);
            if let Ok(e) = ctx.insert_edge(a, b, &mut log) {
                made.push(e);
            }
        }
        if !made.is_empty() {
            for picks in faces {
                let uses = picks
                    .iter()
                    .map(|i| EdgeUse::forward(made[i.index(made.len())]))
                    .collect();
                let _ = ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: Vec::new(),
                    },
                    &mut log,
                );
            }
        }
        ctx
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn validate_matches_the_all_pairs_reference(ctx in random_context()) {
        prop_assert_eq!(ctx.validate(), all_pairs(&ctx));
    }
}
