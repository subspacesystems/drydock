// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-solid-ui: a scene with problem marks draws the marked faces and edges in the problem
//! color (`docs/specs/m7-solid-ui.md`). Headless on lavapipe like `render.rs`.

use glam::DVec3;
use modeler_dae::{Aabb, Batch, FlatScene, LineBatch, Material, SolidMarks, tint_problems};
use modeler_render::{Style, render_headless};
use modeler_view::{Camera, Projection, StandardView};

const W: u32 = 64;
const H: u32 = 64;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A light grey square from (-1, -1) to (1, 1) at z = 1, facing down, as two triangles, with
/// one line along x through its middle.
fn scene() -> FlatScene {
    FlatScene {
        materials: vec![Material {
            id: "grey".into(),
            name: "grey".into(),
            color: [0.8, 0.8, 0.8, 1.0],
            texture: None,
            opacity: 1.0,
            unlit: false,
        }],
        batches: vec![Batch {
            material: Some(0),
            positions: vec![
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [1.0, 1.0, 1.0],
                [-1.0, 1.0, 1.0],
            ],
            normals: vec![[0.0, 0.0, -1.0]; 4],
            texcoords: vec![],
            indices: vec![0, 2, 1, 0, 3, 2],
        }],
        lines: vec![LineBatch {
            material: None,
            positions: vec![[-0.9, 0.0, 0.99], [0.9, 0.0, 0.99]],
        }],
        bounds: Some(Aabb {
            min: [-1.0, -1.0, 1.0],
            max: [1.0, 1.0, 1.0],
        }),
    }
}

/// Looking straight up at the square's center from below.
fn from_below() -> Camera {
    let fov_y = 35f64.to_radians();
    let mut c = Camera {
        target: DVec3::new(0.0, 0.0, 1.0),
        distance: 1.2 / (fov_y / 2.0).tan(),
        yaw: 0.0,
        pitch: 0.0,
        fov_y,
        projection: Projection::Perspective,
    };
    c.set_standard_view(StandardView::Bottom);
    c
}

fn pixel(s: &FlatScene, x: u32, y: u32) -> [u8; 4] {
    let (img, _) = render_headless(s, &from_below(), W, H, &Style::default())
        .expect("headless device (lavapipe) available");
    img.get_pixel(x, y).0
}

fn magenta([r, g, b, _]: [u8; 4]) -> bool {
    i32::from(r) >= i32::from(g) + 40 && i32::from(b) >= i32::from(g) + 20
}

#[test]
fn an_unmarked_scene_has_no_magenta() {
    assert!(!magenta(pixel(&scene(), W / 4, H / 4)));
    assert!(!magenta(pixel(&scene(), 3 * W / 4, 3 * H / 4)));
}

#[test]
fn a_marked_face_is_drawn_in_the_problem_color() {
    // The triangle (-1,-1) (1,1) (1,-1) from below; with the camera looking up, one of the
    // two corners of the picture is in it. Mark both triangles and sample both.
    let marks = SolidMarks {
        triangles: vec![
            [v(-1.0, -1.0, 1.0), v(1.0, -1.0, 1.0), v(1.0, 1.0, 1.0)],
            [v(-1.0, -1.0, 1.0), v(1.0, 1.0, 1.0), v(-1.0, 1.0, 1.0)],
        ],
        segments: vec![],
    };
    let tinted = tint_problems(&scene(), &marks);
    assert!(magenta(pixel(&tinted, W / 4, H / 4)));
    assert!(magenta(pixel(&tinted, 3 * W / 4, 3 * H / 4)));
}

#[test]
fn only_the_marked_triangle_changes_color() {
    let marks = SolidMarks {
        triangles: vec![[v(-1.0, -1.0, 1.0), v(1.0, -1.0, 1.0), v(1.0, 1.0, 1.0)]],
        segments: vec![],
    };
    let tinted = tint_problems(&scene(), &marks);
    let corners = [
        pixel(&tinted, 3 * W / 4, H / 4),
        pixel(&tinted, W / 4, 3 * H / 4),
    ];
    assert_eq!(
        corners.iter().filter(|p| magenta(**p)).count(),
        1,
        "exactly one of the two triangles is tinted: {corners:?}"
    );
}

#[test]
fn a_marked_edge_is_drawn_in_the_problem_color() {
    let line_pixels = |s: &FlatScene| -> usize {
        let (img, _) = render_headless(s, &from_below(), W, H, &Style::default())
            .expect("headless device (lavapipe) available");
        (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .filter(|&(x, y)| magenta(img.get_pixel(x, y).0))
            .count()
    };
    assert_eq!(line_pixels(&scene()), 0);
    let marks = SolidMarks {
        triangles: vec![],
        segments: vec![[v(0.9, 0.0, 0.99), v(-0.9, 0.0, 0.99)]],
    };
    let tinted = tint_problems(&scene(), &marks);
    assert!(
        line_pixels(&tinted) >= 10,
        "the marked line should show as a run of magenta pixels"
    );
}
