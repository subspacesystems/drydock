// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-orbit-pivot: orbiting about the point under the cursor. See docs/specs/m4-orbit-pivot.md.

use std::f64::consts::{FRAC_PI_2, PI};

use glam::DVec3;
use modeler_view::{Camera, ORBIT_DEGREES_PER_PIXEL, Projection, StandardView};
use proptest::prelude::*;

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

/// The pivot's screen position (NDC) and its depth before and after.
fn screen(c: &Camera, p: DVec3) -> (glam::DVec2, f64) {
    c.project(p, 1.5).expect("pivot in front of the camera")
}

#[test]
fn yaw_and_pitch_change_as_in_plain_orbit() {
    let pivot = DVec3::new(3.0, -1.0, 0.0);
    for (dx, dy) in [(10.0, 0.0), (0.0, 12.0), (-25.0, 7.0), (40.0, -30.0)] {
        let mut a = cam(Projection::Perspective);
        let mut b = a;
        a.orbit(dx, dy);
        b.orbit_about(pivot, dx, dy);
        assert!((a.yaw - b.yaw).abs() < 1e-12, "{dx} {dy}");
        assert!((a.pitch - b.pitch).abs() < 1e-12, "{dx} {dy}");
        assert_eq!(a.distance, b.distance);
        assert_eq!(a.fov_y, b.fov_y);
        assert_eq!(a.projection, b.projection);
    }
}

#[test]
fn orbiting_about_the_target_is_plain_orbit() {
    let mut a = cam(Projection::Perspective);
    let mut b = a;
    a.orbit(33.0, -12.0);
    b.orbit_about(b.target, 33.0, -12.0);
    assert!(near(a.target, b.target, 1e-9), "{} {}", a.target, b.target);
    assert!(near(a.eye(), b.eye(), 1e-9));
}

#[test]
fn a_horizontal_drag_turns_about_the_vertical_through_the_pivot() {
    let pivot = DVec3::new(5.0, 0.0, 0.0);
    let mut c = cam(Projection::Perspective);
    let eye = c.eye();
    // 180 degrees of yaw: the eye ends up mirrored through the pivot's vertical axis.
    let pixels = 180.0 / ORBIT_DEGREES_PER_PIXEL;
    c.orbit_about(pivot, pixels, 0.0);
    let want = DVec3::new(2.0 * pivot.x - eye.x, 2.0 * pivot.y - eye.y, eye.z);
    assert!(near(c.eye(), want, 1e-9), "eye {} want {want}", c.eye());
}

#[test]
fn the_pivot_keeps_its_screen_position_in_both_projections() {
    for projection in [Projection::Perspective, Projection::Parallel] {
        let pivot = DVec3::new(2.0, 3.0, 1.0);
        let mut c = cam(projection);
        let (before, depth_before) = screen(&c, pivot);
        c.orbit_about(pivot, 30.0, 20.0);
        let (after, depth_after) = screen(&c, pivot);
        assert!(
            (before - after).length() < 1e-9,
            "{projection:?}: {before} -> {after}"
        );
        assert!((depth_before - depth_after).abs() < 1e-9);
    }
}

#[test]
fn pitch_stops_at_straight_down_and_the_pivot_stays_put() {
    let pivot = DVec3::new(-2.0, 1.0, 0.0);
    let mut c = cam(Projection::Perspective);
    let (before, _) = screen(&c, pivot);
    // Far more than enough to pass the top.
    c.orbit_about(pivot, 0.0, 1000.0);
    assert!((c.pitch - FRAC_PI_2).abs() < 1e-12, "pitch {}", c.pitch);
    let (after, _) = screen(&c, pivot);
    assert!((before - after).length() < 1e-9, "{before} -> {after}");
    // Further drags the same way change nothing.
    let stuck = c;
    c.orbit_about(pivot, 0.0, 50.0);
    assert!(near(c.eye(), stuck.eye(), 1e-9));
    assert!(near(c.target, stuck.target, 1e-9));
}

#[test]
fn a_zero_drag_changes_nothing() {
    let mut c = cam(Projection::Perspective);
    let before = c;
    c.orbit_about(DVec3::new(9.0, 9.0, 9.0), 0.0, 0.0);
    assert!(near(c.target, before.target, 1e-12));
    assert_eq!(c.yaw, before.yaw);
    assert_eq!(c.pitch, before.pitch);
}

#[test]
fn a_non_finite_pivot_orbits_about_the_target() {
    for pivot in [
        DVec3::new(f64::NAN, 0.0, 0.0),
        DVec3::new(0.0, f64::INFINITY, 0.0),
    ] {
        let mut a = cam(Projection::Perspective);
        let mut b = a;
        a.orbit(15.0, 5.0);
        b.orbit_about(pivot, 15.0, 5.0);
        assert_eq!(a, b);
    }
}

fn camera() -> impl Strategy<Value = Camera> {
    (
        (-50.0..50.0f64, -50.0..50.0f64, -50.0..50.0f64),
        0.5..200.0f64,
        -2.0 * PI..2.0 * PI,
        -1.5..1.5f64,
        20.0..70.0f64,
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

proptest! {
    /// A pivot somewhere on screen in front of the camera: its screen position and
    /// distance to the eye survive any drag, and the camera stays a valid orbit camera.
    #[test]
    fn the_pivot_is_fixed_on_screen(
        c in camera(),
        sx in -0.8..0.8f64,
        sy in -0.8..0.8f64,
        depth in 0.2..3.0f64,
        dx in -400.0..400.0f64,
        dy in -400.0..400.0f64,
    ) {
        let aspect = 1.5;
        // A point that shows at (sx, sy), `depth` times the target distance away.
        let (forward, _, _) = c.basis();
        let on_plane = c.target_plane_point(glam::DVec2::new(sx, sy), aspect);
        let pivot = match c.projection {
            Projection::Perspective => c.eye() + (on_plane - c.eye()) * depth,
            Projection::Parallel => on_plane + forward * (depth - 1.0) * c.distance,
        };
        let (before, d0) = c.project(pivot, aspect).unwrap();
        let eye_to_pivot = (pivot - c.eye()).length();
        let mut after = c;
        after.orbit_about(pivot, dx, dy);
        let (now, d1) = after.project(pivot, aspect).unwrap();
        let scale = c.distance.max(1.0);
        prop_assert!((before - now).length() < 1e-7, "{before} -> {now}");
        prop_assert!((d0 - d1).abs() < 1e-7 * scale, "depth {d0} -> {d1}");
        prop_assert!(((pivot - after.eye()).length() - eye_to_pivot).abs() < 1e-7 * scale);
        prop_assert_eq!(after.distance, c.distance);
        prop_assert!(after.pitch.abs() <= FRAC_PI_2);
        prop_assert!(after.target.is_finite());
    }
}
