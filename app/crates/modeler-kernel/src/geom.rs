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
        let _ = p;
        unimplemented!("m1-geom")
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
    let _ = (pts,);
    unimplemented!("m1-geom")
}

/// Unit normal of the polygon (right-hand rule along the vertex order), or `DVec3::ZERO`
/// when the polygon has fewer than 3 points or its area is below 1e-12 m².
pub fn polygon_normal(pts: &[DVec3]) -> DVec3 {
    let _ = (pts,);
    unimplemented!("m1-geom")
}

/// Best-fit plane: normal from `polygon_normal`, point at the vertex average. `None` when
/// the normal is zero.
pub fn fit_plane(pts: &[DVec3]) -> Option<Plane> {
    let _ = (pts,);
    unimplemented!("m1-geom")
}

/// Largest distance of any point from the plane.
pub fn max_plane_deviation(pts: &[DVec3], plane: &Plane) -> f64 {
    let _ = (pts, plane);
    unimplemented!("m1-geom")
}

/// True when the points have a fit plane and all lie within `tol` of it.
pub fn is_planar(pts: &[DVec3], tol: f64) -> bool {
    let _ = (pts, tol);
    unimplemented!("m1-geom")
}

/// Distance from `p` to the segment `a`-`b` (a degenerate segment is a point).
pub fn point_segment_distance(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let _ = (p, a, b);
    unimplemented!("m1-geom")
}

/// Intersection of segments `a0`-`a1` and `b0`-`b1` within `tol`. Parameters within `tol`
/// (as a length) of an endpoint are snapped to exactly 0.0 or 1.0. Returns `None` if
/// either segment is shorter than `tol`. For skew segments, a `Point` is reported when
/// their closest approach is within `tol`.
pub fn segment_intersection(a0: DVec3, a1: DVec3, b0: DVec3, b1: DVec3, tol: f64) -> SegmentHit {
    let _ = (a0, a1, b0, b1, tol);
    unimplemented!("m1-geom")
}

/// Orthonormal in-plane axes `(u, v)` with `u x v == normal`. Deterministic for a given
/// normal. `normal` must be unit length.
pub fn plane_basis(normal: DVec3) -> (DVec3, DVec3) {
    let _ = (normal,);
    unimplemented!("m1-geom")
}

/// Coordinates of `pts` along `plane_basis(normal)`, measured from the world origin.
pub fn to_plane_2d(pts: &[DVec3], normal: DVec3) -> Vec<DVec2> {
    let _ = (pts, normal);
    unimplemented!("m1-geom")
}

/// Signed area of a 2D polygon: positive when counter-clockwise.
pub fn signed_area_2d(poly: &[DVec2]) -> f64 {
    let _ = (poly,);
    unimplemented!("m1-geom")
}

/// Classify `p` against the polygon (any winding). `Boundary` when within `tol` of an edge.
pub fn point_in_polygon_2d(p: DVec2, poly: &[DVec2], tol: f64) -> Containment {
    let _ = (p, poly, tol);
    unimplemented!("m1-geom")
}

/// True when the closed polygon has at least 3 vertices, no two consecutive vertices
/// within `tol`, an area above `tol * tol`, and its edges meet only at the shared
/// vertices of consecutive edges (no crossing, touching or overlapping).
pub fn polygon_is_simple_2d(poly: &[DVec2], tol: f64) -> bool {
    let _ = (poly, tol);
    unimplemented!("m1-geom")
}
