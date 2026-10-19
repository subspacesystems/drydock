// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::{DVec2, DVec3};
use modeler_kernel::geom::*;

const T: f64 = LINEAR_TOL;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}
fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn tolerance_is_ten_micrometers() {
    assert_eq!(LINEAR_TOL, 1e-5);
}

#[test]
fn area_vector_and_normal_of_ccw_square() {
    let sq = [v(0., 0., 0.), v(2., 0., 0.), v(2., 2., 0.), v(0., 2., 0.)];
    assert!((area_vector(&sq) - v(0., 0., 4.)).length() < 1e-12);
    assert!((polygon_normal(&sq) - DVec3::Z).length() < 1e-12);
    let cw: Vec<_> = sq.iter().rev().copied().collect();
    assert!((polygon_normal(&cw) + DVec3::Z).length() < 1e-12);
}

#[test]
fn area_vector_is_translation_stable() {
    let sq = [v(0., 0., 0.), v(2., 0., 0.), v(2., 2., 0.), v(0., 2., 0.)];
    let far: Vec<_> = sq.iter().map(|p| *p + v(1e5, -2e5, 3e4)).collect();
    assert!((area_vector(&far) - v(0., 0., 4.)).length() < 1e-6);
}

#[test]
fn degenerate_polygons_have_no_normal_or_plane() {
    assert_eq!(polygon_normal(&[]), DVec3::ZERO);
    assert_eq!(polygon_normal(&[v(0., 0., 0.), v(1., 0., 0.)]), DVec3::ZERO);
    let line = [v(0., 0., 0.), v(1., 0., 0.), v(2., 0., 0.)];
    assert_eq!(polygon_normal(&line), DVec3::ZERO);
    assert!(fit_plane(&line).is_none());
    assert!(!is_planar(&line, 1.0));
    assert_eq!(area_vector(&line[..2]), DVec3::ZERO);
}

#[test]
fn planarity_uses_tolerance() {
    let mut q = vec![v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)];
    assert!(is_planar(&q, T));
    let plane = fit_plane(&q).unwrap();
    assert!((plane.normal - DVec3::Z).length() < 1e-12);
    assert!(close(plane.signed_distance(v(5., 5., 3.)), 2.0));
    assert!(close(plane.signed_distance(v(5., 5., 0.)), -1.0));
    q[2].z += 5.0 * T;
    assert!(!is_planar(&q, T));
    assert!(is_planar(&q, 1e-3));
    let p = fit_plane(&q).unwrap();
    assert!(max_plane_deviation(&q, &p) > T);
}

#[test]
fn triangle_is_always_planar() {
    let t = [v(0., 0., 0.), v(3., 1., 7.), v(-2., 5., 1.)];
    assert!(is_planar(&t, T));
}

#[test]
fn point_segment_distance_cases() {
    let (a, b) = (v(0., 0., 0.), v(10., 0., 0.));
    assert!(close(point_segment_distance(v(5., 3., 0.), a, b), 3.0));
    assert!(close(point_segment_distance(v(-3., 4., 0.), a, b), 5.0));
    assert!(close(point_segment_distance(v(13., 4., 0.), a, b), 5.0));
    assert!(close(
        point_segment_distance(v(1., 1., 1.), a, a),
        3f64.sqrt()
    ));
}

#[test]
fn crossing_segments_report_parameters() {
    let hit = segment_intersection(
        v(0., 0., 0.),
        v(4., 0., 0.),
        v(1., -1., 0.),
        v(1., 3., 0.),
        T,
    );
    match hit {
        SegmentHit::Point { ta, tb } => {
            assert!(close(ta, 0.25));
            assert!(close(tb, 0.25));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn t_junction_and_shared_endpoint_snap_to_exact_ends() {
    let t = segment_intersection(
        v(0., 0., 0.),
        v(4., 0., 0.),
        v(2., 0., 0.),
        v(2., 5., 0.),
        T,
    );
    assert_eq!(t, SegmentHit::Point { ta: 0.5, tb: 0.0 });
    let e = segment_intersection(
        v(0., 0., 0.),
        v(4., 0., 0.),
        v(4., 0., 0.),
        v(4., 5., 0.),
        T,
    );
    assert_eq!(e, SegmentHit::Point { ta: 1.0, tb: 0.0 });
    // Within tolerance of the end counts as the end.
    let n = segment_intersection(
        v(0., 0., 0.),
        v(4., 0., 0.),
        v(4. + T / 2.0, 0., 0.),
        v(4., 5., 0.),
        T,
    );
    assert_eq!(n, SegmentHit::Point { ta: 1.0, tb: 0.0 });
}

#[test]
fn skew_segments_hit_only_within_tolerance() {
    let a = (v(0., 0., 0.), v(4., 0., 0.));
    let far = segment_intersection(a.0, a.1, v(2., -1., 0.5), v(2., 1., 0.5), T);
    assert_eq!(far, SegmentHit::None);
    let near = segment_intersection(a.0, a.1, v(2., -1., T / 2.0), v(2., 1., T / 2.0), T);
    assert!(matches!(near, SegmentHit::Point { .. }));
}

#[test]
fn near_misses_do_not_hit() {
    // Lines would cross at x=5, past the end of a.
    let h = segment_intersection(
        v(0., 0., 0.),
        v(4., 0., 0.),
        v(5., -1., 0.),
        v(5., 1., 0.),
        T,
    );
    assert_eq!(h, SegmentHit::None);
    let h = segment_intersection(
        v(0., 0., 0.),
        v(4., 0., 0.),
        v(0., 1., 0.),
        v(4., 1., 0.),
        T,
    );
    assert_eq!(h, SegmentHit::None);
}

#[test]
fn collinear_overlap_touch_and_gap() {
    let a0 = v(0., 0., 0.);
    let a1 = v(10., 0., 0.);
    let o = segment_intersection(a0, a1, v(4., 0., 0.), v(14., 0., 0.), T);
    match o {
        SegmentHit::Overlap {
            a_from,
            a_to,
            b_from,
            b_to,
        } => {
            assert!(close(a_from, 0.4) && close(a_to, 1.0));
            assert!(close(b_from, 0.0) && close(b_to, 0.6));
        }
        other => panic!("{other:?}"),
    }
    // Reversed b: b_from is the parameter at a_from.
    let r = segment_intersection(a0, a1, v(14., 0., 0.), v(4., 0., 0.), T);
    match r {
        SegmentHit::Overlap {
            a_from,
            a_to,
            b_from,
            b_to,
        } => {
            assert!(close(a_from, 0.4) && close(a_to, 1.0));
            assert!(close(b_from, 1.0) && close(b_to, 0.4));
        }
        other => panic!("{other:?}"),
    }
    // b inside a.
    let inside = segment_intersection(a0, a1, v(2., 0., 0.), v(3., 0., 0.), T);
    assert!(
        matches!(inside, SegmentHit::Overlap { a_from, a_to, .. } if close(a_from, 0.2) && close(a_to, 0.3))
    );
    // End to end collinear: a single point.
    let touch = segment_intersection(a0, a1, v(10., 0., 0.), v(15., 0., 0.), T);
    assert_eq!(touch, SegmentHit::Point { ta: 1.0, tb: 0.0 });
    let gap = segment_intersection(a0, a1, v(11., 0., 0.), v(15., 0., 0.), T);
    assert_eq!(gap, SegmentHit::None);
}

#[test]
fn degenerate_segments_never_hit() {
    let p = v(1., 1., 1.);
    assert_eq!(
        segment_intersection(p, p, v(0., 1., 1.), v(2., 1., 1.), T),
        SegmentHit::None
    );
    assert_eq!(
        segment_intersection(v(0., 1., 1.), v(2., 1., 1.), p, p, T),
        SegmentHit::None
    );
}

#[test]
fn plane_basis_is_right_handed_and_orthonormal() {
    for n in [
        DVec3::X,
        DVec3::Y,
        DVec3::Z,
        -DVec3::Z,
        v(1., 2., 3.).normalize(),
    ] {
        let (u, w) = plane_basis(n);
        assert!(close(u.length(), 1.0) && close(w.length(), 1.0));
        assert!(u.dot(w).abs() < 1e-12 && u.dot(n).abs() < 1e-12 && w.dot(n).abs() < 1e-12);
        assert!((u.cross(w) - n).length() < 1e-12);
        assert_eq!((u, w), plane_basis(n));
    }
}

#[test]
fn to_plane_2d_preserves_area_and_orientation() {
    let tri = [v(0., 0., 5.), v(3., 0., 5.), v(0., 4., 5.)];
    let flat = to_plane_2d(&tri, DVec3::Z);
    assert!(close(signed_area_2d(&flat), 6.0));
    let flat_down = to_plane_2d(&tri, -DVec3::Z);
    assert!(close(signed_area_2d(&flat_down), -6.0));
}

#[test]
fn signed_area_sign_follows_winding() {
    let ccw = [v2(0., 0.), v2(2., 0.), v2(2., 1.), v2(0., 1.)];
    assert!(close(signed_area_2d(&ccw), 2.0));
    let cw: Vec<_> = ccw.iter().rev().copied().collect();
    assert!(close(signed_area_2d(&cw), -2.0));
    assert_eq!(signed_area_2d(&ccw[..2]), 0.0);
}

#[test]
fn point_in_polygon_inside_outside_boundary() {
    // L shape (concave).
    let l = [
        v2(0., 0.),
        v2(4., 0.),
        v2(4., 1.),
        v2(1., 1.),
        v2(1., 4.),
        v2(0., 4.),
    ];
    assert_eq!(
        point_in_polygon_2d(v2(0.5, 0.5), &l, T),
        Containment::Inside
    );
    assert_eq!(
        point_in_polygon_2d(v2(0.5, 3.0), &l, T),
        Containment::Inside
    );
    assert_eq!(
        point_in_polygon_2d(v2(3.0, 3.0), &l, T),
        Containment::Outside
    );
    assert_eq!(
        point_in_polygon_2d(v2(-1., 0.5), &l, T),
        Containment::Outside
    );
    assert_eq!(
        point_in_polygon_2d(v2(2., 0.), &l, T),
        Containment::Boundary
    );
    assert_eq!(
        point_in_polygon_2d(v2(1., 1.), &l, T),
        Containment::Boundary
    );
    assert_eq!(
        point_in_polygon_2d(v2(2., 1. + T / 2.0), &l, T),
        Containment::Boundary
    );
    assert_eq!(
        point_in_polygon_2d(v2(2., 1. + 10.0 * T), &l, T),
        Containment::Outside
    );
    // Winding does not matter.
    let rev: Vec<_> = l.iter().rev().copied().collect();
    assert_eq!(
        point_in_polygon_2d(v2(0.5, 0.5), &rev, T),
        Containment::Inside
    );
    assert_eq!(
        point_in_polygon_2d(v2(0., 0.), &l[..2], T),
        Containment::Outside
    );
}

#[test]
fn simple_polygons() {
    let sq = [v2(0., 0.), v2(1., 0.), v2(1., 1.), v2(0., 1.)];
    assert!(polygon_is_simple_2d(&sq, T));
    let rev: Vec<_> = sq.iter().rev().copied().collect();
    assert!(polygon_is_simple_2d(&rev, T));
    let tri = [v2(0., 0.), v2(1., 0.), v2(0., 1.)];
    assert!(polygon_is_simple_2d(&tri, T));
    let l = [
        v2(0., 0.),
        v2(4., 0.),
        v2(4., 1.),
        v2(1., 1.),
        v2(1., 4.),
        v2(0., 4.),
    ];
    assert!(polygon_is_simple_2d(&l, T));
    // Collinear extra vertex on an edge is fine.
    let mid = [v2(0., 0.), v2(0.5, 0.), v2(1., 0.), v2(1., 1.), v2(0., 1.)];
    assert!(polygon_is_simple_2d(&mid, T));
}

#[test]
fn non_simple_polygons() {
    let bow = [v2(0., 0.), v2(1., 1.), v2(1., 0.), v2(0., 1.)];
    assert!(!polygon_is_simple_2d(&bow, T));
    assert!(!polygon_is_simple_2d(&[v2(0., 0.), v2(1., 0.)], T));
    // Repeated vertex.
    assert!(!polygon_is_simple_2d(
        &[v2(0., 0.), v2(0., 0.), v2(1., 0.), v2(0., 1.)],
        T
    ));
    // Zero area (all collinear).
    assert!(!polygon_is_simple_2d(
        &[v2(0., 0.), v2(1., 0.), v2(2., 0.)],
        T
    ));
    // Pinched: vertex touches a non-adjacent edge.
    let pinch = [v2(0., 0.), v2(4., 0.), v2(4., 4.), v2(2., 0.), v2(0., 4.)];
    assert!(!polygon_is_simple_2d(&pinch, T));
    // Spike doubling back over itself.
    let spike = [v2(0., 0.), v2(2., 0.), v2(1., 0.), v2(1., 1.)];
    assert!(!polygon_is_simple_2d(&spike, T));
}
