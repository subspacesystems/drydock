// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-orbit-pivot: `FlatScene::raycast`, the point under the cursor.
//! See docs/specs/m4-orbit-pivot.md.

use glam::DVec3;
use modeler_dae::{Batch, FlatScene, LineBatch};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A unit-free square (-s..s) at height z, two triangles, wound up or down.
fn quad(s: f32, z: f32, down: bool) -> Batch {
    Batch {
        material: None,
        positions: vec![[-s, -s, z], [s, -s, z], [s, s, z], [-s, s, z]],
        normals: vec![[0.0, 0.0, if down { -1.0 } else { 1.0 }]; 4],
        texcoords: vec![],
        indices: if down {
            vec![0, 2, 1, 0, 3, 2]
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    }
}

fn scene(batches: Vec<Batch>) -> FlatScene {
    FlatScene {
        materials: vec![],
        batches,
        lines: vec![],
        bounds: None,
    }
}

fn near(a: Option<DVec3>, b: DVec3) -> bool {
    a.is_some_and(|a| a.distance(b) < 1e-5)
}

#[test]
fn a_ray_down_onto_a_square_hits_it() {
    let s = scene(vec![quad(1.0, 0.0, false)]);
    let hit = s.raycast(v(0.25, -0.5, 10.0), v(0.0, 0.0, -1.0));
    assert!(near(hit, v(0.25, -0.5, 0.0)), "{hit:?}");
}

#[test]
fn back_sides_count_too() {
    let s = scene(vec![quad(1.0, 0.0, true)]);
    let hit = s.raycast(v(0.5, 0.5, 10.0), v(0.0, 0.0, -1.0));
    assert!(near(hit, v(0.5, 0.5, 0.0)), "{hit:?}");
    let from_below = s.raycast(v(0.5, 0.5, -10.0), v(0.0, 0.0, 1.0));
    assert!(near(from_below, v(0.5, 0.5, 0.0)), "{from_below:?}");
}

#[test]
fn a_ray_beside_the_square_misses() {
    let s = scene(vec![quad(1.0, 0.0, false)]);
    assert_eq!(s.raycast(v(1.5, 0.0, 10.0), v(0.0, 0.0, -1.0)), None);
    // Pointing away from it.
    assert_eq!(s.raycast(v(0.0, 0.0, 10.0), v(0.0, 0.0, 1.0)), None);
}

#[test]
fn the_nearest_hit_wins_whatever_the_batch_order() {
    for order in [false, true] {
        let mut batches = vec![quad(1.0, 0.0, false), quad(1.0, 2.0, false)];
        if order {
            batches.reverse();
        }
        let s = scene(batches);
        let hit = s.raycast(v(0.0, 0.5, 10.0), v(0.0, 0.0, -1.0));
        assert!(near(hit, v(0.0, 0.5, 2.0)), "{hit:?}");
        let up = s.raycast(v(0.0, 0.5, -10.0), v(0.0, 0.0, 1.0));
        assert!(near(up, v(0.0, 0.5, 0.0)), "{up:?}");
    }
}

#[test]
fn only_hits_in_front_of_the_origin_count() {
    let s = scene(vec![quad(1.0, 0.0, false), quad(1.0, 2.0, false)]);
    let hit = s.raycast(v(0.0, 0.0, 1.0), v(0.0, 0.0, -1.0));
    assert!(near(hit, v(0.0, 0.0, 0.0)), "{hit:?}");
}

#[test]
fn slanted_rays_hit_where_they_cross() {
    let s = scene(vec![quad(2.0, 0.0, false)]);
    let dir = v(1.0, 1.0, -2.0).normalize();
    let hit = s.raycast(v(-1.0, -1.0, 2.0), dir);
    assert!(near(hit, v(0.0, 0.0, 0.0)), "{hit:?}");
    // A direction that isn't unit length gives the same point.
    let hit = s.raycast(v(-1.0, -1.0, 2.0), dir * 7.0);
    assert!(near(hit, v(0.0, 0.0, 0.0)), "{hit:?}");
}

#[test]
fn lines_are_not_hit() {
    let mut s = scene(vec![]);
    s.lines.push(LineBatch {
        material: None,
        positions: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
    });
    assert_eq!(s.raycast(v(0.0, 0.0, 5.0), v(0.0, 0.0, -1.0)), None);
}

#[test]
fn empty_scenes_degenerate_rays_and_broken_indices_hit_nothing() {
    assert_eq!(
        FlatScene::default().raycast(v(0.0, 0.0, 5.0), v(0.0, 0.0, -1.0)),
        None
    );
    let s = scene(vec![quad(1.0, 0.0, false)]);
    assert_eq!(s.raycast(v(0.0, 0.0, 5.0), DVec3::ZERO), None);
    assert_eq!(s.raycast(v(0.0, 0.0, 5.0), v(f64::NAN, 0.0, -1.0)), None);
    let mut broken = quad(1.0, 0.0, false);
    broken.indices = vec![0, 1, 99];
    assert_eq!(
        scene(vec![broken]).raycast(v(0.0, -0.5, 5.0), v(0.0, 0.0, -1.0)),
        None
    );
}

#[test]
fn a_ray_in_the_plane_of_a_triangle_misses_it() {
    let s = scene(vec![quad(1.0, 0.0, false)]);
    assert_eq!(s.raycast(v(-5.0, 0.0, 0.0), v(1.0, 0.0, 0.0)), None);
}
