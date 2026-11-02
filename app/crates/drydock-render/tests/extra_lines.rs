// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-axes-depth: extra lines are depth-tested against the scene.
//! See docs/specs/m4-axes-depth.md.

use drydock_dae::{Aabb, Batch, FlatScene, Material};
use drydock_render::{ExtraLine, Style, render_headless, render_headless_with_lines};
use drydock_view::{Camera, Projection};
use glam::{DVec2, DVec3};

const W: u32 = 96;
const H: u32 = 96;
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

fn scene() -> FlatScene {
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

fn camera(projection: Projection) -> Camera {
    let c = Camera {
        target: DVec3::ZERO,
        distance: 2.5,
        yaw: 30f64.to_radians(),
        pitch: 60f64.to_radians(),
        fov_y: 35f64.to_radians(),
        projection,
    };
    assert!(c.eye().z > 0.0, "looks down at the square");
    c
}

fn line(a: DVec3, b: DVec3) -> ExtraLine {
    ExtraLine {
        a,
        b,
        color: GREEN,
        width_px: 2.0,
        dash_px: None,
    }
}

/// Pixel of a world point.
fn pixel(c: &Camera, p: DVec3) -> (u32, u32) {
    let (ndc, _) = c
        .project(p, W as f64 / H as f64)
        .expect("point in front of the camera");
    let px = (ndc + DVec2::ONE) / 2.0 * DVec2::new(W as f64, H as f64);
    (px.x.floor() as u32, (H as f64 - px.y).floor() as u32)
}

/// Whether some pixel within one of `(x, y)` is clearly green.
fn greenish(img: &image::RgbaImage, (x, y): (u32, u32)) -> bool {
    (x.saturating_sub(1)..=(x + 1).min(W - 1)).any(|i| {
        (y.saturating_sub(1)..=(y + 1).min(H - 1)).any(|j| {
            let p = img.get_pixel(i, j).0;
            p[1] >= 120 && p[0] <= 90 && p[2] <= 90
        })
    })
}

fn render(extra: &[ExtraLine], projection: Projection) -> image::RgbaImage {
    render_headless_with_lines(
        &scene(),
        &camera(projection),
        W,
        H,
        &Style::default(),
        extra,
    )
    .expect("headless device (lavapipe) available")
    .0
}

#[test]
fn a_line_over_empty_space_shows_in_its_color() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        let c = camera(projection);
        let a = DVec3::new(-1.3, 0.2, 0.0);
        let b = DVec3::new(-0.8, 0.2, 0.0);
        let img = render(&[line(a, b)], projection);
        assert!(greenish(&img, pixel(&c, (a + b) / 2.0)), "{projection:?}");
    }
}

#[test]
fn a_line_above_a_face_hides_part_of_it() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        let c = camera(projection);
        let a = DVec3::new(-0.3, 0.0, 0.3);
        let b = DVec3::new(0.3, 0.0, 0.3);
        let img = render(&[line(a, b)], projection);
        assert!(greenish(&img, pixel(&c, (a + b) / 2.0)), "{projection:?}");
    }
}

#[test]
fn a_line_below_a_face_is_hidden_by_it() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        let c = camera(projection);
        let a = DVec3::new(-0.3, 0.0, -0.3);
        let b = DVec3::new(0.3, 0.0, -0.3);
        let img = render(&[line(a, b)], projection);
        assert!(
            !greenish(&img, pixel(&c, (a + b) / 2.0)),
            "{projection:?}: the square hides the line"
        );
    }
}

#[test]
fn a_line_poking_through_a_face_is_hidden_only_below_it() {
    let projection = Projection::Parallel;
    let c = camera(projection);
    let high = DVec3::new(0.0, 0.0, 0.6);
    let low = DVec3::new(0.2, 0.2, -0.6);
    let img = render(&[line(high, low)], projection);
    assert!(greenish(
        &img,
        pixel(&c, DVec3::new(0.0, 0.0, 0.5) * 0.9 + high * 0.1)
    ));
    assert!(
        !greenish(&img, pixel(&c, low)),
        "the part under the face is hidden"
    );
}

#[test]
fn no_extra_lines_gives_the_plain_image() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        let plain = render_headless(&scene(), &camera(projection), W, H, &Style::default())
            .expect("headless device (lavapipe) available")
            .0;
        assert_eq!(plain, render(&[], projection), "{projection:?}");
    }
}

#[test]
fn lines_with_bad_ends_are_skipped() {
    let c = camera(Projection::Parallel);
    let good = line(DVec3::new(-1.3, 0.2, 0.0), DVec3::new(-0.8, 0.2, 0.0));
    let bad = [
        line(DVec3::new(f64::NAN, 0.0, 0.0), DVec3::ONE),
        line(DVec3::ZERO, DVec3::new(f64::INFINITY, 0.0, 0.0)),
        good,
    ];
    let img = render(&bad, Projection::Parallel);
    assert!(greenish(&img, pixel(&c, (good.a + good.b) / 2.0)));
}

#[test]
fn extra_lines_do_not_move_the_view() {
    // A very long line must not change the near and far planes: the square keeps
    // its pixels.
    let projection = Projection::Perspective;
    let far = line(DVec3::new(-500.0, 5.0, 5.0), DVec3::new(500.0, 5.0, 5.0));
    let with = render(&[far], projection);
    let without = render(&[], projection);
    let c = camera(projection);
    let p = pixel(&c, DVec3::new(0.1, 0.1, 0.0));
    assert_eq!(with.get_pixel(p.0, p.1), without.get_pixel(p.0, p.1));
}
