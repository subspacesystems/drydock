// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-orbit-ground: the ground point under the cursor as an orbit pivot.
//! See docs/specs/m4-orbit-ground.md.

use glam::DVec3;
use modeler_view::{Camera, GROUND_PIVOT_RANGE, Projection};
use proptest::prelude::*;

fn cam(target: DVec3, distance: f64) -> Camera {
    Camera {
        target,
        distance,
        yaw: 0.3,
        pitch: 0.4,
        fov_y: 35f64.to_radians(),
        projection: Projection::Perspective,
    }
}

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn straight_down_hits_below_the_origin() {
    let c = cam(DVec3::ZERO, 10.0);
    let p = c
        .ground_pivot(DVec3::new(3.0, 4.0, 10.0), DVec3::NEG_Z)
        .expect("hit");
    assert!(near(p, DVec3::new(3.0, 4.0, 0.0)), "{p}");
}

#[test]
fn oblique_ray_hits_where_it_crosses_z_zero() {
    let c = cam(DVec3::ZERO, 10.0);
    let p = c
        .ground_pivot(
            DVec3::new(0.0, 0.0, 10.0),
            DVec3::new(1.0, 0.0, -1.0).normalize(),
        )
        .expect("hit");
    assert!(near(p, DVec3::new(10.0, 0.0, 0.0)), "{p}");
}

#[test]
fn direction_length_does_not_matter() {
    let c = cam(DVec3::ZERO, 10.0);
    let o = DVec3::new(-2.0, 5.0, 7.0);
    let d = DVec3::new(0.3, -0.2, -0.9);
    let a = c.ground_pivot(o, d).expect("hit");
    let b = c.ground_pivot(o, d * 37.5).expect("hit");
    let u = c.ground_pivot(o, d.normalize()).expect("hit");
    assert!(near(a, b) && near(a, u), "{a} {b} {u}");
}

#[test]
fn result_z_is_exactly_zero() {
    let c = cam(DVec3::ZERO, 10.0);
    let p = c
        .ground_pivot(
            DVec3::new(0.1, 0.2, 3.3),
            DVec3::new(0.7, 0.11, -0.3).normalize(),
        )
        .expect("hit");
    assert_eq!(p.z, 0.0, "{p}");
}

#[test]
fn ray_pointing_up_from_above_misses() {
    let c = cam(DVec3::ZERO, 10.0);
    assert_eq!(
        c.ground_pivot(DVec3::new(0.0, 0.0, 5.0), DVec3::new(0.2, 0.0, 1.0)),
        None
    );
}

#[test]
fn level_ray_misses() {
    let c = cam(DVec3::ZERO, 10.0);
    assert_eq!(c.ground_pivot(DVec3::new(0.0, 0.0, 5.0), DVec3::X), None);
    assert_eq!(
        c.ground_pivot(DVec3::new(0.0, 0.0, 5.0), DVec3::new(1.0, 1.0, 1e-14)),
        None
    );
}

#[test]
fn ray_from_below_going_up_hits() {
    let c = cam(DVec3::ZERO, 10.0);
    let p = c
        .ground_pivot(DVec3::new(1.0, 1.0, -5.0), DVec3::Z)
        .expect("hit");
    assert!(near(p, DVec3::new(1.0, 1.0, 0.0)), "{p}");
}

#[test]
fn origin_on_the_ground_misses() {
    let c = cam(DVec3::ZERO, 10.0);
    assert_eq!(
        c.ground_pivot(DVec3::new(1.0, 2.0, 0.0), DVec3::NEG_Z),
        None
    );
}

#[test]
fn bad_input_misses() {
    let c = cam(DVec3::ZERO, 10.0);
    let o = DVec3::new(0.0, 0.0, 5.0);
    assert_eq!(c.ground_pivot(o, DVec3::ZERO), None);
    assert_eq!(c.ground_pivot(o, DVec3::new(f64::NAN, 0.0, -1.0)), None);
    assert_eq!(
        c.ground_pivot(o, DVec3::new(0.0, 0.0, f64::NEG_INFINITY)),
        None
    );
    assert_eq!(
        c.ground_pivot(DVec3::new(f64::INFINITY, 0.0, 5.0), DVec3::NEG_Z),
        None
    );
    let bad_target = cam(DVec3::new(f64::NAN, 0.0, 0.0), 10.0);
    assert_eq!(bad_target.ground_pivot(o, DVec3::NEG_Z), None);
    let bad_distance = cam(DVec3::ZERO, f64::INFINITY);
    assert_eq!(bad_distance.ground_pivot(o, DVec3::NEG_Z), None);
}

#[test]
fn far_crossing_near_the_horizon_misses() {
    // range = 50 * 1 m from the target at the origin.
    let c = cam(DVec3::ZERO, 1.0);
    let o = DVec3::new(0.0, 0.0, 1.0);
    assert_eq!(c.ground_pivot(o, DVec3::new(1000.0, 0.0, -1.0)), None);
    assert_eq!(c.ground_pivot(o, DVec3::new(50.5, 0.0, -1.0)), None);
    let p = c
        .ground_pivot(o, DVec3::new(49.0, 0.0, -1.0))
        .expect("within range");
    assert!(near(p, DVec3::new(49.0, 0.0, 0.0)), "{p}");
    let x = GROUND_PIVOT_RANGE - 1e-6;
    let edge = c
        .ground_pivot(DVec3::new(0.0, 0.0, 2.0), DVec3::new(x, 0.0, -2.0))
        .expect("just inside the range");
    assert!(near(edge, DVec3::new(x, 0.0, 0.0)), "{edge}");
}

#[test]
fn range_is_measured_from_the_target_not_the_origin() {
    // A parallel-projection ray starts far behind the camera; the ground point is close to
    // the target, so it counts.
    let c = Camera {
        projection: Projection::Parallel,
        ..cam(DVec3::new(100.0, 0.0, 0.0), 2.0)
    };
    let o = DVec3::new(100.0, -5000.0, 5000.0);
    let p = c
        .ground_pivot(o, DVec3::new(0.0, 1.0, -1.0))
        .expect("hit near the target");
    assert!(near(p, DVec3::new(100.0, 0.0, 0.0)), "{p}");

    // Close to the origin but far from the target: no.
    let far_target = cam(DVec3::new(1000.0, 0.0, 0.0), 2.0);
    assert_eq!(
        far_target.ground_pivot(DVec3::new(0.0, 0.0, 1.0), DVec3::NEG_Z),
        None
    );
}

#[test]
fn camera_center_ray_onto_a_ground_target_gives_the_target() {
    let target = DVec3::new(4.0, -3.0, 0.0);
    let c = cam(target, 12.0);
    let eye = c.eye();
    let p = c.ground_pivot(eye, target - eye).expect("hit");
    assert!(near(p, target), "{p} vs {target}");
}

proptest! {
    #[test]
    fn hits_lie_on_the_ray_in_front_and_on_the_ground(
        ox in -100.0..100.0f64,
        oy in -100.0..100.0f64,
        oz in 0.01..100.0f64,
        dx in -1.0..1.0f64,
        dy in -1.0..1.0f64,
        dz in -1.0..-0.05f64,
        scale in 0.01..100.0f64,
    ) {
        let o = DVec3::new(ox, oy, oz);
        let d = DVec3::new(dx, dy, dz) * scale;
        let t = -oz / d.z;
        let expected = o + d * t;
        // Put the target on the expected point so the range never rejects it.
        let c = cam(DVec3::new(expected.x, expected.y, 0.0), 1.0);
        let p = c.ground_pivot(o, d).expect("hit");
        prop_assert_eq!(p.z, 0.0);
        let along = (p - o).dot(d.normalize());
        prop_assert!(along > 0.0);
        let off = (p - o) - d.normalize() * along;
        prop_assert!(off.length() < 1e-9 * (1.0 + (p - o).length()), "{} off the ray", off);
        prop_assert!((p - expected).length() < 1e-9 * (1.0 + expected.length()));
    }
}
