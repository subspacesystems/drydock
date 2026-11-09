// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Paint Bucket pours to the left everywhere it appears: the paint
//! drop sits left of the tilted bucket body in the brand icon and in the cursor image.

use std::path::PathBuf;

fn icons_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../design/brand/icons")
}

/// Numbers of the first `d="..."` path that starts with `start`.
fn path_numbers(svg: &str, start: &str) -> Vec<f32> {
    let from = svg
        .find(&format!("d=\"{start}"))
        .unwrap_or_else(|| panic!("no path starting {start}"));
    let rest = &svg[from + 3..];
    let d = &rest[..rest.find('"').unwrap()];
    d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn check(svg: &str, what: &str) {
    // Bucket body: M x y L x y L x y L x y Z (corners in pairs).
    let body = path_numbers(svg, "M4.78");
    let xs: Vec<f32> = body.iter().step_by(2).copied().collect();
    let ys: Vec<f32> = body.iter().skip(1).step_by(2).copied().collect();
    let cx = xs.iter().sum::<f32>() / xs.len() as f32;
    // The rim is the first two corners: its left end must be the lower one (y grows down),
    // so the bucket is tilted to pour left.
    assert!(ys[0] > ys[1], "{what}: rim should tilt down to the left");
    assert!(xs[0] < xs[1], "{what}: rim corners out of order");
    // The paint drop starts at the left end of the rim and lies left of the body centre.
    let drop = path_numbers(svg, "M3.6");
    let dx = drop.iter().step_by(2).copied().fold(f32::MAX, f32::min);
    assert!(
        dx < xs[0] && dx < cx,
        "{what}: paint drop must be on the left"
    );
    assert!(drop[0] < cx - 5.0, "{what}: drop too far right");
}

#[test]
fn brand_icon_pours_left() {
    let svg = std::fs::read_to_string(icons_dir().join("paint.svg")).unwrap();
    check(&svg, "paint.svg");
}

#[test]
fn cursor_image_pours_left() {
    let svg = std::fs::read_to_string(icons_dir().join("cursors/paint.svg")).unwrap();
    check(&svg, "cursors/paint.svg");
}

#[test]
fn the_app_embeds_only_the_brand_file() {
    let src =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/icons.rs"))
            .unwrap();
    assert_eq!(src.matches("icons/paint.svg").count(), 1);
}
