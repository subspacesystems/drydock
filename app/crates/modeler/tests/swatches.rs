// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-material-swatch-texture: texture previews, loaded off the UI thread and kept by path.
//! See docs/specs/m4-material-swatch-texture.md.

use std::path::PathBuf;

use eframe::egui;
use image::{Rgba, RgbaImage};
use modeler::materials_panel::MaterialsPanel;
use modeler::swatches::{SWATCH_MAX_SIDE, SwatchState, Swatches, cover_uv, thumbnail};

fn temp_dir(test: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("modeler-swatch-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

/// Write a `width` x `height` image to `dir/name` (format from the extension).
fn write_image(dir: &std::path::Path, name: &str, width: u32, height: u32) -> String {
    let path = dir.join(name);
    let image = solid(width, height, [200, 60, 30, 255]);
    if name.ends_with(".jpg") {
        image::DynamicImage::ImageRgba8(image)
            .to_rgb8()
            .save(&path)
            .unwrap();
    } else {
        image.save(&path).unwrap();
    }
    path.to_string_lossy().into_owned()
}

fn close(a: egui::Rect, b: egui::Rect) -> bool {
    (a.min - b.min).length() < 1e-5 && (a.max - b.max).length() < 1e-5
}

fn uv(x0: f32, y0: f32, x1: f32, y1: f32) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
}

// ---------------------------------------------------------------------------------------
// thumbnail

#[test]
fn the_max_side_is_64() {
    assert_eq!(SWATCH_MAX_SIDE, 64);
}

#[test]
fn large_images_shrink_to_fit_keeping_their_shape() {
    assert_eq!(
        thumbnail(solid(256, 128, [0; 4]), 64).dimensions(),
        (64, 32)
    );
    assert_eq!(
        thumbnail(solid(128, 256, [0; 4]), 64).dimensions(),
        (32, 64)
    );
    assert_eq!(
        thumbnail(solid(300, 300, [0; 4]), 64).dimensions(),
        (64, 64)
    );
    assert_eq!(thumbnail(solid(100, 30, [0; 4]), 50).dimensions(), (50, 15));
}

#[test]
fn a_thin_image_keeps_at_least_one_pixel() {
    assert_eq!(thumbnail(solid(1000, 3, [0; 4]), 64).dimensions(), (64, 1));
    assert_eq!(thumbnail(solid(2, 900, [0; 4]), 64).dimensions(), (1, 64));
}

#[test]
fn an_image_that_fits_is_unchanged() {
    let mut image = solid(40, 20, [10, 20, 30, 255]);
    image.put_pixel(3, 4, Rgba([255, 0, 0, 128]));
    assert_eq!(thumbnail(image.clone(), 64), image);
    let square = solid(64, 64, [1, 2, 3, 4]);
    assert_eq!(thumbnail(square.clone(), 64), square);
}

#[test]
fn a_solid_color_stays_that_color() {
    let small = thumbnail(solid(512, 256, [200, 60, 30, 255]), 64);
    for pixel in small.pixels() {
        for (got, want) in pixel.0.iter().zip([200u8, 60, 30, 255]) {
            assert!(got.abs_diff(want) <= 1, "{pixel:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// cover_uv

#[test]
fn matching_shapes_use_the_whole_image() {
    assert!(close(
        cover_uv([64, 64], egui::vec2(14.0, 14.0)),
        uv(0.0, 0.0, 1.0, 1.0)
    ));
    assert!(close(
        cover_uv([128, 80], egui::vec2(64.0, 40.0)),
        uv(0.0, 0.0, 1.0, 1.0)
    ));
}

#[test]
fn a_taller_image_is_cropped_top_and_bottom() {
    // Square image in a 64 x 40 tile: 40/64 of its height shows, centered.
    assert!(close(
        cover_uv([64, 64], egui::vec2(64.0, 40.0)),
        uv(0.0, 0.1875, 1.0, 0.8125)
    ));
}

#[test]
fn a_wider_image_is_cropped_left_and_right() {
    assert!(close(
        cover_uv([200, 50], egui::vec2(14.0, 14.0)),
        uv(0.375, 0.0, 0.625, 1.0)
    ));
}

#[test]
fn degenerate_sizes_use_the_whole_image() {
    for (image, swatch) in [
        ([0, 10], egui::vec2(14.0, 14.0)),
        ([10, 0], egui::vec2(14.0, 14.0)),
        ([10, 10], egui::vec2(0.0, 14.0)),
        ([10, 10], egui::vec2(14.0, 0.0)),
        ([10, 10], egui::vec2(f32::NAN, 14.0)),
    ] {
        assert!(
            close(cover_uv(image, swatch), uv(0.0, 0.0, 1.0, 1.0)),
            "{image:?} {swatch:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Swatches

#[test]
fn a_path_never_asked_for_is_unknown() {
    let mut swatches = Swatches::default();
    assert_eq!(swatches.state("/nowhere/brick.png"), SwatchState::Unknown);
    swatches.wait();
    assert_eq!(swatches.state("/nowhere/brick.png"), SwatchState::Unknown);
}

#[test]
fn an_image_loads_into_a_preview() {
    let dir = temp_dir("loads");
    let path = write_image(&dir, "brick.png", 256, 128);
    let ctx = egui::Context::default();
    let mut swatches = Swatches::default();
    let first = swatches.get(&ctx, &path);
    if first.is_none() {
        assert_eq!(swatches.state(&path), SwatchState::Loading);
    }
    swatches.wait();
    assert_eq!(swatches.state(&path), SwatchState::Ready { size: [64, 32] });
    let id = swatches.get(&ctx, &path).expect("ready after wait");
    assert_eq!(
        swatches.get(&ctx, &path),
        Some(id),
        "the same texture every frame"
    );
    assert_ne!(id, egui::TextureId::default());
}

#[test]
fn jpeg_images_load_too() {
    let dir = temp_dir("jpeg");
    let path = write_image(&dir, "oak.jpg", 32, 48);
    let ctx = egui::Context::default();
    let mut swatches = Swatches::default();
    swatches.get(&ctx, &path);
    swatches.wait();
    assert_eq!(swatches.state(&path), SwatchState::Ready { size: [32, 48] });
    assert!(swatches.get(&ctx, &path).is_some());
}

#[test]
fn a_missing_or_broken_file_fails_and_stays_failed() {
    let dir = temp_dir("broken");
    let missing = dir.join("missing.png").to_string_lossy().into_owned();
    let broken = dir.join("broken.png");
    std::fs::write(&broken, b"not an image").unwrap();
    let broken = broken.to_string_lossy().into_owned();
    let ctx = egui::Context::default();
    let mut swatches = Swatches::default();
    for path in [&missing, &broken] {
        swatches.get(&ctx, path);
    }
    swatches.wait();
    for path in [&missing, &broken] {
        assert!(
            matches!(swatches.state(path), SwatchState::Failed(_)),
            "{path}: {:?}",
            swatches.state(path)
        );
        assert_eq!(swatches.get(&ctx, path), None);
    }
    // Asking again doesn't start another load: the file appearing later changes nothing.
    write_image(&dir, "missing.png", 8, 8);
    assert_eq!(swatches.get(&ctx, &missing), None);
    swatches.wait();
    assert!(matches!(swatches.state(&missing), SwatchState::Failed(_)));
}

#[test]
fn paths_are_kept_apart() {
    let dir = temp_dir("apart");
    let a = write_image(&dir, "a.png", 16, 16);
    let b = write_image(&dir, "b.png", 128, 64);
    let ctx = egui::Context::default();
    let mut swatches = Swatches::default();
    swatches.get(&ctx, &a);
    swatches.get(&ctx, &b);
    swatches.wait();
    assert_eq!(swatches.state(&a), SwatchState::Ready { size: [16, 16] });
    assert_eq!(swatches.state(&b), SwatchState::Ready { size: [64, 32] });
    assert_ne!(swatches.get(&ctx, &a), swatches.get(&ctx, &b));
}

#[test]
fn clones_share_the_cache() {
    let dir = temp_dir("clones");
    let path = write_image(&dir, "tile.png", 20, 10);
    let ctx = egui::Context::default();
    let mut swatches = Swatches::default();
    let mut other = swatches.clone();
    swatches.get(&ctx, &path);
    swatches.wait();
    assert_eq!(other.state(&path), SwatchState::Ready { size: [20, 10] });
}

#[test]
fn many_loads_finish() {
    let dir = temp_dir("many");
    let paths: Vec<String> = (0..24)
        .map(|i| write_image(&dir, &format!("t{i}.png"), 100 + i, 50))
        .collect();
    let ctx = egui::Context::default();
    let mut swatches = Swatches::default();
    for path in &paths {
        swatches.get(&ctx, path);
    }
    swatches.wait();
    for path in &paths {
        assert!(
            matches!(swatches.state(path), SwatchState::Ready { .. }),
            "{path}"
        );
    }
}

#[test]
fn the_cache_does_not_affect_panel_equality() {
    let dir = temp_dir("equality");
    let path = write_image(&dir, "brick.png", 8, 8);
    let ctx = egui::Context::default();
    let mut panel = MaterialsPanel::default();
    panel.swatches.get(&ctx, &path);
    panel.swatches.wait();
    assert_eq!(panel, MaterialsPanel::default());
}
