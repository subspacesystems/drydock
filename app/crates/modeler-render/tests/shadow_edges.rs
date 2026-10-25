// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-shadow-cascades: shadow edges stay sharp and in place up close
//! (`docs/specs/m5-shadow-cascades.md`). Headless on lavapipe like `sun_shadows.rs`.
//!
//! Each test renders a big site, works out for every pixel whether the ground point it
//! shows is in shadow (exact geometry), and checks every pixel more than a few pixels from
//! a shadow edge against that.

use glam::{DMat4, DVec3, DVec4};
use modeler_dae::{Aabb, Batch, FlatScene, Material};
use modeler_render::{Style, SunLight, render_headless};
use modeler_view::{Camera, Projection, StandardView};

const LIGHT: f32 = 0.6;
const DARK: f32 = 0.3;
const FOV: f64 = 0.610_865_238_198_015_2; // 35 degrees

fn srgb_byte(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// The site is turned this far about +Z so edges run across the shadow map's texel grid.
fn turn() -> f64 {
    30f64.to_radians()
}

/// Site axes (x across the edge, y along it, z up) to world.
fn world(x: f64, y: f64, z: f64) -> DVec3 {
    let (s, c) = turn().sin_cos();
    DVec3::new(c * x - s * y, s * x + c * y, z)
}

/// World to site axes.
fn site(p: DVec3) -> DVec3 {
    let (s, c) = turn().sin_cos();
    DVec3::new(c * p.x + s * p.y, -s * p.x + c * p.y, p.z)
}

fn quad(corners: [DVec3; 4], normal: DVec3) -> Batch {
    let f = |p: DVec3| [p.x as f32, p.y as f32, p.z as f32];
    Batch {
        material: Some(0),
        positions: corners.map(f).to_vec(),
        normals: vec![f(normal); 4],
        texcoords: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// Horizontal rectangle in site axes, normal +Z.
fn flat(x0: f64, x1: f64, y0: f64, y1: f64, z: f64) -> Batch {
    quad(
        [
            world(x0, y0, z),
            world(x1, y0, z),
            world(x1, y1, z),
            world(x0, y1, z),
        ],
        DVec3::Z,
    )
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
            color: [1.0, 1.0, 1.0, 1.0],
            texture: None,
            opacity: 1.0,
            unlit: false,
        }],
        batches,
        lines: vec![],
        bounds: Some(Aabb { min: lo, max: hi }),
    }
}

fn style(sun: DVec3) -> Style {
    Style {
        sun: Some(SunLight {
            direction: sun,
            light: LIGHT,
            dark: DARK,
            shadows: true,
            on_ground: false,
        }),
        ..Style::default()
    }
}

/// The ground point (z = 0) the center of pixel (px, py) shows.
fn ground_at(cam: &Camera, w: u32, h: u32, px: u32, py: u32) -> Option<DVec3> {
    let aspect = w as f64 / h as f64;
    let inv: DMat4 = (cam.projection_matrix(aspect, None) * cam.view_matrix()).inverse();
    let nx = (px as f64 + 0.5) / w as f64 * 2.0 - 1.0;
    let ny = 1.0 - (py as f64 + 0.5) / h as f64 * 2.0;
    let un = |z: f64| {
        let v = inv * DVec4::new(nx, ny, z, 1.0);
        v.truncate() / v.w
    };
    let (a, b) = (un(0.0), un(1.0));
    let d = b - a;
    if d.z.abs() < 1e-12 {
        return None;
    }
    let t = -a.z / d.z;
    let p = a + d * t;
    // Stay on the 200 m ground every test stands on.
    let q = site(p);
    (t > 0.0 && q.x.abs() < 90.0 && q.y.abs() < 90.0).then_some(p)
}

/// Renders and compares every ground pixel more than `keep_off` pixels (in x or y) from an
/// edge of `shadowed` with the lit or dark ground color.
#[allow(clippy::too_many_arguments)]
fn check(
    what: &str,
    s: &FlatScene,
    cam: &Camera,
    w: u32,
    h: u32,
    sun: DVec3,
    keep_off: i64,
    shadowed: impl Fn(DVec3) -> bool,
) {
    let (img, _) = render_headless(s, cam, w, h, &style(sun)).expect("headless device (lavapipe)");
    let lit = srgb_byte((DARK + LIGHT * sun.normalize().z as f32).min(1.0));
    let dark = srgb_byte(DARK);
    let mut classes = vec![None; (w * h) as usize];
    for py in 0..h {
        for px in 0..w {
            classes[(py * w + px) as usize] = ground_at(cam, w, h, px, py).map(&shadowed);
        }
    }
    let mut bad = Vec::new();
    let (mut lit_seen, mut dark_seen) = (0, 0);
    for py in 0..h as i64 {
        for px in 0..w as i64 {
            let class = classes[(py * w as i64 + px) as usize];
            let Some(class) = class else { continue };
            let steady = (-keep_off..=keep_off).all(|dy| {
                (-keep_off..=keep_off).all(|dx| {
                    let (x, y) = (px + dx, py + dy);
                    x < 0
                        || y < 0
                        || x >= w as i64
                        || y >= h as i64
                        || classes[(y * w as i64 + x) as usize] == Some(class)
                })
            });
            if !steady {
                continue;
            }
            let want = if class { dark } else { lit };
            let got = img.get_pixel(px as u32, py as u32).0;
            if class {
                dark_seen += 1;
            } else {
                lit_seen += 1;
            }
            if got[..3].iter().any(|&c| c.abs_diff(want) > 6) {
                bad.push((px, py, got, if class { "shadow" } else { "sun" }));
            }
        }
    }
    assert!(
        lit_seen > 100 && dark_seen > 100,
        "{what}: test setup should show both sides ({lit_seen} lit, {dark_seen} dark)"
    );
    assert!(
        bad.is_empty(),
        "{what}: {} pixels off (lit {lit}, dark {dark}); first: {:?}",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

/// 200 m ground with a big roof 10 m up whose shadow edge runs through the origin:
/// ground with site x in 0..55 (and |y| < 60) is shaded. The roof is out of view.
fn roof_site() -> (FlatScene, DVec3, impl Fn(DVec3) -> bool) {
    let s = scene(vec![
        flat(-100.0, 100.0, -100.0, 100.0, 0.0),
        flat(5.0, 60.0, -60.0, 60.0, 10.0),
    ]);
    let sun = world(0.5, 0.0, 1.0).normalize();
    let shadowed = |p: DVec3| {
        let q = site(p);
        (0.0..=55.0).contains(&q.x) && q.y.abs() < 60.0
    };
    (s, sun, shadowed)
}

#[test]
fn edge_is_sharp_from_above_up_close() {
    let (s, sun, shadowed) = roof_site();
    let mut cam = Camera {
        target: DVec3::ZERO,
        distance: 3.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: FOV,
        projection: Projection::Parallel,
    };
    cam.set_standard_view(StandardView::Top);
    check(
        "parallel top, 3.8 m across",
        &s,
        &cam,
        192,
        192,
        sun,
        3,
        shadowed,
    );
}

#[test]
fn edge_is_sharp_in_perspective_up_close() {
    // Eye 3 m from the edge on the -y side, looking 30 degrees down along it: the frame
    // spans view depths of about 2 to 7 m, so it uses more than one cascade.
    let (s, sun, shadowed) = roof_site();
    let cam = Camera {
        target: DVec3::ZERO,
        distance: 3.0,
        yaw: turn() - 90f64.to_radians(),
        pitch: 30f64.to_radians(),
        fov_y: FOV,
        projection: Projection::Perspective,
    };
    check("perspective, 3 m", &s, &cam, 192, 192, sun, 3, shadowed);
}

#[test]
fn shadow_starts_at_the_foot_of_a_wall() {
    // A 1 m wall standing on 200 m of ground, sun 45 degrees up behind it: the ground
    // behind the wall is in shadow right up to its foot, and in front of it there is no
    // acne. Seen from straight above, 0.2 m across (about 1.6 mm per pixel).
    let s = scene(vec![
        flat(-100.0, 100.0, -100.0, 100.0, 0.0),
        quad(
            [
                world(0.0, -50.0, 0.0),
                world(0.0, 50.0, 0.0),
                world(0.0, 50.0, 1.0),
                world(0.0, -50.0, 1.0),
            ],
            world(1.0, 0.0, 0.0),
        ),
    ]);
    let sun = world(1.0, 0.0, 1.0).normalize();
    let mut cam = Camera {
        target: DVec3::ZERO,
        distance: 0.1 / (FOV / 2.0).tan(),
        yaw: 0.0,
        pitch: 0.0,
        fov_y: FOV,
        projection: Projection::Parallel,
    };
    cam.set_standard_view(StandardView::Top);
    let shadowed = |p: DVec3| {
        let q = site(p);
        (-1.0..=0.0).contains(&q.x) && q.y.abs() < 50.0
    };
    check("wall foot", &s, &cam, 128, 128, sun, 3, shadowed);
}

#[test]
fn distant_shadow_still_shows_in_a_long_view() {
    // Low perspective view across the site: the roof's shadow 40 m away is still drawn
    // (by a far cascade) and the near ground is lit.
    let (s, sun, shadowed) = roof_site();
    let cam = Camera {
        target: world(-2.0, 0.0, 0.0),
        distance: 2.0,
        yaw: turn() + 180f64.to_radians(),
        pitch: 12f64.to_radians(),
        fov_y: FOV,
        projection: Projection::Perspective,
    };
    // Far away a pixel covers a lot of ground, so stay well off the edges there.
    check("long view", &s, &cam, 160, 120, sun, 6, shadowed);
}
