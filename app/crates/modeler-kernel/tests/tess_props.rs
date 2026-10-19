// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-kernel-scene: `triangulate` on random star-shaped polygons with collinear points
//! and holes.

mod tess_common;

use glam::DVec2;
use proptest::prelude::*;
use tess_common::check;

/// Hole cells: a 3x3 grid inside the square `[-0.35, 0.35]^2`, which lies inside every
/// generated outer polygon (they contain the disk of radius 0.5).
const CELL: f64 = 0.7 / 3.0;

#[derive(Clone, Debug)]
struct Case {
    /// Angle weights (normalised to a full turn) and radii of the outer corners.
    corners: Vec<(f64, f64)>,
    /// Extra collinear points per outer edge, cycled.
    extra: Vec<usize>,
    clockwise: bool,
    /// `(cell, corner count, radius fraction, rotation, clockwise, extra points per edge)`.
    holes: Vec<(usize, usize, f64, f64, bool, usize)>,
    offset: (f64, f64),
    scale: f64,
}

fn case() -> impl Strategy<Value = Case> {
    (
        prop::collection::vec((1.0..2.0f64, 1.0..3.0f64), 5..40),
        prop::collection::vec(prop_oneof![4 => Just(0usize), 1 => 1..4usize], 1..8),
        any::<bool>(),
        prop::collection::vec(
            (
                0..9usize,
                3..9usize,
                0.3..0.9f64,
                0.0..6.3f64,
                any::<bool>(),
                0..3usize,
            ),
            0..9,
        ),
        (-100.0..100.0f64, -100.0..100.0f64),
        prop_oneof![Just(1.0), 0.01..10.0f64],
    )
        .prop_map(|(corners, extra, clockwise, holes, offset, scale)| Case {
            corners,
            extra,
            clockwise,
            holes,
            offset,
            scale,
        })
}

fn with_collinear(p: &[DVec2], extra: &[usize]) -> Vec<DVec2> {
    let mut out = Vec::new();
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        out.push(a);
        let k = extra[i % extra.len()];
        for j in 1..=k {
            out.push(a.lerp(b, j as f64 / (k + 1) as f64));
        }
    }
    out
}

fn build(c: &Case) -> (Vec<DVec2>, Vec<Vec<DVec2>>) {
    let place = |p: DVec2| p * c.scale + DVec2::new(c.offset.0, c.offset.1);

    // Weights in [1, 2) and at least 5 corners keep every angle gap at most a third of a
    // turn, so the polygon is simple and contains the disk of radius 1 * cos(60) = 0.5.
    let total: f64 = c.corners.iter().map(|&(w, _)| w).sum();
    let mut angle = 0.0f64;
    let mut outer = Vec::new();
    for &(w, r) in &c.corners {
        outer.push(DVec2::new(r * angle.cos(), r * angle.sin()));
        angle += w / total * std::f64::consts::TAU;
    }
    let mut outer = with_collinear(&outer, &c.extra);
    if c.clockwise {
        outer.reverse();
    }

    let mut seen = [false; 9];
    let mut holes = Vec::new();
    for &(cell, corners, frac, rot, cw, extra) in &c.holes {
        if seen[cell] {
            continue;
        }
        seen[cell] = true;
        let centre = DVec2::new(
            -0.35 + CELL * ((cell % 3) as f64 + 0.5),
            -0.35 + CELL * ((cell / 3) as f64 + 0.5),
        );
        let r = frac * CELL * 0.45;
        let mut hole: Vec<DVec2> = (0..corners)
            .map(|i| {
                let a = rot + i as f64 / corners as f64 * std::f64::consts::TAU;
                centre + DVec2::new(a.cos(), a.sin()) * r
            })
            .collect();
        hole = with_collinear(&hole, &[extra]);
        if cw {
            hole.reverse();
        }
        holes.push(hole.into_iter().map(place).collect());
    }
    (outer.into_iter().map(place).collect(), holes)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn random_polygons_with_holes_tile_exactly(c in case()) {
        let (outer, holes) = build(&c);
        check(&outer, &holes);
    }

    #[test]
    fn random_polygons_without_holes_tile_exactly(c in case()) {
        let (outer, _) = build(&c);
        check(&outer, &[]);
    }
}
