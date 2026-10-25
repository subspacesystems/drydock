// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-render-sun: lit materials shaded by the sun (`docs/specs/m5-render-sun.md`). Headless on
//! lavapipe like `render.rs`.

use glam::DVec3;
use modeler_dae::{Aabb, Batch, FlatScene, Material};
use modeler_render::{Style, SunLight, render_headless};
use modeler_view::{Camera, Projection, StandardView};

const W: u32 = 64;
const H: u32 = 64;

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

fn byte(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn material(color: [f32; 4]) -> Material {
    Material {
        id: "m".into(),
        name: "m".into(),
        color,
        texture: None,
        opacity: 1.0,
        unlit: false,
    }
}

/// Square from (-s, -s) to (s, s) at height 0, normal +Z (or -Z when `down`).
fn quad(s: f32, down: bool) -> Batch {
    let n = if down {
        [0.0, 0.0, -1.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    Batch {
        material: Some(0),
        positions: vec![[-s, -s, 0.0], [s, -s, 0.0], [s, s, 0.0], [-s, s, 0.0]],
        normals: vec![n; 4],
        texcoords: vec![],
        indices: if down {
            vec![0, 2, 1, 0, 3, 2]
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    }
}

/// Vertical square in the plane y = 0, from x = -s..s and z = -s..s, normal -Y.
fn wall(s: f32) -> Batch {
    Batch {
        material: Some(0),
        positions: vec![[-s, 0.0, -s], [s, 0.0, -s], [s, 0.0, s], [-s, 0.0, s]],
        normals: vec![[0.0, -1.0, 0.0]; 4],
        texcoords: vec![],
        indices: vec![0, 2, 1, 0, 3, 2],
    }
}

fn scene(color: [f32; 4], unlit: bool, batches: Vec<Batch>) -> FlatScene {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in batches.iter().flat_map(|b| &b.positions) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let mut m = material(color);
    m.unlit = unlit;
    FlatScene {
        materials: vec![m],
        batches,
        lines: vec![],
        bounds: Some(Aabb { min: lo, max: hi }),
    }
}

fn camera(view: StandardView, projection: Projection) -> Camera {
    let fov_y = 35f64.to_radians();
    let mut c = Camera {
        target: DVec3::ZERO,
        distance: 1.0 / (fov_y / 2.0).tan(),
        yaw: 0.0,
        pitch: 0.0,
        fov_y,
        projection,
    };
    c.set_standard_view(view);
    c
}

fn sun(direction: DVec3, light: f32, dark: f32) -> Style {
    Style {
        sun: Some(SunLight {
            direction: direction.normalize(),
            light,
            dark,
            shadows: false,
            on_ground: false,
        }),
        ..Style::default()
    }
}

fn center(s: &FlatScene, camera: &Camera, style: &Style) -> [u8; 4] {
    let (img, _) =
        render_headless(s, camera, W, H, style).expect("headless device (lavapipe) available");
    img.get_pixel(W / 2, H / 2).0
}

fn assert_rgb(got: [u8; 4], want: [u8; 3], tol: u8, what: &str) {
    for i in 0..3 {
        assert!(
            got[i].abs_diff(want[i]) <= tol,
            "{what}: got {:?}, want {:?}",
            &got[..3],
            want
        );
    }
}

/// The sRGB byte of a white surface at linear `shade`.
fn grey(shade: f32) -> [u8; 3] {
    [byte(linear_to_srgb(shade)); 3]
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

#[test]
fn default_style_has_no_sun() {
    assert_eq!(Style::default().sun, None);
}

#[test]
fn sun_overhead_lights_a_flat_face_fully() {
    let s = scene(WHITE, false, vec![quad(0.5, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let got = center(&s, &top, &sun(DVec3::Z, 0.6, 0.4));
    assert_rgb(got, grey(1.0), 2, "dark 0.4 + light 0.6 * 1");
}

#[test]
fn shade_is_dark_plus_light_times_the_cosine() {
    let s = scene(WHITE, false, vec![quad(0.5, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    // 60 degrees from the normal: cosine 0.5.
    let d = DVec3::new(0.0, 60f64.to_radians().sin(), 60f64.to_radians().cos());
    let got = center(&s, &top, &sun(d, 0.6, 0.3));
    assert_rgb(got, grey(0.3 + 0.6 * 0.5), 3, "60 degrees");
    let got = center(&s, &top, &sun(d, 0.2, 0.1));
    assert_rgb(got, grey(0.1 + 0.2 * 0.5), 3, "dim sun");
}

#[test]
fn shading_does_not_follow_the_viewer() {
    // With the view light an oblique view darkens a face; with the sun it doesn't.
    let s = scene(WHITE, false, vec![quad(0.5, false)]);
    let style = sun(DVec3::new(0.3, -0.2, 1.0), 0.5, 0.4);
    let top = center(&s, &camera(StandardView::Top, Projection::Parallel), &style);
    let iso = center(
        &s,
        &camera(StandardView::Iso, Projection::Perspective),
        &style,
    );
    for i in 0..3 {
        assert!(top[i].abs_diff(iso[i]) <= 3, "top {top:?} iso {iso:?}");
    }
    let d = DVec3::new(0.3, -0.2, 1.0).normalize();
    assert_rgb(top, grey(0.4 + 0.5 * d.z as f32), 3, "cosine to +Z");
}

#[test]
fn faces_are_lit_on_the_side_the_viewer_sees() {
    // A face whose stored normal points down, seen from above, is lit like one facing up.
    let s = scene(WHITE, false, vec![quad(0.5, true)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let got = center(&s, &top, &sun(DVec3::Z, 0.5, 0.5));
    assert_rgb(got, grey(1.0), 2, "back side seen from above");
}

#[test]
fn the_side_turned_away_from_the_sun_is_dark() {
    let s = scene(WHITE, false, vec![wall(0.5)]);
    let front = camera(StandardView::Front, Projection::Parallel);
    // The sun high on the far side of the wall from the camera.
    let away = (front.target - front.eye()).with_z(0.0).normalize();
    let d = away + DVec3::new(0.0, 0.0, 1.0);
    let got = center(&s, &front, &sun(d, 0.6, 0.25));
    assert_rgb(got, grey(0.25), 2, "far side lit, near side dark");
    // On the camera's side it is lit: cosine 1/sqrt(2).
    let toward = -away + DVec3::new(0.0, 0.0, 1.0);
    let got = center(&s, &front, &sun(toward, 0.6, 0.25));
    assert_rgb(
        got,
        grey(0.25 + 0.6 * std::f32::consts::FRAC_1_SQRT_2),
        3,
        "near side lit",
    );
}

#[test]
fn no_direct_light_when_the_sun_is_down() {
    let s = scene(WHITE, false, vec![quad(0.5, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let got = center(&s, &top, &sun(-DVec3::Z, 0.8, 0.2));
    assert_rgb(got, grey(0.2), 2, "sun straight below");
    // A wall facing a sun just under the horizon gets no light either.
    let w = scene(WHITE, false, vec![wall(0.5)]);
    let front = camera(StandardView::Front, Projection::Parallel);
    let toward = (front.eye() - front.target).with_z(0.0).normalize();
    let got = center(
        &w,
        &front,
        &sun(toward + DVec3::new(0.0, 0.0, -0.05), 0.8, 0.2),
    );
    assert_rgb(got, grey(0.2), 2, "wall facing a set sun");
}

#[test]
fn shade_never_exceeds_the_material_color() {
    let s = scene(WHITE, false, vec![quad(0.5, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let got = center(&s, &top, &sun(DVec3::Z, 0.9, 0.9));
    assert_rgb(got, grey(1.0), 1, "clamped at full color");
}

#[test]
fn shade_scales_the_material_color() {
    let color = [0.8, 0.4, 0.2, 1.0];
    let s = scene(color, false, vec![quad(0.5, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let d = DVec3::new(0.0, 60f64.to_radians().sin(), 60f64.to_radians().cos());
    let got = center(&s, &top, &sun(d, 0.6, 0.3));
    let shade = 0.3 + 0.6 * 0.5;
    let want = [
        byte(linear_to_srgb(srgb_to_linear(0.8) * shade)),
        byte(linear_to_srgb(srgb_to_linear(0.4) * shade)),
        byte(linear_to_srgb(srgb_to_linear(0.2) * shade)),
    ];
    assert_rgb(got, want, 3, "colored face");
}

#[test]
fn unlit_materials_ignore_the_sun() {
    let s = scene([0.8, 0.4, 0.2, 1.0], true, vec![quad(0.5, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let got = center(&s, &top, &sun(-DVec3::Z, 0.8, 0.1));
    assert_rgb(got, [204, 102, 51], 2, "unlit");
}

#[test]
fn background_is_unchanged() {
    let s = scene(WHITE, false, vec![quad(0.2, false)]);
    let top = camera(StandardView::Top, Projection::Parallel);
    let (img, _) = render_headless(&s, &top, W, H, &sun(DVec3::Z, 0.5, 0.2))
        .expect("headless device (lavapipe) available");
    let b = Style::default().background;
    assert_rgb(
        img.get_pixel(1, 1).0,
        [byte(b[0]), byte(b[1]), byte(b[2])],
        1,
        "background",
    );
}
