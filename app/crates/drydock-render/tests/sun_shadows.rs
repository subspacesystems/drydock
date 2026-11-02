// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-render-shadows: shadows from the sun (`docs/specs/m5-render-shadows.md`). Headless on
//! lavapipe like `sun_light.rs`.

use drydock_dae::{Aabb, Batch, FlatScene, Material};
use drydock_render::{Style, SunLight, render_headless};
use drydock_view::{Camera, Projection, StandardView};
use glam::DVec3;

const W: u32 = 64;
const H: u32 = 64;
const LIGHT: f32 = 0.6;
const DARK: f32 = 0.3;

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

fn material(unlit: bool) -> Material {
    Material {
        id: "m".into(),
        name: "m".into(),
        color: [1.0, 1.0, 1.0, 1.0],
        texture: None,
        opacity: 1.0,
        unlit,
    }
}

/// Horizontal rectangle at height `z`, x from `x0` to `x1`, y from `-h` to `h`, normal +Z.
fn slab(x0: f32, x1: f32, h: f32, z: f32) -> Batch {
    Batch {
        material: Some(0),
        positions: vec![[x0, -h, z], [x1, -h, z], [x1, h, z], [x0, h, z]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        texcoords: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn scene(unlit: bool, batches: Vec<Batch>) -> FlatScene {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in batches.iter().flat_map(|b| &b.positions) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    FlatScene {
        materials: vec![material(unlit)],
        batches,
        lines: vec![],
        bounds: Some(Aabb { min: lo, max: hi }),
    }
}

/// A roof of half-size 0.25 at height 1 over a floor of half-size 1 at height 0.
fn roof_and_floor(unlit_floor: bool) -> FlatScene {
    let mut s = scene(
        false,
        vec![slab(-0.25, 0.25, 0.25, 1.0), slab(-1.0, 1.0, 1.0, 0.0)],
    );
    if unlit_floor {
        // Material 1 is the unlit one, used by the floor.
        s.materials.push(material(true));
        s.batches[1].material = Some(1);
    }
    s
}

fn roof_only() -> FlatScene {
    scene(false, vec![slab(-0.25, 0.25, 0.25, 1.0)])
}

/// Parallel view from above: the frame shows x and y from -1 to 1 at 32 pixels per unit.
fn top() -> Camera {
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

/// Tilted toward +x so the roof's shadow lands half a unit toward -x: on the floor it covers
/// x from -0.75 to -0.25 and y from -0.25 to 0.25.
fn sun_dir() -> DVec3 {
    DVec3::new(0.5, 0.0, 1.0).normalize()
}

fn style(shadows: bool, on_ground: bool) -> Style {
    Style {
        sun: Some(SunLight {
            direction: sun_dir(),
            light: LIGHT,
            dark: DARK,
            shadows,
            on_ground,
        }),
        ..Style::default()
    }
}

fn pixel(s: &FlatScene, style: &Style, p: DVec3) -> [u8; 4] {
    let cam = top();
    let (img, _) =
        render_headless(s, &cam, W, H, style).expect("headless device (lavapipe) available");
    let (ndc, _) = cam.project(p, 1.0).expect("point in front of the camera");
    let x = ((ndc.x * 0.5 + 0.5) * W as f64) as u32;
    let y = ((0.5 - ndc.y * 0.5) * H as f64) as u32;
    img.get_pixel(x.min(W - 1), y.min(H - 1)).0
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

fn grey(shade: f32) -> [u8; 3] {
    [byte(linear_to_srgb(shade)); 3]
}

fn lit() -> f32 {
    (DARK + LIGHT * sun_dir().z as f32).min(1.0)
}

fn on_floor(x: f64, y: f64) -> DVec3 {
    DVec3::new(x, y, 0.0)
}

#[test]
fn without_shadows_the_floor_is_lit_everywhere() {
    let s = roof_and_floor(false);
    let st = style(false, false);
    let got = pixel(&s, &st, on_floor(-0.5, 0.0));
    assert_rgb(got, grey(lit()), 3, "no shadows, under the roof's shadow");
}

#[test]
fn the_roof_shadows_the_floor() {
    let s = roof_and_floor(false);
    let st = style(true, false);
    assert_rgb(
        pixel(&s, &st, on_floor(-0.5, 0.0)),
        grey(DARK),
        3,
        "in the shadow",
    );
    assert_rgb(
        pixel(&s, &st, on_floor(-0.5, 0.15)),
        grey(DARK),
        3,
        "in the shadow, off the middle",
    );
}

#[test]
fn the_floor_beside_the_shadow_is_lit() {
    let s = roof_and_floor(false);
    let st = style(true, false);
    for (x, y, what) in [
        (0.5, 0.0, "on the sun side"),
        (-0.5, 0.6, "beside the shadow"),
        (-0.5, -0.6, "beside the shadow, other side"),
        (-0.9, 0.0, "beyond the shadow's far end"),
    ] {
        assert_rgb(pixel(&s, &st, on_floor(x, y)), grey(lit()), 3, what);
    }
}

#[test]
fn shadow_edges_are_where_the_geometry_puts_them() {
    let s = roof_and_floor(false);
    let st = style(true, false);
    // 0.1 inside and 0.1 outside each end of the shadow (3 pixels).
    assert_rgb(
        pixel(&s, &st, on_floor(-0.35, 0.0)),
        grey(DARK),
        3,
        "near end, inside",
    );
    assert_rgb(
        pixel(&s, &st, on_floor(-0.65, 0.0)),
        grey(DARK),
        3,
        "far end, inside",
    );
    assert_rgb(
        pixel(&s, &st, on_floor(-0.85, 0.0)),
        grey(lit()),
        3,
        "far end, outside",
    );
    assert_rgb(
        pixel(&s, &st, on_floor(-0.5, 0.35)),
        grey(lit()),
        3,
        "side, outside",
    );
}

#[test]
fn a_face_in_the_sun_does_not_shadow_itself() {
    let s = roof_and_floor(false);
    let st = style(true, false);
    for (x, y) in [(0.0, 0.0), (0.15, 0.1), (-0.15, -0.1)] {
        assert_rgb(
            pixel(&s, &st, DVec3::new(x, y, 1.0)),
            grey(lit()),
            3,
            "the roof",
        );
    }
    // A lone floor has no acne either.
    let alone = scene(false, vec![slab(-1.0, 1.0, 1.0, 0.0)]);
    for (x, y) in [(0.0, 0.0), (0.7, -0.4), (-0.6, 0.8)] {
        assert_rgb(
            pixel(&alone, &st, on_floor(x, y)),
            grey(lit()),
            3,
            "the floor",
        );
    }
}

#[test]
fn unlit_materials_do_not_receive_shadows() {
    let s = roof_and_floor(true);
    let st = style(true, false);
    assert_rgb(
        pixel(&s, &st, on_floor(-0.5, 0.0)),
        [255, 255, 255],
        2,
        "unlit floor under the roof's shadow",
    );
}

#[test]
fn a_sun_below_the_horizon_leaves_everything_dark() {
    let s = roof_and_floor(false);
    let mut st = style(true, true);
    st.sun.as_mut().unwrap().direction = DVec3::new(0.5, 0.0, -1.0).normalize();
    assert_rgb(pixel(&s, &st, on_floor(0.5, 0.0)), grey(DARK), 3, "floor");
    assert_rgb(
        pixel(&s, &st, DVec3::new(0.0, 0.0, 1.0)),
        grey(DARK),
        3,
        "roof",
    );
}

fn background_linear() -> [f32; 3] {
    let b = Style::default().background;
    [
        srgb_to_linear(b[0]),
        srgb_to_linear(b[1]),
        srgb_to_linear(b[2]),
    ]
}

fn background_srgb() -> [u8; 3] {
    let b = Style::default().background;
    [byte(b[0]), byte(b[1]), byte(b[2])]
}

#[test]
fn the_ground_shows_shadows_only_when_asked() {
    let s = roof_only();
    // Off: the background everywhere.
    let off = style(true, false);
    assert_rgb(
        pixel(&s, &off, on_floor(-0.5, 0.0)),
        background_srgb(),
        2,
        "on_ground false",
    );
    // On: the shadow darkens the background by dark / lit, the rest is untouched.
    let on = style(true, true);
    let f = DARK / lit();
    let want = background_linear().map(|c| byte(linear_to_srgb(c * f)));
    assert_rgb(
        pixel(&s, &on, on_floor(-0.5, 0.0)),
        want,
        3,
        "ground in the shadow",
    );
    assert_rgb(
        pixel(&s, &on, on_floor(0.5, 0.0)),
        background_srgb(),
        2,
        "ground in the sun",
    );
    assert_rgb(
        pixel(&s, &on, on_floor(-0.5, 0.6)),
        background_srgb(),
        2,
        "ground beside the shadow",
    );
}

#[test]
fn ground_shadows_need_shadows_on() {
    let s = roof_only();
    let st = style(false, true);
    assert_rgb(
        pixel(&s, &st, on_floor(-0.5, 0.0)),
        background_srgb(),
        2,
        "shadows false",
    );
}

#[test]
fn the_ground_never_covers_a_face() {
    // The floor lies on the ground plane; with on_ground its lit part stays as shaded.
    let s = roof_and_floor(false);
    let st = style(true, true);
    assert_rgb(
        pixel(&s, &st, on_floor(0.5, 0.0)),
        grey(lit()),
        3,
        "lit floor",
    );
    assert_rgb(
        pixel(&s, &st, on_floor(-0.5, 0.0)),
        grey(DARK),
        3,
        "shadowed floor",
    );
}
