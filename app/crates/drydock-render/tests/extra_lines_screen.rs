// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-axes-screen: extra lines keep their width and dashes in screen pixels.
//! See docs/specs/m4-axes-screen.md.

use drydock_dae::{Aabb, Batch, FlatScene, Material};
use drydock_render::{ExtraLine, Style, render_headless_with_lines};
use drydock_view::{Camera, Projection};
use glam::{DVec2, DVec3};

const W: u32 = 256;
const H: u32 = 256;
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

/// A 1 m red square at z = 0, so near and far hug it.
fn square() -> FlatScene {
    FlatScene {
        materials: vec![Material {
            id: "m".into(),
            name: "m".into(),
            color: [1.0, 0.0, 0.0, 1.0],
            texture: None,
            opacity: 1.0,
            unlit: false,
        }],
        batches: vec![Batch {
            material: Some(0),
            positions: vec![
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [0.5, 0.5, 0.0],
                [-0.5, 0.5, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            texcoords: vec![],
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
        lines: vec![],
        bounds: Some(Aabb {
            min: [-0.5, -0.5, 0.0],
            max: [0.5, 0.5, 0.0],
        }),
    }
}

/// Nothing at all: the renderer's default near and far.
fn empty() -> FlatScene {
    FlatScene {
        materials: vec![],
        batches: vec![],
        lines: vec![],
        bounds: None,
    }
}

fn camera(projection: Projection) -> Camera {
    Camera {
        target: DVec3::ZERO,
        distance: 2.5,
        yaw: 30f64.to_radians(),
        pitch: 30f64.to_radians(),
        fov_y: 35f64.to_radians(),
        projection,
    }
}

fn line(a: DVec3, b: DVec3, width_px: f32, dash_px: Option<[f32; 2]>) -> ExtraLine {
    ExtraLine {
        a,
        b,
        color: GREEN,
        width_px,
        dash_px,
    }
}

fn render(scene: &FlatScene, c: &Camera, extra: &[ExtraLine]) -> image::RgbaImage {
    render_headless_with_lines(scene, c, W, H, &Style::default(), extra)
        .expect("headless device (lavapipe) available")
        .0
}

/// Screen position (pixels, y down, pixel centers at +0.5) of a point in front.
fn screen(c: &Camera, p: DVec3) -> DVec2 {
    let (ndc, _) = c
        .project(p, W as f64 / H as f64)
        .expect("point in front of the camera");
    DVec2::new(
        (ndc.x + 1.0) / 2.0 * W as f64,
        (1.0 - ndc.y) / 2.0 * H as f64,
    )
}

/// How much pixel `(x, y)` of `img` lost in red against `base`: the background is light
/// grey and the line pure green, so a covered pixel drops by about 240.
fn red_drop(img: &image::RgbaImage, base: &image::RgbaImage, x: i64, y: i64) -> i32 {
    if x < 0 || y < 0 || x >= W as i64 || y >= H as i64 {
        return 0;
    }
    let (x, y) = (x as u32, y as u32);
    i32::from(base.get_pixel(x, y).0[0]) - i32::from(img.get_pixel(x, y).0[0])
}

/// Pixels the lines changed clearly.
fn changed(img: &image::RgbaImage, base: &image::RgbaImage) -> Vec<DVec2> {
    let mut out = Vec::new();
    for y in 0..H {
        for x in 0..W {
            let a = img.get_pixel(x, y).0;
            let b = base.get_pixel(x, y).0;
            let diff = (0..3)
                .map(|k| (i32::from(a[k]) - i32::from(b[k])).abs())
                .max()
                .unwrap();
            if diff > 40 {
                out.push(DVec2::new(x as f64 + 0.5, y as f64 + 0.5));
            }
        }
    }
    out
}

/// Distance from `p` to the segment `a`-`b`.
fn to_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + ab * t).distance(p)
}

/// The part of the screen segment `a`-`b` at least `margin` pixels inside the image.
fn inside(a: DVec2, b: DVec2, margin: f64) -> Option<(DVec2, DVec2)> {
    let (lo, hi) = (
        DVec2::splat(margin),
        DVec2::new(W as f64, H as f64) - margin,
    );
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for k in 0..2 {
        if d[k].abs() < 1e-12 {
            if a[k] < lo[k] || a[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (u, v) = ((lo[k] - a[k]) / d[k], (hi[k] - a[k]) / d[k]);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
    }
    (t1 > t0).then(|| (a + d * t0, a + d * t1))
}

/// Points every `step` pixels along the screen segment `a`-`b`.
fn samples(a: DVec2, b: DVec2, step: f64) -> Vec<DVec2> {
    let n = (a.distance(b) / step).floor() as usize;
    (0..=n)
        .map(|i| a + (b - a) * (i as f64 * step / a.distance(b)))
        .collect()
}

/// Whether the line covers the pixel under `p` well (more than about half).
fn covered(img: &image::RgbaImage, base: &image::RgbaImage, p: DVec2) -> bool {
    red_drop(img, base, p.x.floor() as i64, p.y.floor() as i64) >= 120
}

/// Whether some pixel within one of the pixel under `p` is covered well.
fn covered_near(img: &image::RgbaImage, base: &image::RgbaImage, p: DVec2) -> bool {
    let (x, y) = (p.x.floor() as i64, p.y.floor() as i64);
    (-1..=1).any(|i| (-1..=1).any(|j| red_drop(img, base, x + i, y + j) >= 120))
}

/// How many pixels the line covers across its direction at `p` (perpendicular scan).
fn thickness(img: &image::RgbaImage, base: &image::RgbaImage, p: DVec2, dir: DVec2) -> f64 {
    let n = DVec2::new(-dir.y, dir.x).normalize();
    let step = 0.25;
    let hits = (-80..=80)
        .filter(|&k| covered(img, base, p + n * (k as f64 * step)))
        .count();
    hits as f64 * step
}

/// Every changed pixel lies within `width / 2 + 1.5` of the screen segment `a`-`b`.
fn assert_hugs(img: &image::RgbaImage, base: &image::RgbaImage, a: DVec2, b: DVec2, width: f64) {
    for p in changed(img, base) {
        let d = to_segment(p, a, b);
        assert!(
            d <= width / 2.0 + 1.5,
            "pixel {p:?} is {d:.1} px from the line {a:?}-{b:?} (width {width})"
        );
    }
}

/// Lengths (pixels) of the covered and uncovered runs along `a`-`b`, sampled every 0.25 px,
/// first and last (partial) runs left out. `(on, off)`.
fn runs(
    img: &image::RgbaImage,
    base: &image::RgbaImage,
    a: DVec2,
    b: DVec2,
) -> (Vec<f64>, Vec<f64>) {
    let step = 0.25;
    let states: Vec<bool> = samples(a, b, step)
        .into_iter()
        .map(|p| covered(img, base, p))
        .collect();
    let mut all: Vec<(bool, f64)> = Vec::new();
    for s in states {
        match all.last_mut() {
            Some((state, length)) if *state == s => *length += step,
            _ => all.push((s, step)),
        }
    }
    if all.len() >= 2 {
        all.remove(0);
        all.pop();
    }
    let on = all.iter().filter(|(s, _)| *s).map(|(_, l)| *l).collect();
    let off = all.iter().filter(|(s, _)| !*s).map(|(_, l)| *l).collect();
    (on, off)
}

/// A line from the target running away from the camera along the ground, 100 camera
/// distances long, lifted a little off the square.
fn receding(c: &Camera) -> (DVec3, DVec3) {
    let (forward, _, _) = c.basis();
    let ground = DVec3::new(forward.x, forward.y, 0.0).normalize();
    let a = DVec3::new(0.0, 0.0, 0.05);
    (a, a + ground * (100.0 * c.distance))
}

#[test]
fn a_receding_line_keeps_its_width_near_and_far() {
    let c = camera(Projection::Perspective);
    let (a, b) = receding(&c);
    for width in [1.5f32, 3.0, 6.0] {
        let base = render(&empty(), &c, &[]);
        let img = render(&empty(), &c, &[line(a, b, width, None)]);
        let (sa, sb) = (screen(&c, a), screen(&c, b));
        assert_hugs(&img, &base, sa, sb, f64::from(width));
        if width >= 3.0 {
            let (p, q) = inside(sa, sb, 8.0).expect("line on screen");
            let dir = (q - p).normalize();
            for t in [0.05, 0.5, 0.95] {
                let at = p + (q - p) * t;
                let across = thickness(&img, &base, at, dir);
                assert!(
                    (across - f64::from(width)).abs() <= 1.5,
                    "width {width}: {across} px across at {at:?}"
                );
            }
        }
    }
}

#[test]
fn a_line_across_the_view_has_the_asked_width() {
    for projection in [Projection::Perspective, Projection::Parallel] {
        let c = camera(projection);
        let (_, right, _) = c.basis();
        let (a, b) = (DVec3::ZERO - right * 0.6, DVec3::ZERO + right * 0.6);
        for width in [1.0f32, 2.0, 4.0, 8.0] {
            let base = render(&empty(), &c, &[]);
            let img = render(&empty(), &c, &[line(a, b, width, None)]);
            let (sa, sb) = (screen(&c, a), screen(&c, b));
            assert_hugs(&img, &base, sa, sb, f64::from(width));
            let across = thickness(&img, &base, (sa + sb) / 2.0, (sb - sa).normalize());
            assert!(
                (across - f64::from(width)).abs() <= 1.5,
                "{projection:?} width {width}: {across} px across"
            );
        }
    }
}

#[test]
fn a_solid_line_has_no_gaps() {
    let c = camera(Projection::Perspective);
    let (a, b) = receding(&c);
    let base = render(&empty(), &c, &[]);
    let img = render(&empty(), &c, &[line(a, b, 3.0, None)]);
    let (p, q) = inside(screen(&c, a), screen(&c, b), 8.0).expect("line on screen");
    for s in samples(p, q, 1.0) {
        assert!(covered_near(&img, &base, s), "gap at {s:?}");
    }
}

#[test]
fn a_line_is_not_cut_at_the_scenes_far_distance() {
    // The square's far distance is about 3.2 m from the eye; the line runs 250 m.
    let c = camera(Projection::Perspective);
    let (a, b) = receding(&c);
    let base = render(&square(), &c, &[]);
    let img = render(&square(), &c, &[line(a, b, 3.0, None)]);
    let (sa, sb) = (screen(&c, a), screen(&c, b));
    assert_hugs(&img, &base, sa, sb, 3.0);
    let (p, q) = inside(sa, sb, 8.0).expect("line on screen");
    for s in samples(p, q, 1.0) {
        assert!(covered_near(&img, &base, s), "gap at {s:?}");
    }
}

#[test]
fn a_line_past_the_far_distance_is_behind_the_scene() {
    // From under the square to far beyond it, 5 cm below the square's plane: the square
    // hides the part under it; the part past the scene's far distance still shows.
    let c = camera(Projection::Perspective);
    let (forward, _, _) = c.basis();
    let ground = DVec3::new(forward.x, forward.y, 0.0).normalize();
    let a = DVec3::new(0.0, 0.0, -0.05);
    let b = a + ground * 250.0;
    let base = render(&square(), &c, &[]);
    let img = render(&square(), &c, &[line(a, b, 3.0, None)]);
    let under = screen(&c, a + ground * 0.2);
    assert!(
        !covered_near(&img, &base, under),
        "the square hides the line under it"
    );
    // 2.5 m past the square's middle: about 4.7 m deep (the far distance is about 3.2 m),
    // above the square on screen and inside the image.
    let past = a + ground * 2.5;
    let (eye_forward, _, _) = c.basis();
    assert!((past - c.eye()).dot(eye_forward) > 4.0);
    let far = screen(&c, past);
    assert!(far.y > 8.0 && far.y < H as f64 / 2.0, "{far:?}");
    assert!(
        covered_near(&img, &base, far),
        "the line shows past the far distance where nothing hides it"
    );
}

#[test]
fn a_line_running_past_the_camera_draws_only_its_front_part() {
    for scene in [square(), empty()] {
        let c = camera(Projection::Perspective);
        let (forward, right, _) = c.basis();
        let eye = c.eye();
        let a = DVec3::new(0.0, 0.0, 0.05);
        // Ends 5 m behind the eye, 0.4 m to its right: the line passes beside the camera.
        let b = eye - forward * 5.0 + right * 0.4;
        let base = render(&scene, &c, &[]);
        let img = render(&scene, &c, &[line(a, b, 3.0, None)]);
        let sa = screen(&c, a);
        // Where the line leaves the image: its screen direction from `a`.
        let ahead = screen(&c, a + (b - a) * 1e-3);
        let dir = (ahead - sa).normalize();
        let far = sa + dir * (2.0 * W as f64);
        assert_hugs(&img, &base, sa, far, 3.0);
        let (p, q) = inside(sa, far, 8.0).expect("line on screen");
        for s in samples(p, q, 1.0) {
            assert!(covered_near(&img, &base, s), "gap at {s:?}");
        }
    }
}

#[test]
fn dashes_are_measured_in_screen_pixels_from_a() {
    let c = camera(Projection::Perspective);
    let (_, right, _) = c.basis();
    let (a, b) = (DVec3::ZERO - right * 0.6, DVec3::ZERO + right * 0.6);
    let base = render(&empty(), &c, &[]);
    let img = render(&empty(), &c, &[line(a, b, 3.0, Some([8.0, 6.0]))]);
    let (sa, sb) = (screen(&c, a), screen(&c, b));
    assert_hugs(&img, &base, sa, sb, 3.0);
    let dir = (sb - sa).normalize();
    // The first dash starts at `a`, the first gap after 8 px.
    assert!(covered_near(&img, &base, sa + dir * 3.0), "first dash");
    assert!(!covered(&img, &base, sa + dir * 11.0), "first gap");
    assert!(covered(&img, &base, sa + dir * 17.0), "second dash");
    let (on, off) = runs(&img, &base, sa, sb);
    assert!(on.len() >= 5 && off.len() >= 5, "{on:?} {off:?}");
    for l in &on {
        assert!((l - 8.0).abs() <= 2.0, "dash {l} px in {on:?}");
    }
    for l in &off {
        assert!((l - 6.0).abs() <= 2.0, "gap {l} px in {off:?}");
    }
}

#[test]
fn dashes_keep_their_screen_length_on_a_receding_line() {
    let c = camera(Projection::Perspective);
    let (a, b) = receding(&c);
    let base = render(&empty(), &c, &[]);
    let img = render(&empty(), &c, &[line(a, b, 3.0, Some([8.0, 6.0]))]);
    let (sa, sb) = (screen(&c, a), screen(&c, b));
    assert_hugs(&img, &base, sa, sb, 3.0);
    let (p, q) = inside(sa, sb, 8.0).expect("line on screen");
    let (on, off) = runs(&img, &base, p, q);
    assert!(on.len() >= 4 && off.len() >= 4, "{on:?} {off:?}");
    for l in &on {
        assert!((l - 8.0).abs() <= 2.0, "dash {l} px in {on:?}");
    }
    for l in &off {
        assert!((l - 6.0).abs() <= 2.0, "gap {l} px in {off:?}");
    }
}

#[test]
fn a_bad_width_or_dash_draws_nothing_odd() {
    // Non-finite or non-positive widths draw nothing; a dash with a non-positive or
    // non-finite length draws the line solid.
    let c = camera(Projection::Perspective);
    let (_, right, _) = c.basis();
    let (a, b) = (DVec3::ZERO - right * 0.6, DVec3::ZERO + right * 0.6);
    let base = render(&empty(), &c, &[]);
    for width in [0.0f32, -2.0, f32::NAN, f32::INFINITY] {
        let img = render(&empty(), &c, &[line(a, b, width, None)]);
        assert!(changed(&img, &base).is_empty(), "width {width}");
    }
    let (sa, sb) = (screen(&c, a), screen(&c, b));
    for dash in [[0.0, 6.0], [8.0, 0.0], [f32::NAN, 6.0], [8.0, -1.0]] {
        let img = render(&empty(), &c, &[line(a, b, 3.0, Some(dash))]);
        assert_hugs(&img, &base, sa, sb, 3.0);
        let (p, q) = inside(sa, sb, 4.0).expect("line on screen");
        for s in samples(p, q, 1.0) {
            assert!(covered_near(&img, &base, s), "dash {dash:?}: gap at {s:?}");
        }
    }
}
