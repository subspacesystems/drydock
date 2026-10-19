// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Checks shared by the triangulation tests: rules 1 to 4 of `docs/specs/m1-kernel-scene.md`.

#![allow(dead_code)]

use std::collections::BTreeSet;

use glam::DVec2;
use modeler_kernel::geom::{
    Containment, point_in_polygon_2d, polygon_is_simple_2d, signed_area_2d,
};
use modeler_kernel::tess::triangulate;

pub fn d2(pts: &[(f64, f64)]) -> Vec<DVec2> {
    pts.iter().map(|&(x, y)| DVec2::new(x, y)).collect()
}

/// Samples per axis for the coverage check.
const GRID: usize = 48;

fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn polygon_distance(p: DVec2, poly: &[DVec2]) -> f64 {
    (0..poly.len())
        .map(|i| segment_distance(p, poly[i], poly[(i + 1) % poly.len()]))
        .fold(f64::INFINITY, f64::min)
}

/// Twice the signed area of `a b c`, measured from `a` (robust far from the origin).
fn cross(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b - a).perp_dot(c - a)
}

/// Unsigned area, measured from the first point (robust far from the origin).
fn area(poly: &[DVec2]) -> f64 {
    let Some(&first) = poly.first() else {
        return 0.0;
    };
    let local: Vec<DVec2> = poly.iter().map(|&p| p - first).collect();
    signed_area_2d(&local).abs()
}

/// Triangulate valid input and check every rule; returns the triangles.
pub fn check(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[usize; 3]> {
    // The test data itself must be valid input.
    assert!(
        polygon_is_simple_2d(outer, 1e-9),
        "test outer is not simple"
    );
    for (k, h) in holes.iter().enumerate() {
        assert!(polygon_is_simple_2d(h, 1e-9), "test hole {k} is not simple");
        for &p in h {
            assert_eq!(
                point_in_polygon_2d(p, outer, 1e-9),
                Containment::Inside,
                "test hole {k} is not inside the outer polygon"
            );
        }
    }

    let tris = triangulate(outer, holes);

    let points: Vec<DVec2> = outer
        .iter()
        .chain(holes.iter().flatten())
        .copied()
        .collect();
    let n = points.len();
    let h = holes.len();

    // 1. count, distinct indices in range, every point used
    assert_eq!(
        tris.len(),
        n + 2 * h - 2,
        "{n} points and {h} holes need {} triangles, got {}",
        n + 2 * h - 2,
        tris.len()
    );
    let mut used = BTreeSet::new();
    for t in &tris {
        assert!(
            t.iter().all(|&i| i < n),
            "{t:?}: index out of range ({n} points)"
        );
        assert!(
            t[0] != t[1] && t[1] != t[2] && t[0] != t[2],
            "{t:?}: repeated index"
        );
        used.extend(t.iter().copied());
    }
    assert_eq!(used.len(), n, "some points are not used");

    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for &p in outer {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let d = lo.distance(hi);

    // 2. counter-clockwise (or flat)
    for t in &tris {
        let a2 = cross(points[t[0]], points[t[1]], points[t[2]]);
        assert!(
            a2 * 0.5 >= -1e-9 * d * d,
            "{t:?} is clockwise (area {})",
            a2 * 0.5
        );
    }

    // 3. total area
    let want = area(outer) - holes.iter().map(|h| area(h)).sum::<f64>();
    let got: f64 = tris
        .iter()
        .map(|t| cross(points[t[0]], points[t[1]], points[t[2]]) * 0.5)
        .sum();
    assert!(
        (got - want).abs() <= 1e-9 * want.abs(),
        "triangles cover area {got}, region has {want}"
    );

    // 4. coverage on a grid of sample points
    let margin = 1e-6 * d;
    let eps = 1e-12 * d * d;
    let boxes: Vec<(DVec2, DVec2)> = tris
        .iter()
        .map(|t| {
            let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
            (a.min(b).min(c), a.max(b).max(c))
        })
        .collect();
    let span = hi - lo;
    for i in 0..GRID {
        for j in 0..GRID {
            let f = DVec2::new(
                (i as f64 + 0.37) / GRID as f64 * 1.1 - 0.05,
                (j as f64 + 0.61) / GRID as f64 * 1.1 - 0.05,
            );
            let p = lo + span * f;
            let near = polygon_distance(p, outer) <= margin
                || holes.iter().any(|h| polygon_distance(p, h) <= margin);
            if near {
                continue;
            }
            let inside = point_in_polygon_2d(p, outer, 0.0) == Containment::Inside
                && holes
                    .iter()
                    .all(|h| point_in_polygon_2d(p, h, 0.0) == Containment::Outside);
            let covering = tris
                .iter()
                .zip(&boxes)
                .filter(|(_, (bl, bh))| p.cmpge(*bl - margin).all() && p.cmple(*bh + margin).all())
                .filter(|(t, _)| {
                    let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
                    cross(a, b, p) >= -eps && cross(b, c, p) >= -eps && cross(c, a, p) >= -eps
                })
                .count();
            if inside {
                assert!(covering >= 1, "{p:?} is inside but no triangle covers it");
            } else {
                assert_eq!(covering, 0, "{p:?} is outside but covered");
            }
        }
    }
    tris
}
