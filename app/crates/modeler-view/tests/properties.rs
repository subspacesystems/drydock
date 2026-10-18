// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-camera: invariants over random cameras. See docs/specs/m0-camera.md.

use std::f64::consts::{FRAC_PI_2, PI};

use glam::{DVec2, DVec3};
use modeler_view::{Bounds, Camera, Projection};
use proptest::prelude::*;

fn camera() -> impl Strategy<Value = Camera> {
    (
        (-100.0..100.0f64, -100.0..100.0f64, -100.0..100.0f64),
        0.05..1000.0f64,
        -2.0 * PI..2.0 * PI,
        -FRAC_PI_2..=FRAC_PI_2,
        10.0..90.0f64,
        any::<bool>(),
    )
        .prop_map(|((x, y, z), distance, yaw, pitch, fov, parallel)| Camera {
            target: DVec3::new(x, y, z),
            distance,
            yaw,
            pitch,
            fov_y: fov.to_radians(),
            projection: if parallel {
                Projection::Parallel
            } else {
                Projection::Perspective
            },
        })
}

fn ndc() -> impl Strategy<Value = DVec2> {
    (-0.95..0.95f64, -0.95..0.95f64).prop_map(|(x, y)| DVec2::new(x, y))
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn basis_is_orthonormal(c in camera()) {
        let (f, r, u) = c.basis();
        for v in [f, r, u] {
            prop_assert!((v.length() - 1.0).abs() < 1e-9);
        }
        prop_assert!(f.dot(r).abs() < 1e-9 && f.dot(u).abs() < 1e-9 && r.dot(u).abs() < 1e-9);
        prop_assert!((r.cross(u) - (-f)).length() < 1e-9, "right-handed: right x up points back at the eye");
        prop_assert!(rel((c.eye() - c.target).length(), c.distance) < 1e-9);
    }

    #[test]
    fn orbit_keeps_target_and_distance(c in camera(), dx in -2000.0..2000.0f64, dy in -2000.0..2000.0f64) {
        let mut o = c;
        o.orbit(dx, dy);
        prop_assert_eq!(o.target, c.target);
        prop_assert_eq!(o.distance, c.distance);
        prop_assert!(o.pitch >= -FRAC_PI_2 && o.pitch <= FRAC_PI_2);
        prop_assert!(rel((o.eye() - o.target).length(), c.distance) < 1e-9);
    }

    #[test]
    fn pan_tracks_the_cursor_on_the_target_plane(
        c in camera(), q in ndc(), dx in -300.0..300.0f64, dy in -300.0..300.0f64, aspect in 0.3..3.0f64,
    ) {
        let height = 800.0;
        let width = height * aspect;
        let p = c.target_plane_point(q, aspect);
        let mut moved = c;
        moved.pan(dx, dy, height);
        prop_assert_eq!(moved.basis(), c.basis());
        prop_assert_eq!(moved.distance, c.distance);
        let (after, _) = moved.project(p, aspect).unwrap();
        let expected = q + DVec2::new(2.0 * dx / width, -2.0 * dy / height);
        prop_assert!((after - expected).length() < 1e-6, "{} vs {}", after, expected);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor(c in camera(), q in ndc(), steps in -10.0..10.0f64, aspect in 0.3..3.0f64) {
        let p = c.target_plane_point(q, aspect);
        let mut z = c;
        z.zoom(steps, q, aspect);
        prop_assert_eq!(z.basis(), c.basis());
        let (after, _) = z.project(p, aspect).unwrap();
        prop_assert!((after - q).length() < 1e-6, "{} vs {}", after, q);
    }

    #[test]
    fn zoom_extents_frames_any_box(
        c in camera(),
        (x, y, z) in (-500.0..500.0f64, -500.0..500.0f64, -500.0..500.0f64),
        (sx, sy, sz) in (0.0..300.0f64, 0.0..300.0f64, 0.0..300.0f64),
        aspect in 0.3..3.0f64,
    ) {
        let b = Bounds { min: DVec3::new(x, y, z), max: DVec3::new(x + sx, y + sy, z + sz) };
        let mut e = c;
        e.zoom_extents(&b, aspect);
        prop_assert_eq!((e.yaw, e.pitch, e.fov_y, e.projection), (c.yaw, c.pitch, c.fov_y, c.projection));
        let (center, _) = e.project((b.min + b.max) / 2.0, aspect).unwrap();
        prop_assert!(center.length() < 1e-6);
        for i in 0..8 {
            let p = DVec3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            );
            let (ndc, depth) = e.project(p, aspect).unwrap();
            prop_assert!(depth > 0.0 || e.projection == Projection::Parallel);
            prop_assert!(ndc.abs().max_element() <= 1.0 + 1e-6, "{} -> {}", p, ndc);
        }
    }
}
