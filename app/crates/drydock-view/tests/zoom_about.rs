// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-zoom-cursor: zooming about the model point under the cursor.
//! See docs/specs/m4-zoom-cursor.md.

use drydock_view::{
    Camera, MAX_DISTANCE, MIN_DISTANCE, Projection, StandardView, ZOOM_STEP_FACTOR,
};
use glam::{DVec2, DVec3};
use proptest::prelude::*;

const ASPECT: f64 = 1.5;

fn cam(projection: Projection) -> Camera {
    let mut c = Camera {
        target: DVec3::new(1.0, 2.0, 0.5),
        distance: 10.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection,
    };
    c.set_standard_view(StandardView::Iso);
    c
}

fn near(a: DVec3, b: DVec3, tol: f64) -> bool {
    (a - b).length() <= tol
}

/// A point that appears at `ndc`, `depth` in front of the eye.
fn point_at(c: &Camera, ndc: DVec2, depth: f64) -> DVec3 {
    let (forward, _, _) = c.basis();
    let q = c.target_plane_point(ndc, ASPECT);
    match c.projection {
        Projection::Perspective => {
            let eye = c.eye();
            let dir = q - eye;
            eye + dir * (depth / dir.dot(forward))
        }
        Projection::Parallel => q + forward * (depth - c.distance),
    }
}

#[test]
fn distance_scales_and_the_orientation_stays() {
    for projection in [Projection::Perspective, Projection::Parallel] {
        let before = cam(projection);
        let mut c = before;
        c.zoom_about(2.0, DVec3::new(3.0, -1.0, 0.0));
        let want = 10.0 * ZOOM_STEP_FACTOR * ZOOM_STEP_FACTOR;
        assert!(
            (c.distance - want).abs() < 1e-12,
            "{projection:?} {}",
            c.distance
        );
        assert_eq!(c.yaw, before.yaw);
        assert_eq!(c.pitch, before.pitch);
        assert_eq!(c.fov_y, before.fov_y);
        assert_eq!(c.projection, projection);
    }
}

#[test]
fn the_eye_and_target_scale_about_the_pivot() {
    let pivot = DVec3::new(3.0, -1.0, 0.25);
    for steps in [1.0, 2.5, -1.0, -3.0] {
        let before = cam(Projection::Perspective);
        let mut c = before;
        c.zoom_about(steps, pivot);
        let ratio = ZOOM_STEP_FACTOR.powf(steps);
        assert!(
            near(c.target, pivot + (before.target - pivot) * ratio, 1e-9),
            "{steps}"
        );
        assert!(
            near(c.eye(), pivot + (before.eye() - pivot) * ratio, 1e-9),
            "{steps}"
        );
    }
}

#[test]
fn a_near_surface_is_not_passed() {
    // Tyrel's case: the target is far behind a surface that is 0.5 m from the eye.
    let mut c = cam(Projection::Perspective);
    c.distance = 50.0;
    let ndc = DVec2::new(0.2, -0.1);
    let surface = point_at(&c, ndc, 0.5);
    let before = c.eye().distance(surface);
    c.zoom_about(1.0, surface);
    let after = c.eye().distance(surface);
    assert!(
        (after - before * ZOOM_STEP_FACTOR).abs() < 1e-9,
        "{before} -> {after}"
    );
    let (moved, depth) = c.project(surface, ASPECT).expect("surface still in front");
    assert!(depth > 0.0);
    assert!(moved.distance(ndc) < 1e-9, "{moved:?}");
}

#[test]
fn the_pivot_keeps_its_screen_position_and_depth_scales() {
    for projection in [Projection::Perspective, Projection::Parallel] {
        for (ndc, depth) in [
            (DVec2::ZERO, 10.0),
            (DVec2::new(0.7, 0.4), 3.0),
            (DVec2::new(-0.9, -0.8), 25.0),
        ] {
            let mut c = cam(projection);
            let pivot = point_at(&c, ndc, depth);
            let (ndc0, depth0) = c.project(pivot, ASPECT).unwrap();
            c.zoom_about(1.5, pivot);
            let (ndc1, depth1) = c.project(pivot, ASPECT).unwrap();
            assert!(
                ndc1.distance(ndc0) < 1e-9,
                "{projection:?} {ndc:?} {ndc1:?}"
            );
            let ratio = ZOOM_STEP_FACTOR.powf(1.5);
            assert!(
                (depth1 - depth0 * ratio).abs() < 1e-9,
                "{projection:?} {depth0} {depth1}"
            );
        }
    }
}

#[test]
fn a_pivot_on_the_target_plane_matches_plain_zoom() {
    for projection in [Projection::Perspective, Projection::Parallel] {
        for ndc in [DVec2::ZERO, DVec2::new(0.5, -0.3), DVec2::new(-1.0, 1.0)] {
            let mut a = cam(projection);
            let mut b = a;
            let pivot = a.target_plane_point(ndc, ASPECT);
            a.zoom(2.0, ndc, ASPECT);
            b.zoom_about(2.0, pivot);
            assert!(near(a.target, b.target, 1e-9), "{projection:?} {ndc:?}");
            assert!((a.distance - b.distance).abs() < 1e-12);
        }
    }
}

#[test]
fn zooming_about_the_target_keeps_the_target() {
    let mut c = cam(Projection::Perspective);
    let target = c.target;
    c.zoom_about(3.0, target);
    assert!(near(c.target, target, 1e-12));
    assert!((c.distance - 10.0 * ZOOM_STEP_FACTOR.powi(3)).abs() < 1e-12);
}

#[test]
fn in_then_out_returns_to_the_start() {
    let pivot = DVec3::new(-2.0, 4.0, 1.0);
    let before = cam(Projection::Perspective);
    let mut c = before;
    c.zoom_about(4.0, pivot);
    c.zoom_about(-4.0, pivot);
    assert!(near(c.target, before.target, 1e-9));
    assert!((c.distance - before.distance).abs() < 1e-9);
}

#[test]
fn distance_is_clamped_and_a_clamped_zoom_does_not_move_the_target() {
    let pivot = DVec3::new(3.0, -1.0, 0.0);
    let mut c = cam(Projection::Perspective);
    c.distance = MIN_DISTANCE;
    let target = c.target;
    c.zoom_about(5.0, pivot);
    assert_eq!(c.distance, MIN_DISTANCE);
    assert!(near(c.target, target, 1e-12), "{:?}", c.target);

    let mut c = cam(Projection::Perspective);
    c.zoom_about(-1000.0, pivot);
    assert_eq!(c.distance, MAX_DISTANCE);
    let ratio = MAX_DISTANCE / 10.0;
    let want = pivot + (cam(Projection::Perspective).target - pivot) * ratio;
    assert!(near(c.target, want, 1e-9 * MAX_DISTANCE), "{:?}", c.target);
}

#[test]
fn a_non_finite_pivot_or_step_leaves_the_camera_alone() {
    for (steps, pivot) in [
        (1.0, DVec3::new(f64::NAN, 0.0, 0.0)),
        (1.0, DVec3::new(0.0, f64::INFINITY, 0.0)),
        (f64::NAN, DVec3::ZERO),
        (f64::INFINITY, DVec3::ZERO),
    ] {
        let before = cam(Projection::Perspective);
        let mut c = before;
        c.zoom_about(steps, pivot);
        assert_eq!(c, before, "{steps} {pivot:?}");
    }
}

fn camera_strategy() -> impl Strategy<Value = Camera> {
    (
        -50.0..50.0f64,
        -50.0..50.0f64,
        -10.0..10.0f64,
        0.5..200.0f64,
        -3.1..3.1f64,
        -1.5..1.5f64,
        prop::bool::ANY,
    )
        .prop_map(|(x, y, z, distance, yaw, pitch, parallel)| Camera {
            target: DVec3::new(x, y, z),
            distance,
            yaw,
            pitch,
            fov_y: 35f64.to_radians(),
            projection: if parallel {
                Projection::Parallel
            } else {
                Projection::Perspective
            },
        })
}

proptest! {
    #[test]
    fn prop_the_pivot_stays_under_the_cursor(
        c in camera_strategy(),
        nx in -1.0..1.0f64,
        ny in -1.0..1.0f64,
        depth_frac in 0.01..3.0f64,
        steps in -4.0..4.0f64,
    ) {
        let ndc = DVec2::new(nx, ny);
        let pivot = point_at(&c, ndc, depth_frac * c.distance);
        let (_, depth0) = c.project(pivot, ASPECT).unwrap();
        let mut z = c;
        z.zoom_about(steps, pivot);
        let ratio = z.distance / c.distance;
        let (ndc1, depth1) = z.project(pivot, ASPECT).unwrap();
        prop_assert!(ndc1.distance(ndc) < 1e-6, "{:?} vs {:?}", ndc1, ndc);
        prop_assert!((depth1 - depth0 * ratio).abs() < 1e-6 * depth0.abs().max(1.0));
        prop_assert_eq!(z.yaw, c.yaw);
        prop_assert_eq!(z.pitch, c.pitch);
    }

    #[test]
    fn prop_zooming_in_never_passes_the_pivot(
        c in camera_strategy(),
        nx in -1.0..1.0f64,
        ny in -1.0..1.0f64,
        depth_frac in 0.001..2.0f64,
        steps in 1usize..60,
    ) {
        let c = Camera { projection: Projection::Perspective, ..c };
        let pivot = point_at(&c, DVec2::new(nx, ny), depth_frac * c.distance);
        let mut z = c;
        let mut last = z.eye().distance(pivot);
        for _ in 0..steps {
            z.zoom_about(1.0, pivot);
            let now = z.eye().distance(pivot);
            prop_assert!(now <= last, "{} -> {}", last, now);
            last = now;
            let (_, depth) = z.project(pivot, ASPECT).expect("pivot still in front");
            prop_assert!(depth > 0.0);
        }
    }
}
