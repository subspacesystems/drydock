// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-zoom-through: `FlatScene::ray_hits`, every surface along the cursor's ray.
//! See docs/specs/m4-zoom-through.md.

use drydock_dae::{Batch, FlatScene, LineBatch};
use glam::DVec3;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A square (-s..s) at height z, two triangles, wound up or down.
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

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

#[test]
fn every_square_along_the_ray_is_listed_nearest_first() {
    // Batches out of order on purpose: the result is sorted, not in scene order.
    let s = scene(vec![quad(5.0, 1.0, false), quad(5.0, 3.0, true)]);
    let hits = s.ray_hits(v(0.3, 0.2, 10.0), v(0.0, 0.0, -1.0));
    assert!(close(&hits, &[7.0, 9.0]), "{hits:?}");
}

#[test]
fn surfaces_behind_the_origin_are_left_out() {
    let s = scene(vec![quad(5.0, 1.0, false), quad(5.0, 3.0, false)]);
    let hits = s.ray_hits(v(0.3, 0.2, 2.0), v(0.0, 0.0, -1.0));
    assert!(close(&hits, &[1.0]), "{hits:?}");
    let up = s.ray_hits(v(0.3, 0.2, 2.0), v(0.0, 0.0, 1.0));
    assert!(close(&up, &[1.0]), "{up:?}");
}

#[test]
fn values_are_ray_parameters_not_lengths() {
    let s = scene(vec![quad(5.0, 1.0, false), quad(5.0, 3.0, false)]);
    let hits = s.ray_hits(v(0.3, 0.2, 10.0), v(0.0, 0.0, -2.0));
    assert!(close(&hits, &[3.5, 4.5]), "{hits:?}");
}

#[test]
fn a_ray_along_a_shared_edge_gives_only_that_surface() {
    // Through the diagonal both triangles of the square may report the crossing; every
    // value is still that one surface.
    let s = scene(vec![quad(5.0, 1.0, false)]);
    let hits = s.ray_hits(v(0.0, 0.0, 4.0), v(0.0, 0.0, -1.0));
    assert!(!hits.is_empty() && hits.len() <= 2, "{hits:?}");
    assert!(hits.iter().all(|t| (t - 3.0).abs() < 1e-5), "{hits:?}");
}

#[test]
fn a_ray_beside_every_square_hits_nothing() {
    let s = scene(vec![quad(1.0, 1.0, false), quad(1.0, 3.0, false)]);
    assert!(s.ray_hits(v(4.0, 0.0, 10.0), v(0.0, 0.0, -1.0)).is_empty());
}

#[test]
fn lines_empty_scenes_and_degenerate_rays_give_nothing() {
    let mut s = scene(vec![]);
    s.lines.push(LineBatch {
        material: None,
        positions: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
    });
    assert!(s.ray_hits(v(0.0, 0.0, 5.0), v(0.0, 0.0, -1.0)).is_empty());
    let s = scene(vec![quad(5.0, 1.0, false)]);
    assert!(s.ray_hits(v(0.0, 0.0, 5.0), DVec3::ZERO).is_empty());
    assert!(
        s.ray_hits(v(f64::NAN, 0.0, 5.0), v(0.0, 0.0, -1.0))
            .is_empty()
    );
    assert!(
        s.ray_hits(v(0.0, 0.0, 5.0), v(0.0, 0.0, f64::INFINITY))
            .is_empty()
    );
    let mut broken = quad(5.0, 1.0, false);
    broken.indices = vec![0, 1, 9];
    assert!(
        scene(vec![broken])
            .ray_hits(v(0.0, 0.0, 5.0), v(0.0, 0.0, -1.0))
            .is_empty()
    );
}

proptest! {
    /// The first value is the point `raycast` returns, and the values never decrease.
    #[test]
    fn the_first_hit_is_the_raycast_point(
        zs in prop::collection::vec(-20.0f32..20.0, 1..6),
        x in -0.9f64..0.9,
        y in -0.9f64..0.9,
        dx in -0.2f64..0.2,
        dy in -0.2f64..0.2,
        oz in -30.0f64..30.0,
        down in any::<bool>(),
    ) {
        // Squares wide enough that every slanted ray stays over them.
        let s = scene(zs.iter().map(|&z| quad(60.0, z, false)).collect());
        let origin = v(x, y, oz);
        let dir = v(dx, dy, if down { -1.0 } else { 1.0 });
        let hits = s.ray_hits(origin, dir);
        prop_assert!(hits.windows(2).all(|w| w[0] <= w[1]), "{:?}", hits);
        prop_assert!(hits.iter().all(|&t| t > 0.0), "{:?}", hits);
        match s.raycast(origin, dir) {
            Some(p) => {
                prop_assert!(!hits.is_empty());
                prop_assert!((origin + dir * hits[0]).distance(p) < 1e-6, "{:?} {:?}", hits, p);
            }
            None => prop_assert!(hits.is_empty(), "{:?}", hits),
        }
        // Each square the ray crosses is listed (twice at most, on its diagonal).
        let crossed = zs
            .iter()
            .filter(|&&z| (f64::from(z) - oz) * dir.z > 0.0)
            .count();
        prop_assert!(hits.len() >= crossed && hits.len() <= 2 * crossed, "{:?} {}", hits, crossed);
    }
}
