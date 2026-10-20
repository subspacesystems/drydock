// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Shape points (`docs/specs/m2-shapes.md`).

use glam::DVec3;
use modeler_kernel::shapes::{arc, circle, rectangle, sweep_angle};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const X: DVec3 = DVec3::X;
const Y: DVec3 = DVec3::Y;
const Z: DVec3 = DVec3::Z;

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

fn assert_pts(got: &[DVec3], want: &[DVec3]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(near(*g, *w), "got {got:?}, want {want:?}");
    }
}

#[test]
fn rectangle_basic() {
    let r = rectangle(DVec3::new(1.0, 1.0, 0.0), DVec3::new(4.0, 3.0, 0.0), X, Y).unwrap();
    assert_pts(
        &r,
        &[
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(4.0, 1.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(1.0, 3.0, 0.0),
        ],
    );
}

#[test]
fn rectangle_drops_the_part_of_b_off_the_plane() {
    let r = rectangle(DVec3::ZERO, DVec3::new(2.0, 3.0, 7.0), X, Y).unwrap();
    assert_pts(
        &r,
        &[
            DVec3::ZERO,
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 3.0, 0.0),
            DVec3::new(0.0, 3.0, 0.0),
        ],
    );
}

#[test]
fn rectangle_in_a_tilted_plane() {
    let u = DVec3::new(0.0, 1.0, 0.0);
    let v = DVec3::new(0.0, 0.0, 1.0);
    let a = DVec3::new(5.0, 0.0, 0.0);
    let r = rectangle(a, DVec3::new(5.0, 2.0, 4.0), u, v).unwrap();
    assert_pts(
        &r,
        &[
            a,
            DVec3::new(5.0, 2.0, 0.0),
            DVec3::new(5.0, 2.0, 4.0),
            DVec3::new(5.0, 0.0, 4.0),
        ],
    );
}

#[test]
fn rectangle_negative_direction_keeps_corner_order() {
    // b is left of and below a: corners still go a, along u, then v.
    let r = rectangle(DVec3::new(2.0, 2.0, 0.0), DVec3::new(0.0, 1.0, 0.0), X, Y).unwrap();
    assert_pts(
        &r,
        &[
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
            DVec3::new(2.0, 1.0, 0.0),
        ],
    );
}

#[test]
fn rectangle_refuses_flat_ones() {
    assert!(rectangle(DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0), X, Y).is_none());
    assert!(rectangle(DVec3::ZERO, DVec3::new(0.0, 3.0, 5.0), X, Y).is_none());
    assert!(rectangle(DVec3::ZERO, DVec3::ZERO, X, Y).is_none());
    assert!(rectangle(DVec3::ZERO, DVec3::new(1e-6, 4.0, 0.0), X, Y).is_none());
    assert!(rectangle(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), X, Y).is_some());
}

#[test]
fn rectangle_refuses_non_finite() {
    assert!(rectangle(DVec3::ZERO, DVec3::new(f64::NAN, 1.0, 0.0), X, Y).is_none());
    assert!(rectangle(DVec3::ZERO, DVec3::new(1.0, f64::INFINITY, 0.0), X, Y).is_none());
}

#[test]
fn circle_four_segments() {
    let c = circle(DVec3::new(1.0, 2.0, 3.0), Z, X, 2.0, 4);
    assert_pts(
        &c,
        &[
            DVec3::new(3.0, 2.0, 3.0),
            DVec3::new(1.0, 4.0, 3.0),
            DVec3::new(-1.0, 2.0, 3.0),
            DVec3::new(1.0, 0.0, 3.0),
        ],
    );
}

#[test]
fn circle_first_point_follows_x_dir() {
    let c = circle(DVec3::ZERO, Z, Y, 1.0, 8);
    assert_eq!(c.len(), 8);
    assert!(near(c[0], Y));
    // counter-clockwise about +Z: from +Y the next quarter is toward -X
    assert!(near(c[2], -X));
}

#[test]
fn circle_x_dir_is_made_perpendicular() {
    // x_dir leans up out of the plane and has a length other than 1
    let c = circle(DVec3::ZERO, Z, DVec3::new(3.0, 0.0, 5.0), 2.0, 4);
    assert_pts(
        &c,
        &[
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
            DVec3::new(-2.0, 0.0, 0.0),
            DVec3::new(0.0, -2.0, 0.0),
        ],
    );
}

#[test]
fn circle_normal_is_normalised_and_flips_direction() {
    let up = circle(DVec3::ZERO, DVec3::new(0.0, 0.0, 9.0), X, 1.0, 4);
    assert!(near(up[1], Y));
    let down = circle(DVec3::ZERO, -Z, X, 1.0, 4);
    assert!(near(down[1], -Y));
}

#[test]
fn circle_in_a_vertical_plane() {
    // normal +Y: x = +X, y = n x x = Y x X = -Z
    let c = circle(DVec3::ZERO, Y, X, 1.0, 4);
    assert_pts(&c, &[X, -Z, -X, Z]);
}

#[test]
fn circle_degenerate_input_is_empty() {
    assert!(circle(DVec3::ZERO, Z, X, 1.0, 2).is_empty());
    assert!(circle(DVec3::ZERO, Z, X, 1.0, 0).is_empty());
    assert!(circle(DVec3::ZERO, Z, X, 0.0, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, X, 1e-6, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, X, -1.0, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, X, f64::NAN, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, X, f64::INFINITY, 12).is_empty());
    assert!(circle(DVec3::ZERO, DVec3::ZERO, X, 1.0, 12).is_empty());
    assert!(circle(DVec3::ZERO, DVec3::new(f64::NAN, 0.0, 1.0), X, 1.0, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, DVec3::ZERO, 1.0, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, Z, 1.0, 12).is_empty());
    assert!(circle(DVec3::ZERO, Z, DVec3::new(0.0, 0.0, -4.0), 1.0, 12).is_empty());
    assert_eq!(circle(DVec3::ZERO, Z, X, 1.0, 3).len(), 3);
}

#[test]
fn arc_quarter() {
    let a = arc(DVec3::ZERO, Z, X, 1.0, 0.0, FRAC_PI_2, 2);
    assert_eq!(a.len(), 3);
    assert!(near(a[0], X));
    assert!(near(a[1], DVec3::new(0.5f64.sqrt(), 0.5f64.sqrt(), 0.0)));
    assert!(near(a[2], Y));
}

#[test]
fn arc_start_angle_and_negative_sweep() {
    let a = arc(
        DVec3::new(0.0, 0.0, 1.0),
        Z,
        X,
        2.0,
        FRAC_PI_2,
        -FRAC_PI_2,
        1,
    );
    assert_pts(&a, &[DVec3::new(0.0, 2.0, 1.0), DVec3::new(2.0, 0.0, 1.0)]);
}

#[test]
fn arc_full_turn_closes_on_itself() {
    let a = arc(DVec3::ZERO, Z, X, 1.0, 0.0, TAU, 6);
    assert_eq!(a.len(), 7);
    assert!(near(a[0], a[6]));
    let c = circle(DVec3::ZERO, Z, X, 1.0, 6);
    assert_pts(&a[..6], &c);
    // a sweep a hair over a full turn is still accepted
    assert_eq!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, -TAU - 1e-10, 3).len(), 4);
}

#[test]
fn arc_degenerate_input_is_empty() {
    assert!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, 1.0, 0).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, 0.0, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, 1e-10, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, TAU + 0.01, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, -7.0, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, f64::NAN, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 1.0, f64::INFINITY, 1.0, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, X, 0.0, 0.0, 1.0, 4).is_empty());
    assert!(arc(DVec3::ZERO, Z, Z, 1.0, 0.0, 1.0, 4).is_empty());
    assert!(arc(DVec3::ZERO, DVec3::ZERO, X, 1.0, 0.0, 1.0, 4).is_empty());
    assert_eq!(arc(DVec3::ZERO, Z, X, 1.0, 0.0, 1.0, 1).len(), 2);
}

fn sweep(from: DVec3, to: DVec3) -> Option<f64> {
    sweep_angle(DVec3::ZERO, Z, from, to)
}

#[test]
fn sweep_angle_quarter_turns() {
    assert!((sweep(X, Y).unwrap() - FRAC_PI_2).abs() < 1e-9);
    assert!((sweep(Y, X).unwrap() - 3.0 * FRAC_PI_2).abs() < 1e-9);
    assert!((sweep(X, -X).unwrap() - PI).abs() < 1e-9);
}

#[test]
fn sweep_angle_ignores_length_and_height_and_center() {
    let c = DVec3::new(10.0, 10.0, 10.0);
    let got = sweep_angle(c, Z, c + X * 4.0 + Z * 3.0, c + Y * 0.5 - Z * 8.0).unwrap();
    assert!((got - FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn sweep_angle_follows_the_normal() {
    let got = sweep_angle(DVec3::ZERO, -Z, X, Y).unwrap();
    assert!((got - 3.0 * FRAC_PI_2).abs() < 1e-9);
    let got = sweep_angle(DVec3::ZERO, DVec3::new(0.0, 0.0, 5.0), X, Y).unwrap();
    assert!((got - FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn sweep_angle_equal_directions_is_zero() {
    assert_eq!(sweep(X, X), Some(0.0));
    assert_eq!(sweep(X, X * 3.0), Some(0.0));
    // a direction a hair clockwise of `from` must not come back as just under a full turn
    let to = DVec3::new(1.0, -1e-12, 0.0);
    assert_eq!(sweep(X, to), Some(0.0));
}

#[test]
fn sweep_angle_refuses_degenerate_input() {
    assert!(sweep(DVec3::ZERO, X).is_none());
    assert!(sweep(X, DVec3::ZERO).is_none());
    assert!(sweep(X, Z * 4.0).is_none()); // straight along the normal
    assert!(sweep(X, DVec3::new(1e-6, 0.0, 3.0)).is_none());
    assert!(sweep_angle(DVec3::ZERO, DVec3::ZERO, X, Y).is_none());
    assert!(sweep_angle(DVec3::ZERO, DVec3::new(f64::NAN, 0.0, 1.0), X, Y).is_none());
}
