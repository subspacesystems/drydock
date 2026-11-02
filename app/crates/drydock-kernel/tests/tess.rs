// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-kernel-scene: `triangulate` on hand-made polygons.

mod tess_common;

use drydock_kernel::tess::triangulate;
use glam::DVec2;
use tess_common::{check, d2};

fn square() -> Vec<DVec2> {
    d2(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
}

/// Regular polygon, counter-clockwise.
fn circle(cx: f64, cy: f64, r: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            DVec2::new(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn reversed(mut p: Vec<DVec2>) -> Vec<DVec2> {
    p.reverse();
    p
}

/// `p` with `k` evenly spaced extra points on every edge.
fn with_collinear(p: &[DVec2], k: usize) -> Vec<DVec2> {
    let mut out = Vec::new();
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        out.push(a);
        for j in 1..=k {
            out.push(a.lerp(b, j as f64 / (k + 1) as f64));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// No holes

#[test]
fn triangle_counter_clockwise() {
    let t = d2(&[(0.0, 0.0), (2.0, 0.0), (0.0, 1.0)]);
    let tris = check(&t, &[]);
    assert_eq!(tris.len(), 1);
}

#[test]
fn triangle_clockwise_comes_out_counter_clockwise() {
    let t = d2(&[(0.0, 0.0), (0.0, 1.0), (2.0, 0.0)]);
    let tris = check(&t, &[]);
    assert_eq!(tris.len(), 1);
}

#[test]
fn unit_square() {
    assert_eq!(check(&square(), &[]).len(), 2);
}

#[test]
fn clockwise_square() {
    assert_eq!(check(&reversed(square()), &[]).len(), 2);
}

#[test]
fn square_far_from_origin() {
    let p: Vec<DVec2> = square()
        .into_iter()
        .map(|q| q + DVec2::new(-350.0, 1200.0))
        .collect();
    assert_eq!(check(&p, &[]).len(), 2);
}

#[test]
fn tiny_square() {
    let p: Vec<DVec2> = square().into_iter().map(|q| q * 1e-3).collect();
    assert_eq!(check(&p, &[]).len(), 2);
}

#[test]
fn convex_polygon() {
    assert_eq!(check(&circle(0.0, 0.0, 1.0, 12), &[]).len(), 10);
}

#[test]
fn split_edge_gives_a_straight_corner() {
    // A square whose bottom edge was split at its middle.
    let p = d2(&[(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
    assert_eq!(check(&p, &[]).len(), 3);
}

#[test]
fn split_edge_at_the_start() {
    // The straight corner is point 0.
    let p = d2(&[(0.5, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
    assert_eq!(check(&p, &[]).len(), 3);
}

#[test]
fn collinear_points_on_every_side() {
    let p = with_collinear(&square(), 3);
    assert_eq!(p.len(), 16);
    assert_eq!(check(&p, &[]).len(), 14);
}

#[test]
fn long_collinear_run() {
    // A thin rectangle with 100 points along the bottom: only a fan from the top works.
    let mut p: Vec<DVec2> = (0..=100).map(|i| DVec2::new(i as f64, 0.0)).collect();
    p.push(DVec2::new(100.0, 1.0));
    p.push(DVec2::new(0.0, 1.0));
    assert_eq!(check(&p, &[]).len(), 101);
}

#[test]
fn collinear_runs_on_both_long_sides() {
    let mut p: Vec<DVec2> = (0..=20).map(|i| DVec2::new(i as f64, 0.0)).collect();
    p.extend((0..=20).rev().map(|i| DVec2::new(i as f64, 0.5)));
    assert_eq!(check(&p, &[]).len(), 40);
}

#[test]
fn l_shape() {
    let p = d2(&[
        (0.0, 0.0),
        (2.0, 0.0),
        (2.0, 1.0),
        (1.0, 1.0),
        (1.0, 2.0),
        (0.0, 2.0),
    ]);
    assert_eq!(check(&p, &[]).len(), 4);
}

#[test]
fn l_shape_clockwise_with_collinear_points() {
    let p = with_collinear(
        &d2(&[
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ]),
        1,
    );
    let p = reversed(p);
    assert_eq!(check(&p, &[]).len(), 10);
}

#[test]
fn comb() {
    // Five teeth pointing up from a bar.
    let mut p = vec![DVec2::new(0.0, 0.0), DVec2::new(9.0, 0.0)];
    for t in (0..5).rev() {
        let x = t as f64 * 2.0;
        p.push(DVec2::new(x + 1.0, 1.0));
        p.push(DVec2::new(x + 1.0, 4.0));
        p.push(DVec2::new(x, 4.0));
        if t > 0 {
            p.push(DVec2::new(x, 1.0));
        }
    }
    let n = p.len();
    assert_eq!(check(&p, &[]).len(), n - 2);
}

#[test]
fn spiral() {
    // A square spiral corridor, width 1, wound inwards: very concave.
    let pts = [
        (0.0, 0.0),
        (7.0, 0.0),
        (7.0, 7.0),
        (1.0, 7.0),
        (1.0, 2.0),
        (5.0, 2.0),
        (5.0, 5.0),
        (3.0, 5.0),
        (3.0, 4.0),
        (4.0, 4.0),
        (4.0, 3.0),
        (2.0, 3.0),
        (2.0, 6.0),
        (6.0, 6.0),
        (6.0, 1.0),
        (0.0, 1.0),
    ];
    let p = d2(&pts);
    assert_eq!(check(&p, &[]).len(), 14);
    assert_eq!(check(&reversed(p), &[]).len(), 14);
}

#[test]
fn star() {
    let p: Vec<DVec2> = (0..16)
        .map(|i| {
            let a = i as f64 / 16.0 * std::f64::consts::TAU;
            let r = if i % 2 == 0 { 3.0 } else { 1.0 };
            DVec2::new(r * a.cos(), r * a.sin())
        })
        .collect();
    assert_eq!(check(&p, &[]).len(), 14);
}

#[test]
fn reflex_corner_next_to_collinear_run() {
    // A notch whose sides are split.
    let p = d2(&[
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (2.0, 2.0),
        (1.5, 2.0),
        (1.0, 1.0),
        (1.0, 1.5),
        (1.0, 2.0),
        (0.5, 2.0),
        (0.0, 2.0),
    ]);
    assert_eq!(check(&p, &[]).len(), 8);
}

// ---------------------------------------------------------------------------------------
// Holes

#[test]
fn square_with_square_hole() {
    let outer = d2(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)]);
    let hole = d2(&[(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0)]);
    assert_eq!(check(&outer, &[hole]).len(), 8);
}

#[test]
fn hole_winding_does_not_matter() {
    let outer = d2(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)]);
    let hole = d2(&[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)]);
    assert_eq!(check(&outer, std::slice::from_ref(&hole)).len(), 8);
    assert_eq!(check(&reversed(outer), &[hole]).len(), 8);
}

#[test]
fn hole_points_are_numbered_after_outer() {
    let outer = d2(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)]);
    let hole = d2(&[(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0)]);
    let tris = triangulate(&outer, &[hole]);
    let used: std::collections::BTreeSet<usize> = tris.iter().flatten().copied().collect();
    assert_eq!(used, (0..8).collect());
}

#[test]
fn two_holes() {
    let outer = d2(&[(0.0, 0.0), (5.0, 0.0), (5.0, 2.0), (0.0, 2.0)]);
    let h1 = d2(&[(1.0, 0.5), (1.0, 1.5), (2.0, 1.5), (2.0, 0.5)]);
    let h2 = d2(&[(3.0, 0.5), (3.0, 1.5), (4.0, 1.5), (4.0, 0.5)]);
    assert_eq!(check(&outer, &[h1, h2]).len(), 12 + 4 - 2);
}

#[test]
fn grid_of_holes() {
    let outer = d2(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    let mut holes = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            let (x, y) = (0.5 + i as f64 * 1.2, 0.5 + j as f64 * 1.2);
            holes.push(d2(&[
                (x, y),
                (x, y + 0.6),
                (x + 0.6, y + 0.6),
                (x + 0.6, y),
            ]));
        }
    }
    assert_eq!(check(&outer, &holes).len(), 40 + 18 - 2);
}

#[test]
fn holes_in_a_row_hide_each_other() {
    // Bridges to the outer boundary must not cross other holes.
    let outer = d2(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)]);
    let holes: Vec<Vec<DVec2>> = (0..8)
        .map(|i| {
            let x = 1.0 + i as f64;
            d2(&[(x, 0.4), (x, 0.6), (x + 0.5, 0.6), (x + 0.5, 0.4)])
        })
        .collect();
    assert_eq!(check(&outer, &holes).len(), 36 + 16 - 2);
}

#[test]
fn hole_near_a_reflex_corner() {
    // A U shape with a hole in one arm, close to the inner corner.
    let outer = d2(&[
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 3.0),
        (2.0, 3.0),
        (2.0, 1.0),
        (1.0, 1.0),
        (1.0, 3.0),
        (0.0, 3.0),
    ]);
    let hole = d2(&[(2.2, 1.05), (2.2, 1.4), (2.6, 1.4), (2.6, 1.05)]);
    assert_eq!(check(&outer, &[hole]).len(), 12 + 2 - 2);
}

#[test]
fn hole_inside_a_concave_pocket() {
    // The hole sits where the nearest outer points are hidden behind a reflex corner.
    let outer = d2(&[
        (0.0, 0.0),
        (6.0, 0.0),
        (6.0, 4.0),
        (4.0, 4.0),
        (4.0, 1.0),
        (3.0, 1.0),
        (3.0, 4.0),
        (0.0, 4.0),
    ]);
    let hole = d2(&[(1.0, 2.0), (1.0, 3.0), (2.0, 3.0), (2.0, 2.0)]);
    assert_eq!(check(&outer, &[hole]).len(), 12 + 2 - 2);
}

#[test]
fn hole_behind_a_reflex_corner() {
    // From the hole, the far end of the slanted edge straight to its right is hidden
    // behind the reflex corner (4, 3).
    let outer = d2(&[
        (0.0, 0.0),
        (3.0, 0.0),
        (9.0, 8.0),
        (5.0, 8.0),
        (4.0, 3.0),
        (3.0, 8.0),
        (0.0, 8.0),
    ]);
    let hole = d2(&[(0.5, 0.5), (0.5, 1.0), (1.0, 1.0), (1.0, 0.5)]);
    assert_eq!(check(&outer, &[hole]).len(), 11);
}

#[test]
fn two_holes_bridge_to_one_corner() {
    // Both holes see the corner (5, 2) best, so it gets joined to two holes.
    let outer = d2(&[(0.0, 0.0), (4.0, 0.0), (5.0, 2.0), (4.0, 4.0), (0.0, 4.0)]);
    let a = d2(&[(1.0, 0.5), (1.0, 1.0), (1.5, 1.0), (1.5, 0.5)]);
    let b = d2(&[(1.0, 3.0), (1.0, 3.5), (1.5, 3.5), (1.5, 3.0)]);
    assert_eq!(check(&outer, &[a.clone(), b.clone()]).len(), 15);
    assert_eq!(check(&outer, &[b, a]).len(), 15);
}

#[test]
fn round_hole_with_collinear_outer() {
    let outer = with_collinear(
        &d2(&[(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)]),
        2,
    );
    let hole = reversed(circle(0.0, 0.0, 1.0, 24));
    assert_eq!(check(&outer, &[hole]).len(), 12 + 24 + 2 - 2);
}

#[test]
fn hole_with_collinear_points() {
    let outer = d2(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)]);
    let hole = with_collinear(&d2(&[(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0)]), 2);
    assert_eq!(check(&outer, &[hole]).len(), 16 + 2 - 2);
}

#[test]
fn hole_vertex_level_with_outer_vertex() {
    // Hole corners share x or y with outer corners (ties when picking bridge points).
    let outer = d2(&[
        (0.0, 0.0),
        (2.0, 0.0),
        (2.0, 1.0),
        (3.0, 1.0),
        (3.0, 3.0),
        (0.0, 3.0),
    ]);
    let hole = d2(&[(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.5)]);
    assert_eq!(check(&outer, &[hole]).len(), 10 + 2 - 2);
}

#[test]
fn ring_of_round_holes() {
    let outer = circle(0.0, 0.0, 10.0, 64);
    let holes: Vec<Vec<DVec2>> = (0..6)
        .map(|i| {
            let a = i as f64 / 6.0 * std::f64::consts::TAU;
            circle(5.0 * a.cos(), 5.0 * a.sin(), 1.0, 12)
        })
        .collect();
    assert_eq!(check(&outer, &holes).len(), 64 + 72 + 12 - 2);
}

// ---------------------------------------------------------------------------------------
// Degenerate input and speed

#[test]
fn too_few_points_give_nothing() {
    assert!(triangulate(&[], &[]).is_empty());
    assert!(triangulate(&d2(&[(0.0, 0.0)]), &[]).is_empty());
    assert!(triangulate(&d2(&[(0.0, 0.0), (1.0, 0.0)]), &[]).is_empty());
    assert!(triangulate(&d2(&[(0.0, 0.0), (1.0, 0.0)]), &[square()]).is_empty());
}

#[test]
fn short_holes_are_ignored() {
    let outer = d2(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)]);
    let short = d2(&[(1.0, 1.0), (2.0, 2.0)]);
    let tris = triangulate(&outer, &[short.clone(), Vec::new()]);
    assert_eq!(tris.len(), 2);
    assert!(tris.iter().flatten().all(|&i| i < 4));

    // A real hole after a short one is numbered after the short one's points.
    let hole = d2(&[(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0)]);
    let tris = triangulate(&outer, &[short, hole]);
    assert_eq!(tris.len(), 8);
    let used: std::collections::BTreeSet<usize> = tris.iter().flatten().copied().collect();
    assert_eq!(used, [0, 1, 2, 3, 6, 7, 8, 9].into_iter().collect());
}

#[test]
fn bad_input_does_not_panic() {
    let nan = DVec2::new(f64::NAN, 0.0);
    let inf = DVec2::new(f64::INFINITY, 1.0);
    let cases: Vec<(Vec<DVec2>, Vec<Vec<DVec2>>)> = vec![
        (d2(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), vec![]),
        (
            d2(&[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
            vec![],
        ),
        (vec![DVec2::ZERO, nan, DVec2::ONE, DVec2::X], vec![]),
        (vec![DVec2::ZERO, inf, DVec2::ONE], vec![square()]),
        // Self-intersecting bow tie.
        (
            d2(&[(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)]),
            vec![],
        ),
        // Hole outside, hole overlapping the boundary, hole equal to the outer polygon.
        (square(), vec![d2(&[(5.0, 5.0), (6.0, 5.0), (6.0, 6.0)])]),
        (square(), vec![d2(&[(0.5, 0.5), (1.5, 0.5), (1.5, 1.5)])]),
        (square(), vec![square()]),
        // Repeated points.
        (
            d2(&[(0.0, 0.0), (1.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
            vec![],
        ),
    ];
    for (outer, holes) in cases {
        let n = outer.len() + holes.iter().map(Vec::len).sum::<usize>();
        for t in triangulate(&outer, &holes) {
            assert!(
                t.iter().all(|&i| i < n),
                "{t:?} out of range for {n} points"
            );
        }
    }
}

#[test]
fn big_polygon_with_many_holes_is_fast() {
    // 1000-point wavy outer boundary, 10 holes of 40 points.
    let outer: Vec<DVec2> = (0..1000)
        .map(|i| {
            let a = i as f64 / 1000.0 * std::f64::consts::TAU;
            let r = 10.0 + 0.3 * (a * 37.0).sin();
            DVec2::new(r * a.cos(), r * a.sin())
        })
        .collect();
    let holes: Vec<Vec<DVec2>> = (0..10)
        .map(|i| {
            let a = i as f64 / 10.0 * std::f64::consts::TAU;
            reversed(circle(5.0 * a.cos(), 5.0 * a.sin(), 0.8, 40))
        })
        .collect();
    let started = std::time::Instant::now();
    let tris = triangulate(&outer, &holes);
    let took = started.elapsed().as_secs_f64();
    assert!(took < 5.0, "took {took:.2} s");
    assert_eq!(tris.len(), 1400 + 20 - 2);
    check(&outer, &holes);
}
