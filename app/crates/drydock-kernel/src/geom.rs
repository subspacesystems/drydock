// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tolerance-aware geometry helpers: planes, segments, polygons.
//!
//! Behaviour: `docs/specs/m1-geom.md`. All lengths are meters. Functions never panic on
//! degenerate input; they return the "nothing" value documented on each.

use glam::{DVec2, DVec3};

/// Two points closer than this are the same point.
pub const LINEAR_TOL: f64 = 1e-5;

/// A plane through `point` with unit `normal`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub point: DVec3,
    pub normal: DVec3,
}

impl Plane {
    /// Signed distance from the plane; positive on the side the normal points to.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.normal.dot(p - self.point)
    }
}

/// Where two segments meet. Parameters are 0..=1 along each segment (0 = first endpoint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegmentHit {
    None,
    /// A single shared point (crossing, T-junction or touching endpoints).
    Point {
        ta: f64,
        tb: f64,
    },
    /// Collinear overlap longer than the tolerance. `a_from < a_to`; `b_from` is the
    /// parameter on `b` of the point at `a_from` (so `b_from` may be larger than `b_to`).
    Overlap {
        a_from: f64,
        a_to: f64,
        b_from: f64,
        b_to: f64,
    },
}

/// Where a point lies relative to a polygon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Containment {
    Inside,
    /// Within the tolerance of an edge or vertex.
    Boundary,
    Outside,
}

/// Half the sum of cross products around the polygon: its area times its normal
/// (counter-clockwise seen from the normal side). Zero for fewer than 3 points.
pub fn area_vector(pts: &[DVec3]) -> DVec3 {
    if pts.len() < 3 {
        return DVec3::ZERO;
    }
    let origin = pts[0];
    let mut sum = DVec3::ZERO;
    for i in 1..pts.len() - 1 {
        sum += (pts[i] - origin).cross(pts[i + 1] - origin);
    }
    sum * 0.5
}

/// Unit normal of the polygon (right-hand rule along the vertex order), or `DVec3::ZERO`
/// when the polygon has fewer than 3 points or its area is below 1e-12 m².
pub fn polygon_normal(pts: &[DVec3]) -> DVec3 {
    let area = area_vector(pts);
    let len = area.length();
    if len < 1e-12 { DVec3::ZERO } else { area / len }
}

/// Best-fit plane: normal from `polygon_normal`, point at the vertex average. `None` when
/// the normal is zero.
pub fn fit_plane(pts: &[DVec3]) -> Option<Plane> {
    let normal = polygon_normal(pts);
    if normal == DVec3::ZERO {
        return None;
    }
    let mut sum = DVec3::ZERO;
    for p in pts {
        sum += *p;
    }
    let point = sum / pts.len() as f64;
    Some(Plane { point, normal })
}

/// Largest distance of any point from the plane.
pub fn max_plane_deviation(pts: &[DVec3], plane: &Plane) -> f64 {
    let mut max = 0.0f64;
    for p in pts {
        let d = plane.signed_distance(*p).abs();
        if d > max {
            max = d;
        }
    }
    max
}

/// True when the points have a fit plane and all lie within `tol` of it.
pub fn is_planar(pts: &[DVec3], tol: f64) -> bool {
    let Some(plane) = fit_plane(pts) else {
        return false;
    };
    max_plane_deviation(pts, &plane) <= tol
}

/// Distance from `p` to the segment `a`-`b` (a degenerate segment is a point).
pub fn point_segment_distance(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn snap_param(t: f64, len: f64, tol: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let d0 = t * len;
    let d1 = (1.0 - t) * len;
    if d0 <= tol && d0 <= d1 {
        0.0
    } else if d1 <= tol {
        1.0
    } else {
        t
    }
}

fn point_line_distance(p: DVec3, a: DVec3, u: DVec3, len: f64) -> f64 {
    (p - a).cross(u).length() / len
}

/// Closest parameters on the two segments, and their squared distance.
fn closest_segment_params(a0: DVec3, a1: DVec3, b0: DVec3, b1: DVec3) -> (f64, f64, f64) {
    let u = a1 - a0;
    let v = b1 - b0;
    let w = a0 - b0;
    let a = u.dot(u);
    let c = v.dot(v);
    let b = u.dot(v);
    let d = u.dot(w);
    let e = v.dot(w);

    let mut best_s = 0.0;
    let mut best_t = 0.0;
    let mut best_d2 = f64::INFINITY;

    let denom = a * c - b * b;
    if denom > 0.0 {
        let s = (b * e - c * d) / denom;
        let t = (a * e - b * d) / denom;
        if s.is_finite() && t.is_finite() && (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t) {
            let diff = a0 + u * s - (b0 + v * t);
            best_s = s;
            best_t = t;
            best_d2 = diff.length_squared();
        }
    }

    for s in [0.0, 1.0] {
        let point = a0 + u * s;
        let t = ((point - b0).dot(v) / c).clamp(0.0, 1.0);
        let diff = point - (b0 + v * t);
        let d2 = diff.length_squared();
        if d2 < best_d2 {
            best_s = s;
            best_t = t;
            best_d2 = d2;
        }
    }

    for t in [0.0, 1.0] {
        let point = b0 + v * t;
        let s = ((point - a0).dot(u) / a).clamp(0.0, 1.0);
        let diff = a0 + u * s - point;
        let d2 = diff.length_squared();
        if d2 < best_d2 {
            best_s = s;
            best_t = t;
            best_d2 = d2;
        }
    }

    (best_s, best_t, best_d2)
}

/// Intersection of segments `a0`-`a1` and `b0`-`b1` within `tol`. Parameters within `tol`
/// (as a length) of an endpoint are snapped to exactly 0.0 or 1.0. Returns `None` if
/// either segment is shorter than `tol`. For skew segments, a `Point` is reported when
/// their closest approach is within `tol`.
pub fn segment_intersection(a0: DVec3, a1: DVec3, b0: DVec3, b1: DVec3, tol: f64) -> SegmentHit {
    let u = a1 - a0;
    let v = b1 - b0;
    let len_a = u.length();
    let len_b = v.length();
    if len_a < tol || len_b < tol {
        return SegmentHit::None;
    }

    let collinear = point_line_distance(b0, a0, u, len_a) <= tol
        && point_line_distance(b1, a0, u, len_a) <= tol;

    if collinear {
        let a = len_a * len_a;
        let c = len_b * len_b;
        let tb0 = (b0 - a0).dot(u) / a;
        let tb1 = (b1 - a0).dot(u) / a;
        let lo = tb0.min(tb1).max(0.0);
        let hi = tb0.max(tb1).min(1.0);
        let overlap = (hi - lo) * len_a;

        if overlap > tol {
            let a_from = snap_param(lo, len_a, tol);
            let a_to = snap_param(hi, len_a, tol);
            let b_param = |ta: f64| {
                let point = a0 + u * ta;
                snap_param((point - b0).dot(v) / c, len_b, tol)
            };
            return SegmentHit::Overlap {
                a_from,
                a_to,
                b_from: b_param(a_from),
                b_to: b_param(a_to),
            };
        }

        if overlap >= -tol {
            let (ta, tb) = if hi >= lo {
                let mid = ((lo + hi) * 0.5).clamp(0.0, 1.0);
                let point = a0 + u * mid;
                (mid, (point - b0).dot(v) / c)
            } else if lo > 1.0 {
                let tb = if tb0 <= tb1 { 0.0 } else { 1.0 };
                (1.0, tb)
            } else {
                let tb = if tb0 >= tb1 { 0.0 } else { 1.0 };
                (0.0, tb)
            };
            return SegmentHit::Point {
                ta: snap_param(ta, len_a, tol),
                tb: snap_param(tb, len_b, tol),
            };
        }

        return SegmentHit::None;
    }

    let (ta, tb, dist2) = closest_segment_params(a0, a1, b0, b1);
    if dist2 <= tol * tol {
        SegmentHit::Point {
            ta: snap_param(ta, len_a, tol),
            tb: snap_param(tb, len_b, tol),
        }
    } else {
        SegmentHit::None
    }
}

/// Orthonormal in-plane axes `(u, v)` with `u x v == normal`. Deterministic for a given
/// normal. `normal` must be unit length.
pub fn plane_basis(normal: DVec3) -> (DVec3, DVec3) {
    let helper = if normal.x.abs() < 0.9 {
        DVec3::X
    } else {
        DVec3::Y
    };
    let u = helper.cross(normal).normalize();
    let v = normal.cross(u);
    (u, v)
}

/// Coordinates of `pts` along `plane_basis(normal)`, measured from the world origin.
pub fn to_plane_2d(pts: &[DVec3], normal: DVec3) -> Vec<DVec2> {
    let (u, v) = plane_basis(normal);
    pts.iter().map(|p| DVec2::new(p.dot(u), p.dot(v))).collect()
}

/// Signed area of a 2D polygon: positive when counter-clockwise.
pub fn signed_area_2d(poly: &[DVec2]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum * 0.5
}

fn point_segment_distance_2d(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Classify `p` against the polygon (any winding). `Boundary` when within `tol` of an edge.
pub fn point_in_polygon_2d(p: DVec2, poly: &[DVec2], tol: f64) -> Containment {
    if poly.len() < 3 {
        return Containment::Outside;
    }

    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if point_segment_distance_2d(p, a, b) <= tol {
            return Containment::Boundary;
        }
    }

    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = (poly[i].x, poly[i].y);
        let (xj, yj) = (poly[j].x, poly[j].y);
        if (yi > p.y) != (yj > p.y) {
            let x = (xj - xi) * (p.y - yi) / (yj - yi) + xi;
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }

    if inside {
        Containment::Inside
    } else {
        Containment::Outside
    }
}

/// True when the closed polygon has at least 3 vertices, no two consecutive vertices
/// within `tol`, an area above `tol * tol`, and its edges meet only at the shared
/// vertices of consecutive edges (no crossing, touching or overlapping).
pub fn polygon_is_simple_2d(poly: &[DVec2], tol: f64) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }

    for i in 0..n {
        if poly[i].distance(poly[(i + 1) % n]) <= tol {
            return false;
        }
    }

    if signed_area_2d(poly).abs() <= tol * tol {
        return false;
    }

    let at = |i: usize| DVec3::new(poly[i].x, poly[i].y, 0.0);
    for i in 0..n {
        let a0 = at(i);
        let a1 = at((i + 1) % n);
        for j in i + 1..n {
            let b0 = at(j);
            let b1 = at((j + 1) % n);
            let consecutive = j == i + 1 || (i == 0 && j == n - 1);
            match segment_intersection(a0, a1, b0, b1, tol) {
                SegmentHit::None => {}
                SegmentHit::Point { ta, tb } => {
                    let expected = if j == i + 1 {
                        ta == 1.0 && tb == 0.0
                    } else {
                        ta == 0.0 && tb == 1.0
                    };
                    if !consecutive || !expected {
                        return false;
                    }
                }
                SegmentHit::Overlap { .. } => return false,
            }
        }
    }

    true
}
