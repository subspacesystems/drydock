// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Shape point invariants (`docs/specs/m2-shapes.md`).

use drydock_kernel::shapes::{arc, circle, rectangle, sweep_angle};
use glam::{DQuat, DVec3};
use proptest::prelude::*;
use std::f64::consts::TAU;

fn vec3(range: f64) -> impl Strategy<Value = DVec3> {
    (-range..range, -range..range, -range..range).prop_map(|(x, y, z)| DVec3::new(x, y, z))
}

/// A unit frame (x, y, normal), right-handed, in a random orientation.
fn frame() -> impl Strategy<Value = (DVec3, DVec3, DVec3)> {
    (vec3(1.0), -3.0..3.0f64).prop_filter_map("axis", |(a, ang)| {
        (a.length() > 0.1).then(|| {
            let q = DQuat::from_axis_angle(a.normalize(), ang);
            (q * DVec3::X, q * DVec3::Y, q * DVec3::Z)
        })
    })
}

proptest! {
    #[test]
    fn rectangle_is_a_planar_rectangle(
        (u, v, n) in frame(),
        a in vec3(50.0),
        b in vec3(50.0),
    ) {
        if let Some(r) = rectangle(a, b, u, v) {
            for p in &r {
                prop_assert!(n.dot(*p - a).abs() < 1e-9);
            }
            // opposite sides equal and parallel, adjacent sides perpendicular
            prop_assert!(((r[1] - r[0]) - (r[2] - r[3])).length() < 1e-9);
            prop_assert!(((r[3] - r[0]) - (r[2] - r[1])).length() < 1e-9);
            prop_assert!((r[1] - r[0]).dot(r[3] - r[0]).abs() < 1e-9);
            // the far corner is b's shadow in the plane
            prop_assert!((r[2] - (b - n * n.dot(b - a))).length() < 1e-9);
            prop_assert_eq!(r[0], a);
        } else {
            let d = b - a;
            prop_assert!(d.dot(u).abs() <= 1e-5 || d.dot(v).abs() <= 1e-5);
        }
    }

    #[test]
    fn circle_points_are_on_the_circle_and_evenly_spaced(
        (x, _y, n) in frame(),
        center in vec3(50.0),
        radius in 0.01..20.0f64,
        segments in 3usize..64,
    ) {
        let pts = circle(center, n, x, radius, segments);
        prop_assert_eq!(pts.len(), segments);
        let side = 2.0 * radius * (std::f64::consts::PI / segments as f64).sin();
        for (i, p) in pts.iter().enumerate() {
            prop_assert!(((*p - center).length() - radius).abs() < 1e-9);
            prop_assert!(n.dot(*p - center).abs() < 1e-9);
            let next = pts[(i + 1) % segments];
            prop_assert!(((next - *p).length() - side).abs() < 1e-9);
            // counter-clockwise about n
            prop_assert!(n.dot((*p - center).cross(next - center)) > 0.0);
        }
        prop_assert!((pts[0] - (center + x * radius)).length() < 1e-9);
    }

    #[test]
    fn arc_steps_are_equal_and_end_where_the_sweep_ends(
        (x, y, n) in frame(),
        center in vec3(50.0),
        radius in 0.01..20.0f64,
        start in -7.0..7.0f64,
        sweep in prop_oneof![0.01..TAU, -TAU..-0.01],
        segments in 1usize..64,
    ) {
        let pts = arc(center, n, x, radius, start, sweep, segments);
        prop_assert_eq!(pts.len(), segments + 1);
        let at = |t: f64| center + (x * t.cos() + y * t.sin()) * radius;
        prop_assert!((pts[0] - at(start)).length() < 1e-8);
        prop_assert!((pts[segments] - at(start + sweep)).length() < 1e-8);
        let chord = 2.0 * radius * (sweep.abs() / segments as f64 / 2.0).sin().abs();
        for w in pts.windows(2) {
            prop_assert!(((w[1] - w[0]).length() - chord).abs() < 1e-8);
        }
    }

    #[test]
    fn sweep_angle_matches_the_arc_it_describes(
        (x, y, n) in frame(),
        center in vec3(50.0),
        r1 in 0.1..20.0f64,
        r2 in 0.1..20.0f64,
        h1 in -5.0..5.0f64,
        h2 in -5.0..5.0f64,
        t1 in 0.0..TAU,
        t2 in 0.0..TAU,
    ) {
        let from = center + (x * t1.cos() + y * t1.sin()) * r1 + n * h1;
        let to = center + (x * t2.cos() + y * t2.sin()) * r2 + n * h2;
        let got = sweep_angle(center, n, from, to).unwrap();
        prop_assert!((0.0..TAU).contains(&got));
        let want = (t2 - t1).rem_euclid(TAU);
        let diff = (got - want).abs();
        prop_assert!(diff < 1e-8 || (TAU - diff) < 1e-8, "got {got}, want {want}");
        // the other way round adds up to a full turn
        let back = sweep_angle(center, n, to, from).unwrap();
        if got > 1e-6 && got < TAU - 1e-6 {
            prop_assert!((got + back - TAU).abs() < 1e-8);
        }
    }
}
