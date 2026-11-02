// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tape measure and guides (`docs/specs/m3-tape.md`).

use drydock_kernel::tape::{Guide, GuideId, Guides, measure, parallel_guide, point_guide};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn line(g: Guide) -> (DVec3, DVec3) {
    match g {
        Guide::Line { point, dir } => (point, dir),
        other => panic!("expected a line, got {other:?}"),
    }
}

#[test]
fn measure_is_euclidean() {
    assert_eq!(measure(v(0.0, 0.0, 0.0), v(3.0, 4.0, 0.0)), 5.0);
    assert_eq!(measure(v(1.0, 1.0, 1.0), v(1.0, 1.0, 1.0)), 0.0);
    assert!((measure(v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0)) - 3f64.sqrt()).abs() < 1e-12);
}

#[test]
fn measure_is_symmetric() {
    let (a, b) = (v(1.0, -2.0, 3.0), v(-4.0, 5.0, 0.5));
    assert_eq!(measure(a, b), measure(b, a));
}

#[test]
fn parallel_guide_through_the_cursor() {
    // Edge along X; cursor 2 m up in Y and 7 m along: the guide sits 2 m away, still along X.
    let g = parallel_guide(v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0), v(7.0, 2.0, 0.0), None).unwrap();
    let (p, d) = line(g);
    assert!((d - v(1.0, 0.0, 0.0)).length() < 1e-12);
    assert!((p - v(0.0, 2.0, 0.0)).length() < 1e-9);
}

#[test]
fn parallel_guide_direction_follows_the_edge_order() {
    let g = parallel_guide(v(4.0, 0.0, 0.0), v(0.0, 0.0, 0.0), v(1.0, 2.0, 0.0), None).unwrap();
    let (_, d) = line(g);
    assert!((d - v(-1.0, 0.0, 0.0)).length() < 1e-12);
}

#[test]
fn parallel_guide_direction_is_unit_length() {
    let g = parallel_guide(v(0.0, 0.0, 0.0), v(3.0, 4.0, 0.0), v(0.0, 0.0, 5.0), None).unwrap();
    let (_, d) = line(g);
    assert!((d.length() - 1.0).abs() < 1e-12);
}

#[test]
fn parallel_guide_in_a_tilted_plane() {
    // Edge along X; cursor above it in Z (the side is the cursor's side, in 3D).
    let g = parallel_guide(v(1.0, 1.0, 1.0), v(3.0, 1.0, 1.0), v(2.0, 1.0, 4.0), None).unwrap();
    let (p, d) = line(g);
    assert!((d - v(1.0, 0.0, 0.0)).length() < 1e-12);
    assert!((p - v(1.0, 1.0, 4.0)).length() < 1e-9);
}

#[test]
fn typed_distance_replaces_the_cursor_distance_but_keeps_the_side() {
    let g = parallel_guide(
        v(0.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(2.0, -9.0, 0.0),
        Some(1.5),
    )
    .unwrap();
    let (p, _) = line(g);
    assert!((p - v(0.0, -1.5, 0.0)).length() < 1e-9);
}

#[test]
fn guide_distance_to_the_edge_line_matches() {
    let (a, b) = (v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0));
    let g = parallel_guide(a, b, v(1.0, 0.0, 3.0), None).unwrap();
    assert!((g.distance_to(a) - 3.0).abs() < 1e-9);
    assert!((g.distance_to(b) - 3.0).abs() < 1e-9);
}

#[test]
fn parallel_guide_refuses_degenerate_input() {
    let a = v(0.0, 0.0, 0.0);
    // zero-length edge
    assert!(parallel_guide(a, a, v(1.0, 1.0, 0.0), None).is_none());
    assert!(parallel_guide(a, v(1e-7, 0.0, 0.0), v(1.0, 1.0, 0.0), None).is_none());
    // cursor on the edge's line: no side, with or without a typed distance
    let b = v(4.0, 0.0, 0.0);
    assert!(parallel_guide(a, b, v(9.0, 0.0, 0.0), None).is_none());
    assert!(parallel_guide(a, b, v(9.0, 0.0, 0.0), Some(1.0)).is_none());
    // bad typed distances
    for t in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-9] {
        assert!(
            parallel_guide(a, b, v(1.0, 1.0, 0.0), Some(t)).is_none(),
            "{t}"
        );
    }
    // non-finite points
    assert!(parallel_guide(a, b, v(f64::NAN, 1.0, 0.0), None).is_none());
    assert!(parallel_guide(a, v(f64::INFINITY, 0.0, 0.0), v(1.0, 1.0, 0.0), None).is_none());
}

#[test]
fn point_guide_is_the_point() {
    assert_eq!(
        point_guide(v(1.0, 2.0, 3.0)),
        Some(Guide::Point(v(1.0, 2.0, 3.0)))
    );
    assert!(point_guide(v(f64::NAN, 0.0, 0.0)).is_none());
    assert!(point_guide(v(0.0, f64::NEG_INFINITY, 0.0)).is_none());
}

#[test]
fn point_guide_distance() {
    let g = point_guide(v(1.0, 0.0, 0.0)).unwrap();
    assert!((g.distance_to(v(1.0, 3.0, 4.0)) - 5.0).abs() < 1e-12);
    assert_eq!(g.distance_to(v(1.0, 0.0, 0.0)), 0.0);
}

#[test]
fn line_distance_ignores_position_along_the_line() {
    let g = Guide::Line {
        point: v(0.0, 0.0, 0.0),
        dir: v(0.0, 0.0, 1.0),
    };
    assert!((g.distance_to(v(3.0, 4.0, 100.0)) - 5.0).abs() < 1e-12);
    assert!((g.distance_to(v(3.0, 4.0, -100.0)) - 5.0).abs() < 1e-12);
}

#[test]
fn guides_add_get_remove() {
    let mut gs = Guides::new();
    assert!(gs.is_empty());
    assert_eq!(gs.len(), 0);
    let a = gs.add(Guide::Point(v(1.0, 0.0, 0.0)));
    let b = gs.add(Guide::Point(v(2.0, 0.0, 0.0)));
    assert_ne!(a, b);
    assert_eq!(gs.len(), 2);
    assert!(!gs.is_empty());
    assert_eq!(gs.get(b), Some(&Guide::Point(v(2.0, 0.0, 0.0))));
    assert_eq!(gs.remove(a), Some(Guide::Point(v(1.0, 0.0, 0.0))));
    assert_eq!(gs.remove(a), None);
    assert_eq!(gs.get(a), None);
    assert_eq!(gs.len(), 1);
}

#[test]
fn guide_ids_are_never_reused() {
    let mut gs = Guides::new();
    let a = gs.add(Guide::Point(v(0.0, 0.0, 0.0)));
    gs.remove(a);
    let b = gs.add(Guide::Point(v(0.0, 0.0, 0.0)));
    assert_ne!(a, b);
    gs.clear();
    let c = gs.add(Guide::Point(v(0.0, 0.0, 0.0)));
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn iteration_is_in_creation_order() {
    let mut gs = Guides::new();
    let ids: Vec<GuideId> = (0..4)
        .map(|i| gs.add(Guide::Point(v(i as f64, 0.0, 0.0))))
        .collect();
    gs.remove(ids[1]);
    let seen: Vec<GuideId> = gs.iter().map(|(i, _)| i).collect();
    assert_eq!(seen, vec![ids[0], ids[2], ids[3]]);
    let g = gs.iter().nth(1).unwrap().1;
    assert_eq!(*g, Guide::Point(v(2.0, 0.0, 0.0)));
}

#[test]
fn clear_returns_what_it_removed_in_order() {
    let mut gs = Guides::new();
    let a = gs.add(Guide::Point(v(1.0, 0.0, 0.0)));
    let b = gs.add(Guide::Point(v(2.0, 0.0, 0.0)));
    let gone = gs.clear();
    assert_eq!(
        gone,
        vec![
            (a, Guide::Point(v(1.0, 0.0, 0.0))),
            (b, Guide::Point(v(2.0, 0.0, 0.0)))
        ]
    );
    assert!(gs.is_empty());
    assert!(gs.clear().is_empty());
}

#[test]
fn clone_is_independent() {
    let mut gs = Guides::new();
    gs.add(Guide::Point(v(1.0, 0.0, 0.0)));
    let copy = gs.clone();
    gs.add(Guide::Point(v(2.0, 0.0, 0.0)));
    assert_eq!(copy.len(), 1);
    assert_eq!(gs.len(), 2);
}
