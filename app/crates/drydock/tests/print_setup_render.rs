// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-print-setup: the bed outline in a headless render (`docs/specs/m7-print-setup.md`).

use drydock::print_setup::{BED_COLOR, Bed, bed_outline};
use drydock_dae::FlatScene;
use drydock_render::{Style, render_headless_with_lines};
use drydock_view::{Camera, Projection};
use glam::{DVec2, DVec3};

const W: u32 = 128;
const H: u32 = 128;

fn top_camera() -> Camera {
    Camera {
        target: DVec3::ZERO,
        distance: 0.6,
        yaw: 0.0,
        pitch: 90f64.to_radians(),
        fov_y: 35f64.to_radians(),
        projection: Projection::Parallel,
    }
}

fn pixel(c: &Camera, p: DVec3) -> (u32, u32) {
    let (ndc, _) = c
        .project(p, W as f64 / H as f64)
        .expect("point in front of the camera");
    let px = (ndc + DVec2::ONE) / 2.0 * DVec2::new(W as f64, H as f64);
    (px.x.floor() as u32, (H as f64 - px.y).floor() as u32)
}

fn is_bed_colored(p: [u8; 4]) -> bool {
    let want = BED_COLOR.map(|c| (c * 255.0) as i32);
    (0..3).all(|i| (p[i] as i32 - want[i]).abs() < 60)
}

fn near_bed_color(img: &image::RgbaImage, (x, y): (u32, u32)) -> bool {
    (x.saturating_sub(2)..=(x + 2).min(W - 1)).any(|i| {
        (y.saturating_sub(2)..=(y + 2).min(H - 1)).any(|j| is_bed_colored(img.get_pixel(i, j).0))
    })
}

#[test]
fn the_outline_draws_its_four_sides_and_leaves_the_middle_empty() {
    let camera = top_camera();
    let bed = Bed {
        x_mm: 200.0,
        y_mm: 150.0,
        z_mm: None,
    };
    let lines = bed_outline(&bed, 1.0);
    let (img, _) = render_headless_with_lines(
        &FlatScene::default(),
        &camera,
        W,
        H,
        &Style::default(),
        &lines,
    )
    .expect("headless device (lavapipe) available");
    for p in [
        DVec3::new(0.0, -0.075, 0.0),
        DVec3::new(0.1, 0.0, 0.0),
        DVec3::new(0.0, 0.075, 0.0),
        DVec3::new(-0.1, 0.0, 0.0),
    ] {
        assert!(near_bed_color(&img, pixel(&camera, p)), "side at {p}");
    }
    assert!(!near_bed_color(&img, pixel(&camera, DVec3::ZERO)), "middle");
    assert!(
        !near_bed_color(&img, pixel(&camera, DVec3::new(0.0, -0.12, 0.0))),
        "outside"
    );
}
