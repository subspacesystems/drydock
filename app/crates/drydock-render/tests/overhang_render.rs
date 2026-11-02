// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-overhang: a tinted scene draws its overhangs in the overhang color
//! (`docs/specs/m7-overhang.md`). Headless on lavapipe like `render.rs`.

use drydock_dae::{Aabb, Batch, FlatScene, Material, tint_overhangs};
use drydock_render::{Style, render_headless};
use drydock_view::{Camera, Projection, StandardView};
use glam::DVec3;

const W: u32 = 64;
const H: u32 = 64;

/// Square from (-s, -s) to (s, s) at height `z`, facing down when `down`.
fn square(s: f32, z: f32, down: bool) -> Batch {
    let n = if down { -1.0 } else { 1.0 };
    Batch {
        material: Some(0),
        positions: vec![[-s, -s, z], [s, -s, z], [s, s, z], [-s, s, z]],
        normals: vec![[0.0, 0.0, n]; 4],
        texcoords: vec![],
        indices: if down {
            vec![0, 2, 1, 0, 3, 2]
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    }
}

/// A light grey ceiling at z = 1 and, far off to the side, a small floor patch at z = 0 that
/// sets the bed.
fn scene() -> FlatScene {
    let mut floor = square(0.1, 0.0, false);
    for p in &mut floor.positions {
        p[0] += 50.0;
    }
    let ceiling = square(1.0, 1.0, true);
    FlatScene {
        materials: vec![Material {
            id: "grey".into(),
            name: "grey".into(),
            color: [0.8, 0.8, 0.8, 1.0],
            texture: None,
            opacity: 1.0,
            unlit: false,
        }],
        batches: vec![ceiling, floor],
        lines: vec![],
        bounds: Some(Aabb {
            min: [-1.0, -1.0, 0.0],
            max: [50.1, 1.0, 1.0],
        }),
    }
}

/// Looking straight up at the ceiling's center from below.
fn from_below() -> Camera {
    let fov_y = 35f64.to_radians();
    let mut c = Camera {
        target: DVec3::new(0.0, 0.0, 1.0),
        distance: 0.5 / (fov_y / 2.0).tan(),
        yaw: 0.0,
        pitch: 0.0,
        fov_y,
        projection: Projection::Perspective,
    };
    c.set_standard_view(StandardView::Bottom);
    c
}

fn center(s: &FlatScene) -> [u8; 4] {
    let (img, _) = render_headless(s, &from_below(), W, H, &Style::default())
        .expect("headless device (lavapipe) available");
    img.get_pixel(W / 2, H / 2).0
}

#[test]
fn the_untinted_ceiling_is_grey() {
    let [r, g, b, _] = center(&scene());
    assert!(
        r.abs_diff(g) <= 8 && r.abs_diff(b) <= 8,
        "grey expected, got {r} {g} {b}"
    );
}

#[test]
fn the_tinted_ceiling_is_red() {
    let tinted = tint_overhangs(&scene(), 45.0);
    let [r, g, b, _] = center(&tinted);
    assert!(
        i32::from(r) > i32::from(g) + 60 && i32::from(r) > i32::from(b) + 60,
        "overhang red expected, got {r} {g} {b}"
    );
}

#[test]
fn a_ninety_degree_limit_leaves_it_grey() {
    let tinted = tint_overhangs(&scene(), 90.0);
    let [r, g, b, _] = center(&tinted);
    assert!(
        r.abs_diff(g) <= 8 && r.abs_diff(b) <= 8,
        "grey expected, got {r} {g} {b}"
    );
}
