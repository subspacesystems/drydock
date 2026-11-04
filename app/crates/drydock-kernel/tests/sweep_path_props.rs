// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for sweep paths (`docs/specs/m9-sweep-path.md`): however a chain's vertices
//! and edges were created and however the edges are given, the path is the same.

use std::collections::BTreeSet;

use drydock_kernel::sweep::*;
use drydock_kernel::topology::*;
use glam::DVec3;
use proptest::prelude::*;

/// A chain of `edges` edges (closed or open) with random vertex and edge creation order, edge
/// directions, and input order with repeats.
#[derive(Clone, Debug)]
struct Case {
    closed: bool,
    edges: usize,
    offsets: Vec<(f64, f64)>,
    vertex_order: Vec<usize>,
    edge_order: Vec<usize>,
    flips: Vec<bool>,
    input: Vec<usize>,
    repeats: Vec<usize>,
}

fn case() -> impl Strategy<Value = Case> {
    (any::<bool>(), 1usize..12)
        .prop_flat_map(|(closed, k)| {
            let edges = if closed { k.max(3) } else { k };
            let verts = if closed { edges } else { edges + 1 };
            (
                Just(closed),
                Just(edges),
                prop::collection::vec((-2.0f64..2.0, -2.0f64..2.0), verts),
                Just((0..verts).collect::<Vec<_>>()).prop_shuffle(),
                Just((0..edges).collect::<Vec<_>>()).prop_shuffle(),
                prop::collection::vec(any::<bool>(), edges),
                Just((0..edges).collect::<Vec<_>>()).prop_shuffle(),
                prop::collection::vec(0..edges, 0..4),
            )
        })
        .prop_map(
            |(closed, edges, offsets, vertex_order, edge_order, flips, input, repeats)| Case {
                closed,
                edges,
                offsets,
                vertex_order,
                edge_order,
                flips,
                input,
                repeats,
            },
        )
}

struct Built {
    ctx: Context,
    /// Vertex id of chain index i.
    vs: Vec<VertexId>,
    /// Edge id of chain edge i (joining chain vertices i and i + 1, wrapping when closed).
    es: Vec<EdgeId>,
}

fn build(c: &Case) -> Built {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let n = c.offsets.len();
    let mut vs = vec![VertexId(u64::MAX); n];
    for &i in &c.vertex_order {
        let (y, z) = c.offsets[i];
        vs[i] = ctx.insert_vertex(DVec3::new(i as f64, y, z), &mut log);
    }
    let mut es = vec![EdgeId(u64::MAX); c.edges];
    for &i in &c.edge_order {
        let (a, b) = (vs[i], vs[(i + 1) % n]);
        let (a, b) = if c.flips[i] { (b, a) } else { (a, b) };
        es[i] = ctx.insert_edge(a, b, &mut log).unwrap();
    }
    Built { ctx, vs, es }
}

fn given(c: &Case, b: &Built) -> Vec<EdgeId> {
    let mut out: Vec<EdgeId> = c.input.iter().map(|i| b.es[*i]).collect();
    out.extend(c.repeats.iter().map(|i| b.es[*i]));
    out
}

/// `path` is a well-formed chain over exactly `edges` in `ctx`.
fn check_chain(
    ctx: &Context,
    path: &SweepPath,
    edges: &BTreeSet<EdgeId>,
) -> Result<(), TestCaseError> {
    let n = path.vertices.len();
    let m = path.edges.len();
    prop_assert_eq!(path.points.len(), n);
    if path.closed {
        prop_assert_eq!(n, m);
    } else {
        prop_assert_eq!(n, m + 1);
    }
    let distinct: BTreeSet<VertexId> = path.vertices.iter().copied().collect();
    prop_assert_eq!(distinct.len(), n, "vertices repeat");
    let used: BTreeSet<EdgeId> = path.edges.iter().copied().collect();
    prop_assert_eq!(used.len(), m, "edges repeat");
    prop_assert_eq!(&used, edges);
    for (i, e) in path.edges.iter().enumerate() {
        let ends = ctx.edge(*e).unwrap().ends;
        let (a, b) = (path.vertices[i], path.vertices[(i + 1) % n]);
        prop_assert!(
            ends == [a, b] || ends == [b, a],
            "edge {i} doesn't join its vertices"
        );
    }
    for (id, p) in path.vertices.iter().zip(&path.points) {
        prop_assert_eq!(ctx.vertex(*id).unwrap().position, *p);
    }
    let length: f64 = path
        .edges
        .iter()
        .map(|e| {
            let [a, b] = ctx.edge(*e).unwrap().ends;
            ctx.vertex(a)
                .unwrap()
                .position
                .distance(ctx.vertex(b).unwrap().position)
        })
        .sum();
    prop_assert!((path.length() - length).abs() < 1e-9);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn any_order_gives_the_canonical_chain(c in case()) {
        let b = build(&c);
        let input = given(&c, &b);
        let path = path_from_edges(&b.ctx, &input).unwrap();
        let all: BTreeSet<EdgeId> = b.es.iter().copied().collect();
        check_chain(&b.ctx, &path, &all)?;
        prop_assert_eq!(path.closed, c.closed);
        if c.closed {
            let lowest = *b.vs.iter().min().unwrap();
            prop_assert_eq!(path.vertices[0], lowest);
            prop_assert!(path.edges[0] < *path.edges.last().unwrap());
        } else {
            let lower = b.vs[0].min(*b.vs.last().unwrap());
            prop_assert_eq!(path.vertices[0], lower);
        }
        // The edges in creation order give the same path.
        let sorted: Vec<EdgeId> = all.iter().copied().collect();
        prop_assert_eq!(path_from_edges(&b.ctx, &sorted).unwrap(), path);
    }

    #[test]
    fn reversing_keeps_the_chain(c in case()) {
        let b = build(&c);
        let path = path_from_edges(&b.ctx, &given(&c, &b)).unwrap();
        let back = path.reversed();
        let all: BTreeSet<EdgeId> = b.es.iter().copied().collect();
        check_chain(&b.ctx, &back, &all)?;
        prop_assert_eq!(back.closed, path.closed);
        if path.closed {
            prop_assert_eq!(back.vertices[0], path.vertices[0]);
        } else {
            prop_assert_eq!(back.vertices[0], *path.vertices.last().unwrap());
        }
        prop_assert!((back.length() - path.length()).abs() < 1e-9);
        prop_assert_eq!(back.reversed(), path);
    }

    #[test]
    fn leaving_out_edges_splits_into_pieces(c in case(), cut in any::<prop::sample::Index>()) {
        let b = build(&c);
        // Open: drop one inner edge (two pieces). Closed: drop two edges that don't touch.
        let dropped: Vec<usize> = if c.closed {
            if c.edges < 4 {
                return Ok(());
            }
            let first = cut.index(c.edges);
            vec![first, (first + 2) % c.edges]
        } else {
            if c.edges < 3 {
                return Ok(());
            }
            vec![1 + cut.index(c.edges - 2)]
        };
        let input: Vec<EdgeId> = given(&c, &b)
            .into_iter()
            .filter(|e| !dropped.iter().any(|d| b.es[*d] == *e))
            .collect();
        prop_assert_eq!(path_from_edges(&b.ctx, &input), Err(PathError::Gap { pieces: 2 }));
    }

    #[test]
    fn a_spur_on_an_inner_vertex_is_a_branch(c in case(), at in any::<prop::sample::Index>()) {
        let mut b = build(&c);
        let i = at.index(b.vs.len());
        let spur_end = b.ctx.insert_vertex(DVec3::new(-5.0, 9.0, 9.0), &mut Edit::new());
        let spur = b.ctx.insert_edge(b.vs[i], spur_end, &mut Edit::new()).unwrap();
        let mut input = given(&c, &b);
        input.push(spur);
        let result = path_from_edges(&b.ctx, &input);
        let is_end = !c.closed && (i == 0 || i == b.vs.len() - 1);
        if is_end {
            // The spur just makes the open chain one edge longer.
            let path = result.unwrap();
            let mut all: BTreeSet<EdgeId> = b.es.iter().copied().collect();
            all.insert(spur);
            check_chain(&b.ctx, &path, &all)?;
            prop_assert!(!path.closed);
        } else {
            prop_assert_eq!(result, Err(PathError::Branch(b.vs[i])));
        }
    }
}
