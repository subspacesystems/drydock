// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Fit check for 3D printing: which triangles of two parts come closer to the other part
//! than the clearance, and a scene copy with those triangles tinted.
//!
//! Behaviour: `docs/specs/m7-clearance.md`.

use glam::{DMat4, DVec3};
use modeler_kernel::model as km;

use crate::FlatScene;

/// A triangle's corners, in meters.
pub type Triangle = [DVec3; 3];

/// The clearance the app starts with: 0.2 mm, in meters.
pub const DEFAULT_CLEARANCE_M: f64 = 0.0002;

/// Color of the material [`tint_fit`] adds.
pub const FIT_COLOR: [f32; 4] = [0.95, 0.75, 0.1, 1.0];

/// How close a scene triangle's corners must be to a flagged triangle's corners for
/// [`tint_fit`] to tint it, in meters.
pub const MATCH_TOLERANCE_M: f64 = 1e-5;

/// Which triangles of two parts are closer to the other part than the clearance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FitReport {
    /// Indices into the first list, ascending.
    pub a: Vec<usize>,
    /// Indices into the second list, ascending.
    pub b: Vec<usize>,
}

/// The shortest distance between the two closed triangles (their points, not just their
/// corners): 0 when they touch, share a point or cross.
pub fn triangle_distance(a: &Triangle, b: &Triangle) -> f64 {
    if triangles_pierce(a, b) {
        return 0.0;
    }
    let mut best = point_triangle(a[0], b).min(point_triangle(a[1], b));
    best = best.min(point_triangle(a[2], b));
    best = best.min(point_triangle(b[0], a));
    best = best.min(point_triangle(b[1], a));
    best = best.min(point_triangle(b[2], a));
    best = best.min(segment_segment(a[0], a[1], b[0], b[1]));
    best = best.min(segment_segment(a[0], a[1], b[0], b[2]));
    best = best.min(segment_segment(a[0], a[1], b[1], b[2]));
    best = best.min(segment_segment(a[0], a[2], b[0], b[1]));
    best = best.min(segment_segment(a[0], a[2], b[0], b[2]));
    best = best.min(segment_segment(a[0], a[2], b[1], b[2]));
    best = best.min(segment_segment(a[1], a[2], b[0], b[1]));
    best = best.min(segment_segment(a[1], a[2], b[0], b[2]));
    best = best.min(segment_segment(a[1], a[2], b[1], b[2]));
    best
}

/// True when the solids of the two triangles pierce: the plane of one strictly crosses
/// the solid of the other.
fn triangles_pierce(a: &Triangle, b: &Triangle) -> bool {
    let n = (a[1] - a[0]).cross(a[2] - a[0]);
    let [d1, d2, d3] = [
        (b[0] - a[0]).dot(n),
        (b[1] - a[0]).dot(n),
        (b[2] - a[0]).dot(n),
    ];
    let nb = (b[1] - b[0]).cross(b[2] - b[0]);
    let [e1, e2, e3] = [
        (a[0] - b[0]).dot(nb),
        (a[1] - b[0]).dot(nb),
        (a[2] - b[0]).dot(nb),
    ];
    !(d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0)
        && !(d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0)
        && !(e1 <= 0.0 && e2 <= 0.0 && e3 <= 0.0)
        && !(e1 >= 0.0 && e2 >= 0.0 && e3 >= 0.0)
}

/// The distance from a point `p` to the closed triangle `t`.
fn point_triangle(p: DVec3, t: &Triangle) -> f64 {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (u, v) = (b - a, c - a);
    let u2 = u.length_squared();
    let v2 = v.length_squared();
    let uv = u.dot(v);
    if u2 <= 1e-30 && v2 <= 1e-30 {
        return p.distance(a);
    }
    if u2 <= 1e-30 {
        return p.distance(b).min(point_segment(p, b, c));
    }
    if v2 <= 1e-30 {
        return p.distance(c).min(point_segment(p, a, b));
    }
    let up = u.dot(p - a);
    let vp = v.dot(p - a);
    let det = u2 * v2 - uv * uv;
    if det <= 1e-30 {
        return point_segment(p, a, b)
            .min(point_segment(p, a, c))
            .min(point_segment(p, b, c));
    }
    let s = (v2 * up - uv * vp) / det;
    let tw = (u2 * vp - uv * up) / det;
    if s >= 0.0 && tw >= 0.0 && s + tw <= 1.0 {
        return (p - (a + u * s + v * tw)).length();
    }
    let d_ab = (p - (a + u * s.clamp(0.0, 1.0))).length();
    let d_ac = (p - (a + v * tw.clamp(0.0, 1.0))).length();
    let d_bc = point_segment(p, b, c);
    let d_min_ab = if s < 0.0 {
        (p - a).length()
    } else if s > 1.0 {
        (p - b).length()
    } else {
        d_ab
    };
    let d_min_ac = if tw < 0.0 {
        (p - a).length()
    } else if tw > 1.0 {
        (p - c).length()
    } else {
        d_ac
    };
    d_min_ab.min(d_min_ac).min(d_bc)
}

/// The distance from a point `p` to the segment `a..b`.
fn point_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let v = b - a;
    let t = v.length_squared().max(1e-30);
    let s = ((p - a).dot(v) / t).clamp(0.0, 1.0);
    (p - (a + v * s)).length()
}

/// The distance between the two segments `pa..pb` and `qc..qd`, 0 when they cross or
/// share an end point.
fn segment_segment(pa: DVec3, pb: DVec3, qc: DVec3, qd: DVec3) -> f64 {
    if segments_cross(pa, pb, qc, qd) {
        return 0.0;
    }
    point_segment(pa, qc, qd)
        .min(point_segment(pb, qc, qd))
        .min(point_segment(qc, pa, pb))
        .min(point_segment(qd, pa, pb))
}

/// True when the two segments `a..b` and `c..d` cross or share an end point.
fn segments_cross(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> bool {
    if a.distance_squared(c) <= 1e-30
        || a.distance_squared(d) <= 1e-30
        || b.distance_squared(c) <= 1e-30
        || b.distance_squared(d) <= 1e-30
    {
        return true;
    }
    let e1 = b - a;
    let e2 = d - c;
    let n = e1.cross(e2);
    let n2 = n.length_squared();
    if n2 <= 1e-30 {
        return false;
    }
    let q = c - a;
    let t = q.dot(n) / n2;
    if t < 0.0 || t > 1.0 {
        return false;
    }
    let p = a + e1 * t;
    is_on_segment(p, c, d)
}

fn is_on_segment(p: DVec3, a: DVec3, b: DVec3) -> bool {
    let v = b - a;
    let t = v.length_squared().max(1e-30);
    let s = v.dot(p - a) / t;
    s >= 0.0 && s <= 1.0 && (p - (a + v * s)).length_squared() <= 1e-20
}

/// The triangles of `a` and of `b` that lie within `clearance` (exclusive) of some
/// triangle of the other list.
pub fn close_triangles(a: &[Triangle], b: &[Triangle], clearance: f64) -> FitReport {
    if !clearance.is_finite() || clearance < 0.0 {
        return FitReport::default();
    }
    let a_bboxes: Vec<[DVec3; 2]> = a
        .iter()
        .map(|t| {
            let mut min = t[0];
            let mut max = t[0];
            for p in &t[1..] {
                min = DVec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                max = DVec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
            }
            [min, max]
        })
        .collect();
    let b_bboxes: Vec<[DVec3; 2]> = b
        .iter()
        .map(|t| {
            let mut min = t[0];
            let mut max = t[0];
            for p in &t[1..] {
                min = DVec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                max = DVec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
            }
            [min, max]
        })
        .collect();

    let mut report = FitReport::default();
    for (i, ta) in a.iter().enumerate() {
        if ta.iter().any(|p| !p.is_finite()) {
            continue;
        }
        for (j, tb) in b.iter().enumerate() {
            if tb.iter().any(|p| !p.is_finite()) {
                continue;
            }
            if !aabb_close(a_bboxes[i], b_bboxes[j], clearance) {
                continue;
            }
            if triangle_distance(ta, tb) < clearance {
                report.a.push(i);
                break;
            }
        }
    }
    for (j, tb) in b.iter().enumerate() {
        if tb.iter().any(|p| !p.is_finite()) {
            continue;
        }
        for (i, ta) in a.iter().enumerate() {
            if ta.iter().any(|p| !p.is_finite()) {
                continue;
            }
            if !aabb_close(a_bboxes[i], b_bboxes[j], clearance) {
                continue;
            }
            if triangle_distance(ta, tb) < clearance {
                report.b.push(j);
                break;
            }
        }
    }
    report
}

/// True when the two axis-aligned bounding boxes are within `d` of each other.
fn aabb_close(a: [DVec3; 2], b: [DVec3; 2], d: f64) -> bool {
    let axes = [
        (a[0].x, a[1].x, b[0].x, b[1].x),
        (a[0].y, a[1].y, b[0].y, b[1].y),
        (a[0].z, a[1].z, b[0].z, b[1].z),
    ];
    for (a_min, a_max, b_min, b_max) in axes {
        if a_max < b_min - d || b_max < a_min - d {
            return false;
        }
    }
    true
}

/// Every triangle of the faces of `instance` (an instance in `body`) and of the instances
/// nested in it, placed by the instance's transform and then by `outer`: the triangles
/// `kernel_scene` draws for it.
pub fn instance_triangles(
    _model: &km::Model,
    _body: km::BodyId,
    _instance: km::InstanceId,
    _outer: DMat4,
) -> Vec<Triangle> {
    unimplemented!("m7-clearance")
}

/// A copy of `scene` with every triangle that matches one of `flagged` moved into one batch
/// drawn in [`FIT_COLOR`].
pub fn tint_fit(_scene: &FlatScene, _flagged: &[Triangle]) -> FlatScene {
    unimplemented!("m7-clearance")
}
