// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The own mesh validator (`docs/specs/m7-roundtrip.md`).

use glam::DVec3;
use modeler_mesh::print_mesh::Triangle;
use modeler_mesh::validate::validate;

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The box `0..a`, `0..b`, `0..c` as twelve outward triangles.
fn cuboid(a: f64, b: f64, c: f64) -> Vec<Triangle> {
    let q = |i: usize| {
        p(
            if i & 1 != 0 { a } else { 0.0 },
            if i & 2 != 0 { b } else { 0.0 },
            if i & 4 != 0 { c } else { 0.0 },
        )
    };
    // Outward quads (bottom, top, front, back, left, right), split along a diagonal.
    let quads = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    quads
        .iter()
        .flat_map(|f| [[q(f[0]), q(f[1]), q(f[2])], [q(f[0]), q(f[2]), q(f[3])]])
        .collect()
}

const WELD: f64 = 1e-6;

#[test]
fn a_closed_box_is_valid_with_its_volume() {
    let r = validate(&cuboid(2.0, 3.0, 4.0), WELD);
    assert!(r.is_valid(), "{r:?}");
    assert_eq!((r.triangles, r.vertices), (12, 8));
    assert_eq!(
        (
            r.degenerate_triangles,
            r.open_edges,
            r.non_manifold_edges,
            r.inconsistent_edges
        ),
        (0, 0, 0, 0)
    );
    assert!((r.volume - 24.0).abs() < 1e-9, "{}", r.volume);
}

#[test]
fn an_empty_list_is_not_valid() {
    let r = validate(&[], WELD);
    assert!(!r.is_valid());
    assert_eq!(r.triangles, 0);
}

#[test]
fn a_missing_triangle_leaves_open_edges() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    t.pop();
    let r = validate(&t, WELD);
    assert!(!r.is_valid());
    assert_eq!(r.open_edges, 3);
    assert_eq!((r.non_manifold_edges, r.inconsistent_edges), (0, 0));
}

#[test]
fn one_reversed_triangle_is_an_inconsistent_orientation() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    t[4].swap(1, 2);
    let r = validate(&t, WELD);
    assert!(!r.is_valid());
    assert_eq!(r.inconsistent_edges, 3);
    assert_eq!(r.open_edges, 0);
}

#[test]
fn a_box_wound_inside_out_has_negative_volume() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    for tri in &mut t {
        tri.swap(1, 2);
    }
    let r = validate(&t, WELD);
    assert!(!r.is_valid());
    assert!((r.volume + 1.0).abs() < 1e-9);
    assert_eq!(
        (r.open_edges, r.non_manifold_edges, r.inconsistent_edges),
        (0, 0, 0)
    );
}

#[test]
fn a_third_triangle_on_an_edge_is_non_manifold() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    // A fin sticking out of the bottom edge from (0,0,0) to (1,0,0).
    t.push([p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(0.5, -1.0, -1.0)]);
    let r = validate(&t, WELD);
    assert!(!r.is_valid());
    assert_eq!(r.non_manifold_edges, 1);
}

#[test]
fn a_repeated_corner_is_degenerate_and_ignored_by_the_edge_counts() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    t.push([p(0.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(1.0, 1.0, 1.0)]);
    let r = validate(&t, WELD);
    assert!(!r.is_valid());
    assert_eq!(r.degenerate_triangles, 1);
    assert_eq!(
        (r.open_edges, r.non_manifold_edges, r.inconsistent_edges),
        (0, 0, 0)
    );
    assert!((r.volume - 1.0).abs() < 1e-9);
}

#[test]
fn a_collinear_triangle_is_degenerate() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    t.push([p(0.0, 0.0, 0.0), p(0.5, 0.0, 0.0), p(1.0, 0.0, 0.0)]);
    let r = validate(&t, WELD);
    assert_eq!(r.degenerate_triangles, 1);
    assert!(!r.is_valid());
}

#[test]
fn corners_closer_than_the_weld_distance_are_the_same_point() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    // Nudge every use of corner 7 differently, by far less than the weld distance.
    let mut k = 0.0;
    for tri in &mut t {
        for c in tri.iter_mut() {
            if c.distance(p(1.0, 1.0, 1.0)) < 1e-9 {
                k += 1.0;
                *c += p(1e-9 * k, -1e-9 * k, 0.0);
            }
        }
    }
    let r = validate(&t, 1e-3);
    assert!(r.is_valid(), "{r:?}");
    assert_eq!(r.vertices, 8);
}

#[test]
fn corners_further_apart_than_the_weld_distance_stay_apart() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    t[0][0] += p(0.5, 0.0, 0.0);
    let r = validate(&t, 1e-6);
    assert!(!r.is_valid());
    assert!(r.open_edges > 0);
}

#[test]
fn two_separate_closed_boxes_add_their_volumes() {
    let mut t = cuboid(1.0, 1.0, 1.0);
    t.extend(
        cuboid(2.0, 2.0, 2.0)
            .into_iter()
            .map(|tri| tri.map(|q| q + p(10.0, 0.0, 0.0))),
    );
    let r = validate(&t, WELD);
    assert!(r.is_valid(), "{r:?}");
    assert_eq!(r.vertices, 16);
    assert!((r.volume - 9.0).abs() < 1e-9);
}
