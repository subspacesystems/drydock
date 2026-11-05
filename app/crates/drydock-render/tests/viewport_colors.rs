// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Background, default face colours and rose back faces (`docs/specs/m10-brand-viewport.md`).

use drydock_dae::{Aabb, Batch, FlatScene, Material};
use drydock_render::{Style, render_headless};
use drydock_view::{Camera, Projection, StandardView};
use glam::DVec3;

const W: u32 = 64;
const H: u32 = 64;

/// Dark `face-front` and `face-back`, `viewport-bg` (sRGB bytes).
const FRONT: [u8; 3] = [0x3D, 0x47, 0x5E];
const BACK: [u8; 3] = [0xB0, 0x4A, 0x80];
const BG: [u8; 3] = [0x05, 0x0A, 0x19];

fn unit(c: [u8; 3]) -> [f32; 4] {
    [
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    ]
}

fn style(back: Option<[u8; 3]>) -> Style {
    Style {
        background: unit(BG),
        default_face: unit(FRONT),
        default_back: back.map(unit),
        ..Style::default()
    }
}

/// A square of half-size 0.5 at z = 0 without a material; `down`: normal -Z, wound the
/// other way, so the top view sees its back.
fn scene(down: bool, material: Option<usize>, materials: Vec<Material>) -> FlatScene {
    let n = if down { -1.0 } else { 1.0 };
    let batch = Batch {
        material,
        positions: vec![
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ],
        normals: vec![[0.0, 0.0, n]; 4],
        texcoords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        indices: if down {
            vec![0, 2, 1, 0, 3, 2]
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    };
    FlatScene {
        materials,
        batches: vec![batch],
        lines: vec![],
        bounds: Some(Aabb {
            min: [-0.5, -0.5, 0.0],
            max: [0.5, 0.5, 0.0],
        }),
    }
}

fn top_camera() -> Camera {
    let fov_y = 35f64.to_radians();
    let mut c = Camera {
        target: DVec3::ZERO,
        distance: 1.0 / (fov_y / 2.0).tan(),
        yaw: 0.0,
        pitch: 0.0,
        fov_y,
        projection: Projection::Parallel,
    };
    c.set_standard_view(StandardView::Top);
    c
}

fn render(s: &FlatScene, style: &Style) -> image::RgbaImage {
    render_headless(s, &top_camera(), W, H, style)
        .expect("headless device (lavapipe) available")
        .0
}

fn assert_rgb(img: &image::RgbaImage, x: u32, y: u32, want: [u8; 3], what: &str) {
    let got = img.get_pixel(x, y).0;
    for i in 0..3 {
        assert!(
            got[i].abs_diff(want[i]) <= 2,
            "{what}: got {:?}, want {want:?}",
            &got[..3]
        );
    }
}

#[test]
fn the_background_is_the_styles_background() {
    let img = render(&FlatScene::default(), &style(Some(BACK)));
    for (x, y) in [(0, 0), (W / 2, H / 2), (W - 1, H - 1)] {
        assert_rgb(&img, x, y, BG, "background");
    }
}

#[test]
fn a_default_face_seen_from_the_front_has_the_front_colour() {
    let img = render(&scene(false, None, vec![]), &style(Some(BACK)));
    assert_rgb(&img, W / 2, H / 2, FRONT, "front");
    assert_rgb(&img, 2, 2, BG, "corner");
}

#[test]
fn a_default_face_seen_from_behind_has_the_back_colour() {
    let img = render(&scene(true, None, vec![]), &style(Some(BACK)));
    assert_rgb(&img, W / 2, H / 2, BACK, "back");
    assert_rgb(&img, 2, 2, BG, "corner");
}

#[test]
fn without_a_back_colour_both_sides_use_the_face_colour() {
    for down in [false, true] {
        let img = render(&scene(down, None, vec![]), &style(None));
        assert_rgb(&img, W / 2, H / 2, FRONT, "same on both sides");
    }
    assert_eq!(Style::default().default_back, None);
}

#[test]
fn a_material_keeps_its_colour_on_both_sides() {
    let material = Material {
        id: "m".into(),
        name: "m".into(),
        color: [0.8, 0.4, 0.2, 1.0],
        texture: None,
        opacity: 1.0,
        unlit: false,
    };
    for down in [false, true] {
        let img = render(
            &scene(down, Some(0), vec![material.clone()]),
            &style(Some(BACK)),
        );
        assert_rgb(&img, W / 2, H / 2, [204, 102, 51], "material, either side");
    }
}
