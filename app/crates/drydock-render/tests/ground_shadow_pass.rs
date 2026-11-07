// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The ground shadow pass shades the ground colour and intersects each pixel's ray from the
//! eye (`docs/specs/m10-ground-shadow-pass.md`). Headless on lavapipe.
//!
//! Colours are chosen so that the background, the sky and the ground differ a lot. The unlit
//! model faces are not under test; the scene is one roof (a flat slab) that casts a shadow.

use drydock_dae::{Aabb, Batch, FlatScene, Material};
use drydock_render::{Style, SunLight, render_headless};
use drydock_view::{Camera, Projection};
use glam::DVec3;

const W: u32 = 96;
const H: u32 = 96;
const LIGHT: f32 = 0.6;
const DARK: f32 = 0.3;

const SKY: [u8; 3] = [0x87, 0xCE, 0xEB];
const GROUND: [u8; 3] = [0xA0, 0xA0, 0x90];
const BG: [u8; 3] = [0x30, 0x10, 0x50];

fn unit(c: [u8; 3]) -> [f32; 4] {
    [
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        1.0,
    ]
}

fn style(sun: DVec3, sky: bool, ground: bool) -> Style {
    Style {
        background: unit(BG),
        sky: sky.then(|| unit(SKY)),
        ground: ground.then(|| unit(GROUND)),
        sun: Some(SunLight {
            direction: sun.normalize(),
            light: LIGHT,
            dark: DARK,
            shadows: true,
            on_ground: true,
        }),
        ..Style::default()
    }
}

/// Horizontal rectangle at height `z`, normal +Z, unlit white.
fn slab(x0: f32, x1: f32, y0: f32, y1: f32, z: f32) -> Batch {
    Batch {
        material: Some(0),
        positions: vec![[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        texcoords: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn scene(batches: Vec<Batch>) -> FlatScene {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in batches.iter().flat_map(|b| &b.positions) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    FlatScene {
        materials: vec![Material {
            id: "m".into(),
            name: "m".into(),
            color: [1.0; 4],
            texture: None,
            opacity: 1.0,
            unlit: false,
        }],
        batches,
        lines: vec![],
        bounds: Some(Aabb { min: lo, max: hi }),
    }
}

fn render(s: &FlatScene, cam: &Camera, st: &Style) -> image::RgbaImage {
    render_headless(s, cam, W, H, st)
        .expect("headless device (lavapipe) available")
        .0
}

/// Looking along +Y from the -Y side, `pitch_deg` above the target.
fn camera(target: DVec3, distance: f64, pitch_deg: f64, projection: Projection) -> Camera {
    Camera {
        target,
        distance,
        yaw: -90f64.to_radians(),
        pitch: pitch_deg.to_radians(),
        fov_y: 35f64.to_radians(),
        projection,
    }
}

fn pixel_of(cam: &Camera, p: DVec3) -> (u32, u32) {
    let (ndc, _) = cam.project(p, 1.0).expect("point in front of the camera");
    let x = ((ndc.x * 0.5 + 0.5) * f64::from(W)) as u32;
    let y = ((0.5 - ndc.y * 0.5) * f64::from(H)) as u32;
    (x.min(W - 1), y.min(H - 1))
}

fn rgb(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let p = img.get_pixel(x, y).0;
    [p[0], p[1], p[2]]
}

fn assert_close(got: [u8; 3], want: [u8; 3], tol: u8, what: &str) {
    for i in 0..3 {
        assert!(
            got[i].abs_diff(want[i]) <= tol,
            "{what}: got {got:?}, want {want:?}"
        );
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// The ground colour in shadow: linear colour times dark / lit, back to sRGB.
fn shadowed(c: [u8; 3], sun: DVec3) -> [u8; 3] {
    let lit = (DARK + LIGHT * sun.normalize().z as f32).min(1.0);
    let f = DARK / lit;
    c.map(|v| {
        let l = srgb_to_linear(f32::from(v) / 255.0) * f;
        (linear_to_srgb(l).clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

/// A 2 x 2 roof at height 2.
fn roof() -> FlatScene {
    scene(vec![slab(-1.0, 1.0, -1.0, 1.0, 2.0)])
}

fn sun() -> DVec3 {
    DVec3::new(0.5, 0.0, 1.0)
}

/// Shadow of the roof: x from -2 to 0 (shifted 1 toward -x for height 2), y from -1 to 1.
fn cam_above() -> Camera {
    camera(
        DVec3::new(-1.0, 0.0, 0.0),
        12.0,
        60.0,
        Projection::Perspective,
    )
}

#[test]
fn lit_ground_keeps_the_ground_colour_and_shadow_darkens_it() {
    let cam = cam_above();
    let img = render(&roof(), &cam, &style(sun(), true, true));
    let (x, y) = pixel_of(&cam, DVec3::new(2.5, 0.0, 0.0));
    assert_close(rgb(&img, x, y), GROUND, 2, "lit ground");
    let (x, y) = pixel_of(&cam, DVec3::new(-1.0, 0.0, 0.0));
    assert_close(
        rgb(&img, x, y),
        shadowed(GROUND, sun()),
        3,
        "ground in the shadow",
    );
}

#[test]
fn the_background_colour_is_nowhere_below_the_horizon() {
    let cam = camera(
        DVec3::new(-1.0, 0.0, 0.0),
        12.0,
        25.0,
        Projection::Perspective,
    );
    let img = render(&roof(), &cam, &style(sun(), true, true));
    // The horizon of a 25 degree pitch is well above the middle: rows from 60 down are ground.
    for y in 60..H {
        for x in (0..W).step_by(5) {
            let got = rgb(&img, x, y);
            let near_bg = (0..3).all(|i| got[i].abs_diff(BG[i]) <= 12);
            assert!(!near_bg, "background colour at ({x},{y}): {got:?}");
            let near_ground = (0..3).all(|i| got[i].abs_diff(GROUND[i]) <= 3)
                || (0..3).all(|i| got[i].abs_diff(shadowed(GROUND, sun())[i]) <= 3);
            assert!(
                near_ground,
                "({x},{y}) is neither lit nor shadowed ground: {got:?}"
            );
        }
    }
}

#[test]
fn with_the_ground_off_the_background_is_shaded_as_before() {
    let cam = cam_above();
    let img = render(&roof(), &cam, &style(sun(), true, false));
    let (x, y) = pixel_of(&cam, DVec3::new(2.5, 0.0, 0.0));
    assert_close(rgb(&img, x, y), BG, 2, "lit, no ground colour");
    let (x, y) = pixel_of(&cam, DVec3::new(-1.0, 0.0, 0.0));
    assert_close(
        rgb(&img, x, y),
        shadowed(BG, sun()),
        3,
        "shadowed, no ground colour",
    );
}

#[test]
fn zoomed_out_the_bottom_row_matches_the_ground_near_the_horizon() {
    // Eye 50 radii away, looking down 35 degrees. The scene is small, so every shadow falls
    // far from the pixels checked and all of them are lit ground.
    let s = roof();
    let (_, radius) = {
        let b = s.bounds.unwrap();
        let c = DVec3::new(
            f64::from(b.min[0] + b.max[0]) / 2.0,
            f64::from(b.min[1] + b.max[1]) / 2.0,
            f64::from(b.min[2] + b.max[2]) / 2.0,
        );
        let h = DVec3::new(
            f64::from(b.max[0] - b.min[0]) / 2.0,
            f64::from(b.max[1] - b.min[1]) / 2.0,
            f64::from(b.max[2] - b.min[2]) / 2.0,
        );
        (c, h.length())
    };
    let cam = camera(DVec3::ZERO, 50.0 * radius, 35.0, Projection::Perspective);
    let img = render(&s, &cam, &style(sun(), true, true));
    // The horizon of a 35 degree pitch is at NDC y = tan 35 / tan 17.5 = 2.2: off the top. The
    // row just under the top and the bottom row are both ground.
    let top = rgb(&img, 10, 3);
    let bottom = rgb(&img, 10, H - 1);
    assert_close(top, GROUND, 2, "row near the top");
    assert_close(bottom, GROUND, 2, "bottom row");
    assert_close(rgb(&img, 80, H - 1), GROUND, 2, "bottom row, right");
}

#[test]
fn ground_nearer_than_the_scene_sphere_still_gets_its_shadow() {
    // Two roofs 11 units apart in y: the bounding sphere is centred between them and its near
    // face lies beyond the front roof's shadow, which falls toward the eye. The camera looks
    // down 12 degrees, so the shadow is in front of the near plane.
    let s = scene(vec![
        slab(-0.5, 0.5, -6.0, -5.0, 3.0),
        slab(-0.5, 0.5, 5.0, 6.0, 3.0),
    ]);
    let sun = DVec3::new(0.0, 1.0, 0.7);
    // Front roof shadow: y shifted by -3 / 0.7 = -4.3, so about -10.3 to -9.3.
    let cam = camera(
        DVec3::new(0.0, 0.0, 3.0),
        30.0,
        12.0,
        Projection::Perspective,
    );
    let target = DVec3::new(0.0, -9.8, 0.0);
    let (centre, radius) = (DVec3::new(0.0, 0.0, 3.0), (0.5f64 * 0.5 + 6.0 * 6.0).sqrt());
    let (forward, _, _) = cam.basis();
    let near = (centre - cam.eye()).dot(forward) - radius;
    assert!(
        (target - cam.eye()).dot(forward) < near,
        "precondition: the shadow is nearer than the near plane"
    );
    let img = render(&s, &cam, &style(sun, true, true));
    let (x, y) = pixel_of(&cam, target);
    assert_close(
        rgb(&img, x, y),
        shadowed(GROUND, sun),
        3,
        "shadow nearer than the near plane",
    );
}

#[test]
fn parallel_views_shade_the_ground_colour_too() {
    let fov_y = 35f64.to_radians();
    let mut cam = camera(
        DVec3::new(-1.0, 0.0, 0.0),
        4.0 / (fov_y / 2.0).tan(),
        90.0,
        Projection::Parallel,
    );
    cam.yaw = 0.0;
    let img = render(&roof(), &cam, &style(sun(), true, true));
    let (x, y) = pixel_of(&cam, DVec3::new(2.5, 0.0, 0.0));
    assert_close(rgb(&img, x, y), GROUND, 2, "lit ground, parallel");
    let (x, y) = pixel_of(&cam, DVec3::new(-1.5, 0.0, 0.0));
    assert_close(
        rgb(&img, x, y),
        shadowed(GROUND, sun()),
        3,
        "shadowed ground, parallel",
    );
}
