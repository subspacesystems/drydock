// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! With Show Shadows on, the ground pass leaves the sky alone when the eye is at or below
//! ground level (`docs/specs/m10-ground-pass-horizon.md`). Headless on lavapipe.

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

fn roof() -> FlatScene {
    scene(vec![slab(-1.0, 1.0, -1.0, 1.0, 2.0)])
}

fn sun() -> DVec3 {
    DVec3::new(0.5, 0.0, 1.0)
}

fn with_shadows(on: bool) -> Style {
    let mut st = style(sun(), true, true);
    st.sun.as_mut().unwrap().shadows = on;
    st
}

/// Top rows are sky; bottom rows are ground.
fn assert_sky_over_ground(img: &image::RgbaImage, what: &str) {
    for x in [W / 4, W / 2, 3 * W / 4] {
        for y in [2, H / 8, H / 4 - 2] {
            assert_close(rgb(img, x, y), SKY, 3, &format!("{what}: sky at ({x},{y})"));
        }
        for y in [3 * H / 4 + 2, 7 * H / 8, H - 3] {
            assert_close(
                rgb(img, x, y),
                GROUND,
                3,
                &format!("{what}: ground at ({x},{y})"),
            );
        }
    }
}

#[test]
fn front_view_zoomed_out_keeps_sky_above_the_horizon() {
    for distance in [5000.0, 50000.0] {
        let cam = camera(DVec3::ZERO, distance, 0.0, Projection::Perspective);
        let img = render(&roof(), &cam, &with_shadows(true));
        assert_sky_over_ground(&img, &format!("eye at z=0, distance {distance}"));
    }
}

#[test]
fn eye_just_below_the_ground_keeps_sky_on_top() {
    for distance in [5000.0, 50000.0] {
        let cam = camera(
            DVec3::new(0.0, 0.0, -1e-3 * distance),
            distance,
            0.0,
            Projection::Perspective,
        );
        let img = render(&roof(), &cam, &with_shadows(true));
        for x in [W / 4, W / 2, 3 * W / 4] {
            for y in [2, H / 8, H / 4 - 2] {
                assert_close(
                    rgb(&img, x, y),
                    SKY,
                    3,
                    &format!("distance {distance}: sky at ({x},{y})"),
                );
            }
        }
    }
}

#[test]
fn eye_under_the_ground_looking_up_matches_shadows_off() {
    let cam = camera(
        DVec3::new(0.0, 0.0, -50.0),
        12.0,
        -10.0,
        Projection::Perspective,
    );
    assert!(cam.eye().z < 0.0, "the eye must be under the ground");
    let on = render(&roof(), &cam, &with_shadows(true));
    let off = render(&roof(), &cam, &with_shadows(false));
    for y in 0..(2 * H / 5) {
        for x in (0..W).step_by(8) {
            assert_close(rgb(&on, x, y), SKY, 3, &format!("shadows on at ({x},{y})"));
            assert_close(
                rgb(&on, x, y),
                rgb(&off, x, y),
                2,
                &format!("on vs off at ({x},{y})"),
            );
        }
    }
}

#[test]
fn top_view_still_draws_the_shadowed_ground() {
    let cam = camera(
        DVec3::new(-1.0, 0.0, 0.0),
        4.0 / (35f64.to_radians() / 2.0).tan(),
        90.0,
        Projection::Parallel,
    );
    let img = render(&roof(), &cam, &with_shadows(true));
    let (x, y) = pixel_of(&cam, DVec3::new(-1.5, 0.0, 0.0));
    assert_close(
        rgb(&img, x, y),
        shadowed(GROUND, sun()),
        6,
        "shadow under the roof",
    );
    let (x, y) = pixel_of(&cam, DVec3::new(2.5, 0.0, 0.0));
    assert_close(rgb(&img, x, y), GROUND, 3, "lit ground");
}
