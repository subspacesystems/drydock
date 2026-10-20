// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Points of the shapes the drawing tools make: rectangle, circle, arc. Pure functions; the
//! tools turn the points into edges.
//!
//! Behaviour: `docs/specs/m2-shapes.md`. All lengths are meters, angles radians.

use crate::geom::LINEAR_TOL;
use glam::DVec3;
use std::f64::consts::TAU;

/// The four corners of the rectangle that has `a` and the point under `b` as opposite corners,
/// with sides along the unit, perpendicular directions `u` and `v`.
pub fn rectangle(a: DVec3, b: DVec3, u: DVec3, v: DVec3) -> Option<[DVec3; 4]> {
    let du = (b - a).dot(u);
    let dv = (b - a).dot(v);
    if !du.is_finite() || !dv.is_finite() || du.abs() <= LINEAR_TOL || dv.abs() <= LINEAR_TOL {
        return None;
    }
    Some([a, a + du * u, a + du * u + dv * v, a + dv * v])
}

/// `circle` and `arc` share a frame: the unit `normal`, a unit `x` left over from removing
/// `x_dir`'s component along it, and `y = n × x`. `None` when the inputs make no such frame
/// (zero or non-finite directions, or `x_dir` parallel to `normal`).
fn frame(normal: DVec3, x_dir: DVec3) -> Option<(DVec3, DVec3, DVec3)> {
    if !normal.is_finite() || normal.length() <= 0.0 {
        return None;
    }
    let n = normal / normal.length();
    let x = x_dir - n * x_dir.dot(n);
    if !x_dir.is_finite() || x.length() <= 1e-9 {
        return None;
    }
    let x = x / x.length();
    if !x.is_finite() {
        return None;
    }
    Some((n, x, n.cross(x)))
}

fn radius_ok(radius: f64) -> bool {
    radius.is_finite() && radius > LINEAR_TOL
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
    if segments < 3 || !radius_ok(radius) {
        return Vec::new();
    }
    let (_, x, y) = match frame(normal, x_dir) {
        Some(f) => f,
        None => return Vec::new(),
    };
    (0..segments)
        .map(|k| {
            let t = TAU * k as f64 / segments as f64;
            center + (x * t.cos() + y * t.sin()) * radius
        })
        .collect()
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
    if segments == 0
        || !start.is_finite()
        || !sweep.is_finite()
        || sweep.abs() < 1e-9
        || sweep.abs() > TAU + 1e-9
        || !radius_ok(radius)
    {
        return Vec::new();
    }
    let (_, x, y) = match frame(normal, x_dir) {
        Some(f) => f,
        None => return Vec::new(),
    };
    (0..=segments)
        .map(|k| {
            let t = start + sweep * k as f64 / segments as f64;
            center + (x * t.cos() + y * t.sin()) * radius
        })
        .collect()
}

/// The counter-clockwise angle about `normal`, in `[0, 2π)`, from the direction of `from` to
/// the direction of `to`, both seen from `center`.
pub fn sweep_angle(center: DVec3, normal: DVec3, from: DVec3, to: DVec3) -> Option<f64> {
    if !normal.is_finite() || normal.length() <= 0.0 {
        return None;
    }
    let n = normal / normal.length();
    // an in-plane unit `x`: `X` unless nearly parallel to the normal, then `Y`
    let (_, x, y) = frame(
        n,
        if n.dot(DVec3::X).abs() > 1e-6 {
            DVec3::X
        } else {
            DVec3::Y
        },
    )?;
    let project = |p: DVec3| -> Option<DVec3> {
        let d = p - center;
        if !d.is_finite() {
            return None;
        }
        let q = d - n * d.dot(n);
        (q.is_finite() && q.length() > LINEAR_TOL).then_some(q)
    };
    let f = project(from)?;
    let t = project(to)?;
    let a_to = t.dot(y).atan2(t.dot(x));
    let a_from = f.dot(y).atan2(f.dot(x));
    let mut result = (a_to - a_from).rem_euclid(TAU);
    if result > TAU - 1e-9 {
        result = 0.0;
    }
    Some(result)
}
