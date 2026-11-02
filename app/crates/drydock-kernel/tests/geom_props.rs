// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use drydock_kernel::geom::*;
use glam::{DMat3, DQuat, DVec3};
use proptest::prelude::*;

const T: f64 = LINEAR_TOL;

fn vec3(range: f64) -> impl Strategy<Value = DVec3> {
    (-range..range, -range..range, -range..range).prop_map(|(x, y, z)| DVec3::new(x, y, z))
}

fn rotation() -> impl Strategy<Value = DQuat> {
    (vec3(1.0), -3.0..3.0f64).prop_filter_map("axis", |(a, ang)| {
        (a.length() > 0.1).then(|| DQuat::from_axis_angle(a.normalize(), ang))
    })
}

/// A regular n-gon (radius 0.5..5) in the XY plane, counter-clockwise, then moved rigidly.
fn rigid_ngon() -> impl Strategy<Value = (Vec<DVec3>, DQuat, DVec3)> {
    (3usize..12, 0.5..5.0f64, rotation(), vec3(100.0)).prop_map(|(n, r, q, t)| {
        let pts = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                q * DVec3::new(r * a.cos(), r * a.sin(), 0.0) + t
            })
            .collect();
        (pts, q, t)
    })
}

proptest! {
    #[test]
    fn rigid_motion_keeps_polygon_planar_with_rotated_normal((pts, q, _t) in rigid_ngon()) {
        prop_assert!(is_planar(&pts, T));
        let n = polygon_normal(&pts);
        prop_assert!((n - q * DVec3::Z).length() < 1e-9);
        let plane = fit_plane(&pts).unwrap();
        prop_assert!(max_plane_deviation(&pts, &plane) < T);
    }

    #[test]
    fn lifting_one_vertex_breaks_planarity((pts, _q, _t) in rigid_ngon(), k in 0usize..12, lift in 20.0..100.0f64) {
        let n = polygon_normal(&pts);
        let mut bad = pts.clone();
        let i = k % bad.len();
        // Four or more points only; a triangle stays planar.
        prop_assume!(bad.len() >= 4);
        bad[i] += n * lift * T;
        prop_assert!(!is_planar(&bad, T));
    }

    #[test]
    fn plane_2d_area_matches_3d_area((pts, _q, _t) in rigid_ngon()) {
        let n = polygon_normal(&pts);
        let flat = to_plane_2d(&pts, n);
        let a3 = area_vector(&pts).length();
        prop_assert!((signed_area_2d(&flat) - a3).abs() < 1e-7 * (1.0 + a3));
        prop_assert!(polygon_is_simple_2d(&flat, T));
    }

    #[test]
    fn plane_basis_is_a_rotation(n in vec3(1.0).prop_filter("nonzero", |v| v.length() > 0.1)) {
        let n = n.normalize();
        let (u, w) = plane_basis(n);
        let m = DMat3::from_cols(u, w, n);
        prop_assert!((m.determinant() - 1.0).abs() < 1e-9);
        prop_assert!((m.transpose() * m - DMat3::IDENTITY).to_cols_array().iter().all(|x| x.abs() < 1e-9));
    }

    /// Two lines through a chosen point cross there; swapping the segments swaps ta/tb.
    #[test]
    fn constructed_crossing_is_found(
        p in vec3(50.0),
        d1 in vec3(1.0).prop_filter("len", |v| v.length() > 0.2),
        d2 in vec3(1.0).prop_filter("len", |v| v.length() > 0.2),
        s0 in 0.1..5.0f64, s1 in 0.1..5.0f64, u0 in 0.1..5.0f64, u1 in 0.1..5.0f64,
    ) {
        let (d1, d2) = (d1.normalize(), d2.normalize());
        prop_assume!(d1.cross(d2).length() > 0.05);
        let (a0, a1) = (p - d1 * s0, p + d1 * s1);
        let (b0, b1) = (p - d2 * u0, p + d2 * u1);
        let ab = segment_intersection(a0, a1, b0, b1, T);
        let ba = segment_intersection(b0, b1, a0, a1, T);
        match (ab, ba) {
            (SegmentHit::Point { ta, tb }, SegmentHit::Point { ta: ta2, tb: tb2 }) => {
                prop_assert!((ta - s0 / (s0 + s1)).abs() < 1e-6);
                prop_assert!((tb - u0 / (u0 + u1)).abs() < 1e-6);
                prop_assert!((ta - tb2).abs() < 1e-9 && (tb - ta2).abs() < 1e-9);
            }
            other => prop_assert!(false, "{:?}", other),
        }
    }

    /// Moving b away from a along the common normal by 10 tolerances removes the hit.
    #[test]
    fn lifted_crossing_is_not_a_hit(
        p in vec3(50.0),
        d1 in vec3(1.0).prop_filter("len", |v| v.length() > 0.2),
        d2 in vec3(1.0).prop_filter("len", |v| v.length() > 0.2),
    ) {
        let (d1, d2) = (d1.normalize(), d2.normalize());
        prop_assume!(d1.cross(d2).length() > 0.05);
        let n = d1.cross(d2).normalize();
        let lift = n * 10.0 * T;
        let hit = segment_intersection(p - d1, p + d1, p - d2 + lift, p + d2 + lift, T);
        prop_assert_eq!(hit, SegmentHit::None);
    }

    /// Collinear overlap length is symmetric and parameters map to the same points.
    #[test]
    fn collinear_overlap_points_agree(
        o in vec3(50.0),
        d in vec3(1.0).prop_filter("len", |v| v.length() > 0.5),
        a1 in 1.0..10.0f64, b0 in -5.0..5.0f64, b1 in -5.0..15.0f64,
    ) {
        prop_assume!((b1 - b0).abs() > 0.5);
        let d = d.normalize();
        let (pa0, pa1) = (o, o + d * a1);
        let (pb0, pb1) = (o + d * b0, o + d * b1);
        if let SegmentHit::Overlap { a_from, a_to, b_from, b_to } = segment_intersection(pa0, pa1, pb0, pb1, T) {
            prop_assert!(a_from < a_to);
            let on_a = |t: f64| pa0 + (pa1 - pa0) * t;
            let on_b = |t: f64| pb0 + (pb1 - pb0) * t;
            prop_assert!(on_a(a_from).distance(on_b(b_from)) < 10.0 * T);
            prop_assert!(on_a(a_to).distance(on_b(b_to)) < 10.0 * T);
            let lo = b0.min(b1).max(0.0);
            let hi = b0.max(b1).min(a1);
            prop_assert!((a_from * a1 - lo).abs() < 10.0 * T && (a_to * a1 - hi).abs() < 10.0 * T);
        } else {
            let lo = b0.min(b1).max(0.0);
            let hi = b0.max(b1).min(a1);
            prop_assert!(hi - lo <= 10.0 * T);
        }
    }
}
