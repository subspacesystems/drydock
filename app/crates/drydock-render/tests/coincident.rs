// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-render-twosided: coincident front and back copies of a face. See
//! docs/specs/m0-render-twosided.md.

use drydock_dae::{Aabb, Batch, FlatScene, Material};
use drydock_render::{Style, render_headless};
use drydock_view::{Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const W: u32 = 96;
const H: u32 = 96;
const RED: usize = 0;
const BLUE: usize = 1;

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

fn materials() -> Vec<Material> {
    vec![
        material([1.0, 0.0, 0.0, 1.0]),
        material([0.0, 0.0, 1.0, 1.0]),
    ]
}

/// Square from (-0.5, -0.5) to (0.5, 0.5) at height z. `up`: wound counter-clockwise
/// seen from +Z, normal +Z; otherwise the reverse. `other_diagonal` splits the square
/// along (1, 3) instead of (0, 2), like a second copy triangulated independently.
fn quad(material: usize, z: f32, up: bool, other_diagonal: bool) -> Batch {
    let n = if up {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 0.0, -1.0]
    };
    let indices = match (up, other_diagonal) {
        (true, false) => vec![0, 1, 2, 0, 2, 3],
        (true, true) => vec![0, 1, 3, 1, 2, 3],
        (false, false) => vec![0, 2, 1, 0, 3, 2],
        (false, true) => vec![0, 3, 1, 1, 3, 2],
    };
    Batch {
        material: Some(material),
        positions: vec![
            [-0.5, -0.5, z],
            [0.5, -0.5, z],
            [0.5, 0.5, z],
            [-0.5, 0.5, z],
        ],
        normals: vec![n; 4],
        texcoords: vec![],
        indices,
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
        materials: materials(),
        batches,
        lines: vec![],
        bounds: Some(Aabb { min: lo, max: hi }),
    }
}

fn camera(projection: Projection, yaw_deg: f64, pitch_deg: f64) -> Camera {
    let fov_y = 35f64.to_radians();
    Camera {
        target: DVec3::ZERO,
        distance: 2.5,
        yaw: yaw_deg.to_radians(),
        pitch: pitch_deg.to_radians(),
        fov_y,
        projection,
    }
}

fn render(s: &FlatScene, c: &Camera) -> image::RgbaImage {
    render_headless(s, c, W, H, &Style::default())
        .expect("headless device (lavapipe) available")
        .0
}

/// Pixels of points well inside the square at height z (away from its edges).
fn inner_pixels(c: &Camera, z: f64) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for i in 0..=8 {
        for j in 0..=8 {
            let p = DVec3::new(
                -0.35 + 0.7 * i as f64 / 8.0,
                -0.35 + 0.7 * j as f64 / 8.0,
                z,
            );
            let (ndc, _) = c
                .project(p, W as f64 / H as f64)
                .expect("point in front of the camera");
            let px = (ndc + DVec2::ONE) / 2.0 * DVec2::new(W as f64, H as f64);
            let (x, y) = (px.x.floor() as u32, (H as f64 - px.y).floor() as u32);
            if x < W && y < H {
                out.push((x, y));
            }
        }
    }
    assert!(out.len() > 40, "square mostly in view");
    out
}

/// Every inner pixel shows `want` (RED or BLUE) and none of the other material.
fn assert_shows(img: &image::RgbaImage, pixels: &[(u32, u32)], want: usize, what: &str) {
    for &(x, y) in pixels {
        let p = img.get_pixel(x, y).0;
        let (main, other) = if want == RED {
            (p[0], p[2])
        } else {
            (p[2], p[0])
        };
        assert!(
            main >= 60 && other <= 8 && p[1] <= 8,
            "{what}: pixel ({x}, {y}) is {:?}, want pure {}",
            &p[..3],
            if want == RED { "red" } else { "blue" }
        );
    }
}

#[test]
fn coincident_faces_show_the_side_facing_the_viewer() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        for yaw in [-45.0, 60.0, 150.0] {
            for pitch in [25.0, 60.0, 90.0] {
                for (above, sign) in [(true, 1.0), (false, -1.0)] {
                    let c = camera(projection, yaw, sign * pitch);
                    let pixels = inner_pixels(&c, 0.0);
                    let want = if above { RED } else { BLUE };
                    for batches in [
                        vec![quad(RED, 0.0, true, false), quad(BLUE, 0.0, false, true)],
                        vec![quad(BLUE, 0.0, false, true), quad(RED, 0.0, true, false)],
                        vec![quad(RED, 0.0, true, true), quad(BLUE, 0.0, false, true)],
                    ] {
                        let img = render(&scene(batches), &c);
                        let what = format!("{projection:?} yaw {yaw} pitch {}", sign * pitch);
                        assert_shows(&img, &pixels, want, &what);
                    }
                }
            }
        }
    }
}

#[test]
fn lone_faces_are_seen_from_both_sides_at_any_angle() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        for pitch in [30.0, -30.0, 75.0, -75.0] {
            let c = camera(projection, 20.0, pitch);
            let pixels = inner_pixels(&c, 0.0);
            for up in [true, false] {
                let img = render(&scene(vec![quad(RED, 0.0, up, false)]), &c);
                assert_shows(&img, &pixels, RED, &format!("{projection:?} pitch {pitch}"));
            }
        }
    }
}

#[test]
fn a_nearby_surface_seen_from_behind_still_hides_what_is_beyond_it() {
    // Blue panel 1 cm above the red one, wound so the viewer above sees its back.
    for projection in [Projection::Parallel, Projection::Perspective] {
        for pitch in [90.0, 50.0, 25.0] {
            let mut c = camera(projection, -30.0, pitch);
            c.set_standard_view(StandardView::Top);
            c.pitch = pitch.to_radians();
            let pixels = inner_pixels(&c, 0.01);
            let img = render(
                &scene(vec![
                    quad(RED, 0.0, true, false),
                    quad(BLUE, 0.01, false, false),
                ]),
                &c,
            );
            assert_shows(
                &img,
                &pixels,
                BLUE,
                &format!("{projection:?} pitch {pitch}"),
            );
        }
    }
}

#[test]
fn headless_rendering_is_safe_from_many_threads() {
    let threads: Vec<_> = (0..8)
        .map(|i| {
            std::thread::spawn(move || {
                let c = camera(Projection::Parallel, 30.0 * i as f64, 60.0);
                let s = scene(vec![quad(RED, 0.0, true, false)]);
                for _ in 0..4 {
                    render_headless(&s, &c, 32, 32, &Style::default())
                        .expect("headless device (lavapipe) available");
                }
            })
        })
        .collect();
    for t in threads {
        t.join().expect("render thread");
    }
}
