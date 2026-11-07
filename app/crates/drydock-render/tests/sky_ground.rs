// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sky above the horizon, ground below it (`docs/specs/m10-sky-ground-render.md`).
//!
//! Camera pitch is positive when the eye is above the target (looking down). Every pixel
//! checked is at least 8 rows from the horizon line, so an optional soft edge does not matter.

use drydock_dae::{Aabb, Batch, FlatScene};
use drydock_render::{Style, render_headless};
use drydock_view::{Camera, Projection, StandardView};
use glam::DVec3;

const W: u32 = 64;
const H: u32 = 64;

const SKY: [u8; 3] = [0x87, 0xCE, 0xEB];
const GROUND: [u8; 3] = [0x70, 0x70, 0x70];
const BG: [u8; 3] = [0x30, 0x10, 0x50];

fn unit(c: [u8; 3]) -> [f32; 4] {
    [
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    ]
}

fn style(sky: bool, ground: bool) -> Style {
    Style {
        background: unit(BG),
        sky: sky.then(|| unit(SKY)),
        ground: ground.then(|| unit(GROUND)),
        ..Style::default()
    }
}

/// Looking along +Y (yaw -90 degrees), `pitch_deg` above the target, 35 degree field of view.
fn camera(pitch_deg: f64, target_z: f64, projection: Projection) -> Camera {
    Camera {
        target: DVec3::new(0.0, 0.0, target_z),
        distance: 5.0,
        yaw: -90f64.to_radians(),
        pitch: pitch_deg.to_radians(),
        fov_y: 35f64.to_radians(),
        projection,
    }
}

fn render(scene: &FlatScene, camera: &Camera, style: &Style) -> image::RgbaImage {
    render_headless(scene, camera, W, H, style)
        .expect("headless device (lavapipe) available")
        .0
}

fn empty() -> FlatScene {
    FlatScene::default()
}

fn assert_rgb(img: &image::RgbaImage, x: u32, y: u32, want: [u8; 3], what: &str) {
    let got = img.get_pixel(x, y).0;
    for i in 0..3 {
        assert!(
            got[i].abs_diff(want[i]) <= 2,
            "{what} at ({x},{y}): got {:?}, want {want:?}",
            &got[..3]
        );
    }
}

/// Rows `rows` hold `want` in every one of a few columns.
fn assert_rows(img: &image::RgbaImage, rows: &[u32], want: [u8; 3], what: &str) {
    for &y in rows {
        for x in [0, 17, 32, 63] {
            assert_rgb(img, x, y, want, what);
        }
    }
}

const UPPER: [u32; 3] = [0, 8, 20];
const LOWER: [u32; 3] = [44, 56, 63];

#[test]
fn a_level_camera_sees_sky_above_and_ground_below() {
    let img = render(
        &empty(),
        &camera(0.0, 0.0, Projection::Perspective),
        &style(true, true),
    );
    assert_rows(&img, &UPPER, SKY, "upper half");
    assert_rows(&img, &LOWER, GROUND, "lower half");
}

#[test]
fn the_horizon_moves_with_the_pitch() {
    // 10 degrees down: the horizon is at NDC y = tan(10)/tan(17.5) = 0.56, row ~14 of 64.
    let down = render(
        &empty(),
        &camera(10.0, 0.0, Projection::Perspective),
        &style(true, true),
    );
    assert_rows(&down, &[0, 4], SKY, "above the raised horizon");
    assert_rows(&down, &[24, 40, 63], GROUND, "below the raised horizon");
    // 10 degrees up (eye below the target): the horizon is at row ~50.
    let up = render(
        &empty(),
        &camera(-10.0, 0.0, Projection::Perspective),
        &style(true, true),
    );
    assert_rows(&up, &[0, 20, 40], SKY, "above the lowered horizon");
    assert_rows(&up, &[60, 63], GROUND, "below the lowered horizon");
}

#[test]
fn looking_straight_down_shows_only_ground() {
    let mut c = camera(0.0, 0.0, Projection::Perspective);
    c.set_standard_view(StandardView::Top);
    let img = render(&empty(), &c, &style(true, true));
    for (x, y) in [(0, 0), (63, 0), (32, 32), (0, 63), (63, 63)] {
        assert_rgb(&img, x, y, GROUND, "top view");
    }
}

#[test]
fn looking_straight_up_shows_only_sky() {
    let mut c = camera(0.0, 0.0, Projection::Perspective);
    c.set_standard_view(StandardView::Bottom);
    let img = render(&empty(), &c, &style(true, true));
    for (x, y) in [(0, 0), (63, 0), (32, 32), (0, 63), (63, 63)] {
        assert_rgb(&img, x, y, SKY, "bottom view");
    }
}

#[test]
fn from_below_the_ground_the_sky_above_stays_sky() {
    // The eye is 5 units under z = 0: the horizon is still where the view ray is level.
    let img = render(
        &empty(),
        &camera(0.0, -5.0, Projection::Perspective),
        &style(true, true),
    );
    assert_rows(&img, &UPPER, SKY, "sky from below");
    assert_rows(&img, &LOWER, GROUND, "ground from below");
}

#[test]
fn a_missing_sky_or_ground_shows_the_background() {
    let c = camera(0.0, 0.0, Projection::Perspective);
    let only_sky = render(&empty(), &c, &style(true, false));
    assert_rows(&only_sky, &UPPER, SKY, "sky only, upper");
    assert_rows(&only_sky, &LOWER, BG, "sky only, lower");
    let only_ground = render(&empty(), &c, &style(false, true));
    assert_rows(&only_ground, &UPPER, BG, "ground only, upper");
    assert_rows(&only_ground, &LOWER, GROUND, "ground only, lower");
}

#[test]
fn without_sky_and_ground_the_image_is_the_plain_background() {
    assert_eq!(Style::default().sky, None);
    assert_eq!(Style::default().ground, None);
    let c = camera(0.0, 0.0, Projection::Perspective);
    let img = render(&empty(), &c, &style(false, false));
    assert_rows(&img, &[0, 20, 32, 44, 63], BG, "background");
    let plain = render(&empty(), &c, &Style::default());
    let bg = Style::default().background;
    let want = [
        (bg[0] * 255.0).round() as u8,
        (bg[1] * 255.0).round() as u8,
        (bg[2] * 255.0).round() as u8,
    ];
    assert_rows(&plain, &[0, 32, 63], want, "default background");
}

#[test]
fn parallel_projection_uses_the_horizon_of_the_same_perspective_camera() {
    for pitch in [0.0, 10.0, -10.0] {
        let par = render(
            &empty(),
            &camera(pitch, 0.0, Projection::Parallel),
            &style(true, true),
        );
        let per = render(
            &empty(),
            &camera(pitch, 0.0, Projection::Perspective),
            &style(true, true),
        );
        for y in 0..H {
            for x in [0, 32, 63] {
                let a = par.get_pixel(x, y).0;
                let b = per.get_pixel(x, y).0;
                // Rows within 2 of the horizon may differ by rounding.
                let near_horizon = {
                    let t = (35f64 / 2.0).to_radians().tan();
                    let ndc = pitch.to_radians().tan() / t;
                    let row = (1.0 - ndc) / 2.0 * f64::from(H);
                    (f64::from(y) + 0.5 - row).abs() < 2.5
                };
                if !near_horizon {
                    assert_eq!(a, b, "pitch {pitch}, pixel ({x},{y})");
                }
            }
        }
    }
}

#[test]
fn parallel_projection_level_and_straight_down() {
    let level = render(
        &empty(),
        &camera(0.0, 0.0, Projection::Parallel),
        &style(true, true),
    );
    assert_rows(&level, &UPPER, SKY, "parallel upper");
    assert_rows(&level, &LOWER, GROUND, "parallel lower");
    let mut c = camera(0.0, 0.0, Projection::Parallel);
    c.set_standard_view(StandardView::Top);
    let top = render(&empty(), &c, &style(true, true));
    for (x, y) in [(0, 0), (63, 0), (32, 32), (0, 63), (63, 63)] {
        assert_rgb(&top, x, y, GROUND, "parallel top view");
    }
}

/// A square of half-size 0.5 in the plane y = 0, facing -Y (toward the level camera).
fn wall() -> FlatScene {
    let batch = Batch {
        material: None,
        positions: vec![
            [-0.5, 0.0, -0.5],
            [0.5, 0.0, -0.5],
            [0.5, 0.0, 0.5],
            [-0.5, 0.0, 0.5],
        ],
        normals: vec![[0.0, -1.0, 0.0]; 4],
        texcoords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        indices: vec![0, 2, 1, 0, 3, 2],
    };
    FlatScene {
        materials: vec![],
        batches: vec![batch],
        lines: vec![],
        bounds: Some(Aabb {
            min: [-0.5, 0.0, -0.5],
            max: [0.5, 0.0, 0.5],
        }),
    }
}

#[test]
fn geometry_is_drawn_over_the_sky_and_ground() {
    let c = camera(0.0, 0.0, Projection::Perspective);
    let with = render(&wall(), &c, &style(true, true));
    let without = render(&wall(), &c, &style(false, false));
    // The wall covers the middle of the image, across the horizon; its pixels are the same
    // as without sky and ground, and not sky or ground colored.
    for (x, y) in [(32, 28), (32, 36), (28, 32)] {
        assert_eq!(
            with.get_pixel(x, y),
            without.get_pixel(x, y),
            "wall ({x},{y})"
        );
        let got = with.get_pixel(x, y).0;
        assert_ne!(&got[..3], &SKY[..]);
        assert_ne!(&got[..3], &GROUND[..]);
        assert_ne!(&got[..3], &BG[..]);
    }
    assert_rgb(&with, 32, 4, SKY, "sky above the wall");
    assert_rgb(&with, 32, 60, GROUND, "ground below the wall");
}
