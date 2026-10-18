// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-render: headless rendering on lavapipe. See docs/specs/m0-render.md.

use std::path::PathBuf;

use glam::DVec3;
use modeler_dae::{Aabb, Batch, FlatScene, LineBatch, Material};
use modeler_render::{AMBIENT, Style, render_headless};
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

/// Square from (-s, -s) to (s, s) at height z, normal +Z (or -Z when `down`), with UVs.
fn quad(material: Option<usize>, s: f32, z: f32, down: bool) -> Batch {
    let n = if down {
        [0.0, 0.0, -1.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    Batch {
        material,
        positions: vec![[-s, -s, z], [s, -s, z], [s, s, z], [-s, s, z]],
        normals: vec![n; 4],
        texcoords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        indices: if down {
            vec![0, 2, 1, 0, 3, 2]
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    }
}

fn scene(materials: Vec<Material>, batches: Vec<Batch>, lines: Vec<LineBatch>) -> FlatScene {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in batches
        .iter()
        .flat_map(|b| &b.positions)
        .chain(lines.iter().flat_map(|l| &l.positions))
    {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let bounds = (lo[0] <= hi[0]).then_some(Aabb { min: lo, max: hi });
    FlatScene {
        materials,
        batches,
        lines,
        bounds,
    }
}

/// Parallel top view showing 2 m vertically around the origin.
fn top_camera() -> Camera {
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

fn render(s: &FlatScene, camera: &Camera, w: u32, h: u32) -> (image::RgbaImage, Vec<String>) {
    render_headless(s, camera, w, h, &Style::default())
        .expect("headless device (lavapipe) available")
}

fn px(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    img.get_pixel(x, y).0
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

fn bg() -> [u8; 3] {
    let b = Style::default().background;
    [byte(b[0]), byte(b[1]), byte(b[2])]
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("modeler-render-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn empty_scene_is_all_background() {
    let (img, warnings) = render(&FlatScene::default(), &top_camera(), W, H);
    assert_eq!(img.dimensions(), (W, H));
    assert!(warnings.is_empty());
    for p in img.pixels() {
        assert_rgb(p.0, bg(), 1, "background");
    }
}

#[test]
fn face_on_surfaces_show_their_exact_color_from_either_side() {
    let color = [0.8, 0.4, 0.2, 1.0];
    for down in [false, true] {
        let s = scene(
            vec![material(color)],
            vec![quad(Some(0), 0.5, 0.0, down)],
            vec![],
        );
        let (img, _) = render(&s, &top_camera(), W, H);
        assert_rgb(
            px(&img, W / 2, H / 2),
            [204, 102, 51],
            2,
            if down { "back face" } else { "front face" },
        );
        assert_rgb(px(&img, 2, 2), bg(), 1, "corner stays background");
    }
}

#[test]
fn oblique_faces_are_shaded() {
    // A quad tilted 60 degrees from the view direction: |dot(n, v)| = 0.5.
    let (c, s) = (60f32.to_radians().cos(), 60f32.to_radians().sin());
    let n = [0.0, s, c];
    let b = Batch {
        material: Some(0),
        positions: vec![
            [-0.5, -0.5 * c, 0.5 * s],
            [0.5, -0.5 * c, 0.5 * s],
            [0.5, 0.5 * c, -0.5 * s],
            [-0.5, 0.5 * c, -0.5 * s],
        ],
        normals: vec![n; 4],
        texcoords: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    let shade = AMBIENT + (1.0 - AMBIENT) * 0.5;
    let want = byte(linear_to_srgb(shade));
    let lit = scene(
        vec![material([1.0, 1.0, 1.0, 1.0])],
        vec![b.clone()],
        vec![],
    );
    let (img, _) = render(&lit, &top_camera(), W, H);
    assert_rgb(px(&img, W / 2, H / 2), [want; 3], 3, "lit at 60 degrees");

    let mut flat = material([1.0, 1.0, 1.0, 1.0]);
    flat.unlit = true;
    let unlit = scene(vec![flat], vec![b], vec![]);
    let (img, _) = render(&unlit, &top_camera(), W, H);
    assert_rgb(
        px(&img, W / 2, H / 2),
        [255; 3],
        2,
        "unlit ignores the angle",
    );
}

#[test]
fn nearer_surfaces_win_regardless_of_batch_order() {
    let mats = vec![
        material([1.0, 0.0, 0.0, 1.0]),
        material([0.0, 0.0, 1.0, 1.0]),
    ];
    let red_high = quad(Some(0), 0.5, 1.0, false);
    let blue_low = quad(Some(1), 0.5, 0.0, false);
    for batches in [
        vec![red_high.clone(), blue_low.clone()],
        vec![blue_low, red_high],
    ] {
        let s = scene(mats.clone(), batches, vec![]);
        let (img, _) = render(&s, &top_camera(), W, H);
        assert_rgb(
            px(&img, W / 2, H / 2),
            [255, 0, 0],
            2,
            "red is nearer the eye",
        );
    }
}

#[test]
fn batches_without_material_use_the_default_face_color() {
    let s = scene(vec![], vec![quad(None, 0.5, 0.0, false)], vec![]);
    let (img, _) = render(&s, &top_camera(), W, H);
    let d = Style::default().default_face;
    assert_rgb(
        px(&img, W / 2, H / 2),
        [byte(d[0]), byte(d[1]), byte(d[2])],
        2,
        "default face",
    );
}

#[test]
fn textures_are_oriented_and_sampled() {
    let dir = temp_dir("texture");
    let path = dir.join("quadrants.png");
    // 16x16 image in solid 8x8 blocks, rows top to bottom: red | green, then blue | white.
    // Blocks keep linear filtering from mixing colors at the sample points.
    let img = image::RgbaImage::from_fn(16, 16, |x, y| {
        image::Rgba(match (x / 8, y / 8) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [0, 255, 0, 255],
            (0, 1) => [0, 0, 255, 255],
            _ => [255, 255, 255, 255],
        })
    });
    img.save(&path).unwrap();
    let mut m = material([1.0, 1.0, 1.0, 1.0]);
    m.texture = Some(path);
    // The quad spans the whole view, so screen quadrants map to image quadrants.
    let s = scene(vec![m], vec![quad(Some(0), 1.0, 0.0, false)], vec![]);
    let (out, warnings) = render(&s, &top_camera(), W, H);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_rgb(px(&out, W / 4, H / 4), [255, 0, 0], 3, "top-left");
    assert_rgb(px(&out, 3 * W / 4, H / 4), [0, 255, 0], 3, "top-right");
    assert_rgb(px(&out, W / 4, 3 * H / 4), [0, 0, 255], 3, "bottom-left");
    assert_rgb(
        px(&out, 3 * W / 4, 3 * H / 4),
        [255, 255, 255],
        3,
        "bottom-right",
    );
}

#[test]
fn missing_textures_fall_back_to_the_color_with_a_warning() {
    let mut m = material([0.0, 1.0, 0.0, 1.0]);
    m.texture = Some(PathBuf::from("/nonexistent/texture.png"));
    let s = scene(vec![m], vec![quad(Some(0), 0.5, 0.0, false)], vec![]);
    let (img, warnings) = render(&s, &top_camera(), W, H);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("texture.png"), "{}", warnings[0]);
    assert_rgb(px(&img, W / 2, H / 2), [0, 255, 0], 2, "fallback color");
}

#[test]
fn large_textures_load() {
    let dir = temp_dir("large");
    let path = dir.join("wide.png");
    image::RgbaImage::from_pixel(4096, 8, image::Rgba([10, 200, 30, 255]))
        .save(&path)
        .unwrap();
    let mut m = material([1.0, 1.0, 1.0, 1.0]);
    m.texture = Some(path);
    let s = scene(vec![m], vec![quad(Some(0), 0.5, 0.0, false)], vec![]);
    let (img, warnings) = render(&s, &top_camera(), W, H);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_rgb(
        px(&img, W / 2, H / 2),
        [10, 200, 30],
        3,
        "downscaled texture",
    );
}

#[test]
fn transparent_surfaces_blend_in_linear_space() {
    let mut blue = material([0.0, 0.0, 1.0, 1.0]);
    blue.opacity = 0.5;
    let mats = vec![material([1.0, 0.0, 0.0, 1.0]), blue];
    let want_half = byte(linear_to_srgb(0.5));
    // Translucent blue above opaque red.
    let s = scene(
        mats.clone(),
        vec![
            quad(Some(1), 0.5, 1.0, false),
            quad(Some(0), 0.5, 0.0, false),
        ],
        vec![],
    );
    let (img, _) = render(&s, &top_camera(), W, H);
    assert_rgb(
        px(&img, W / 2, H / 2),
        [want_half, 0, want_half],
        3,
        "50% blue over red",
    );
    // Translucent blue below opaque red is hidden.
    let s = scene(
        mats,
        vec![
            quad(Some(1), 0.5, 0.0, false),
            quad(Some(0), 0.5, 1.0, false),
        ],
        vec![],
    );
    let (img, _) = render(&s, &top_camera(), W, H);
    assert_rgb(
        px(&img, W / 2, H / 2),
        [255, 0, 0],
        2,
        "opaque red in front",
    );
}

#[test]
fn texture_alpha_makes_surfaces_see_through() {
    let dir = temp_dir("alpha");
    let path = dir.join("half.png");
    // Left half opaque green, right half fully transparent (16x8, so filtering doesn't mix
    // the halves at the sample points).
    image::RgbaImage::from_fn(16, 8, |x, _| {
        image::Rgba(if x < 8 {
            [0, 255, 0, 255]
        } else {
            [0, 0, 0, 0]
        })
    })
    .save(&path)
    .unwrap();
    let mut m = material([1.0, 1.0, 1.0, 1.0]);
    m.texture = Some(path);
    let s = scene(vec![m], vec![quad(Some(0), 1.0, 0.0, false)], vec![]);
    let (img, _) = render(&s, &top_camera(), W, H);
    assert_rgb(px(&img, W / 8, H / 2), [0, 255, 0], 3, "opaque texel");
    assert_rgb(
        px(&img, 7 * W / 8, H / 2),
        bg(),
        3,
        "transparent texel shows the background",
    );
}

#[test]
fn lines_are_drawn_in_their_color() {
    // y = -1/64 m is the center of pixel row 32 (the view is 2 m over 64 rows).
    let y = -1.0 / 64.0;
    let line = |material| LineBatch {
        material,
        positions: vec![[-0.9, y, 0.0], [0.9, y, 0.0]],
    };
    let mut edge = material([0.0, 0.0, 1.0, 1.0]);
    edge.unlit = true;
    for (mats, l, want) in [
        (vec![], line(None), [0u8, 0, 0]),
        (vec![edge], line(Some(0)), [0, 0, 255]),
    ] {
        let s = scene(mats, vec![], vec![l]);
        let (img, _) = render(&s, &top_camera(), W, H);
        let best = (H / 2 - 1..=H / 2 + 1)
            .map(|y| px(&img, W / 2, y))
            .min_by_key(|p| (0..3).map(|i| p[i].abs_diff(want[i]) as u32).sum::<u32>())
            .unwrap();
        assert_rgb(best, want, 40, "line pixel near the center row");
        assert_rgb(px(&img, W / 2, 4), bg(), 1, "away from the line");
    }
}

#[test]
fn non_square_viewports_keep_proportions() {
    let s = scene(
        vec![material([1.0, 0.0, 0.0, 1.0])],
        vec![quad(Some(0), 0.5, 0.0, false)],
        vec![],
    );
    let (img, _) = render(&s, &top_camera(), 128, 64);
    assert_eq!(img.dimensions(), (128, 64));
    // 1 m wide quad in a view 2 m tall and 4 m wide: x from 48 to 80, y from 16 to 48.
    assert_rgb(px(&img, 64, 32), [255, 0, 0], 2, "center");
    assert_rgb(px(&img, 52, 32), [255, 0, 0], 2, "inside left edge");
    assert_rgb(px(&img, 44, 32), bg(), 1, "outside left edge");
    assert_rgb(px(&img, 64, 12), bg(), 1, "above");
}

#[test]
fn perspective_views_render() {
    let cube: Vec<Batch> = (0..6)
        .map(|i| {
            let back = i % 2 == 1;
            let mut q = quad(Some(0), 0.5, if back { -0.5 } else { 0.5 }, back);
            let axis = i / 2;
            for p in q.positions.iter_mut().chain(q.normals.iter_mut()) {
                // Rotate the +Z/-Z face onto the X or Y axis.
                *p = match axis {
                    0 => *p,
                    1 => [p[2], p[0], p[1]],
                    _ => [p[1], p[2], p[0]],
                };
            }
            q
        })
        .collect();
    let s = scene(vec![material([0.3, 0.6, 0.9, 1.0])], cube, vec![]);
    let mut c = top_camera();
    c.projection = Projection::Perspective;
    c.set_standard_view(StandardView::Iso);
    c.distance = 4.0;
    let (img, _) = render(&s, &c, W, H);
    let covered = img
        .pixels()
        .filter(|p| (0..3).any(|i| p.0[i].abs_diff(bg()[i]) > 8))
        .count();
    assert!(
        covered > (W * H / 10) as usize,
        "cube covers {covered} pixels"
    );
    assert_rgb(px(&img, 1, 1), bg(), 1, "corner");
    let center = px(&img, W / 2, H / 2);
    assert!(
        center[2] > center[0],
        "cube color at the center: {center:?}"
    );
}

#[test]
fn srgb_helpers_are_consistent() {
    // Guards the expectations above.
    assert!((linear_to_srgb(srgb_to_linear(0.4)) - 0.4).abs() < 1e-6);
}
