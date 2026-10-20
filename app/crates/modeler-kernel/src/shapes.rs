// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Points of the shapes the drawing tools make: rectangle, circle, arc. Pure functions; the
//! tools turn the points into edges.
//!
//! Behaviour: `docs/specs/m2-shapes.md`. All lengths are meters, angles radians.

use glam::DVec3;

/// The four corners of the rectangle that has `a` and the point under `b` as opposite corners,
/// with sides along the unit, perpendicular directions `u` and `v`.
pub fn rectangle(a: DVec3, b: DVec3, u: DVec3, v: DVec3) -> Option<[DVec3; 4]> {
    let _ = (a, b, u, v);
    unimplemented!("m2-shapes")
}

/// `segments` points of a regular polygon inscribed in the circle, counter-clockwise about
/// `normal`, the first in direction `x_dir` (made perpendicular to `normal`).
pub fn circle(
    center: DVec3,
    normal: DVec3,
    x_dir: DVec3,
    radius: f64,
    segments: usize,
) -> Vec<DVec3> {
    let _ = (center, normal, x_dir, radius, segments);
    unimplemented!("m2-shapes")
}

/// `segments + 1` points along an arc, from angle `start` (measured from `x_dir`) through
/// `sweep` (positive is counter-clockwise about `normal`).
pub fn arc(
    center: DVec3,
    normal: DVec3,
    x_dir: DVec3,
    radius: f64,
    start: f64,
    sweep: f64,
    segments: usize,
) -> Vec<DVec3> {
    let _ = (center, normal, x_dir, radius, start, sweep, segments);
    unimplemented!("m2-shapes")
}

/// The counter-clockwise angle about `normal`, in `[0, 2π)`, from the direction of `from` to
/// the direction of `to`, both seen from `center`.
pub fn sweep_angle(center: DVec3, normal: DVec3, from: DVec3, to: DVec3) -> Option<f64> {
    let _ = (center, normal, from, to);
    unimplemented!("m2-shapes")
}
