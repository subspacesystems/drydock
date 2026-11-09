// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m12-cursor-art.md`: one 32x32 outlined PNG per tool
//! (Select excepted) made from `design/brand/icons/cursors/*.svg`, and a hotspot table.
//!
//! To rewrite the PNGs after the generator changes (this is how the task creates them):
//! `python3 design/brand/icons/generate_icons.py` then
//! `cargo test -p drydock --test cursor_art -- --ignored write_cursor_pngs`

use std::path::PathBuf;

use drydock::cursors::{self, CURSOR_SIZE};
use drydock::icons::TOOLS;
use drydock::input::Tool;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn svg_path(name: &str) -> PathBuf {
    crate_dir()
        .join("../../../design/brand/icons/cursors")
        .join(format!("{name}.svg"))
}

fn png_path(name: &str) -> PathBuf {
    crate_dir()
        .join("assets/cursors")
        .join(format!("{name}.png"))
}

/// The cursor SVG rendered by resvg at its own size, straight-alpha RGBA.
fn render(name: &str) -> Vec<u8> {
    let data = std::fs::read(svg_path(name)).unwrap_or_else(|e| panic!("{name}.svg: {e}"));
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("svg parses");
    assert_eq!(
        (tree.size().width(), tree.size().height()),
        (32.0, 32.0),
        "{name}"
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(CURSOR_SIZE, CURSOR_SIZE).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut out = Vec::new();
    for p in pixmap.pixels() {
        let c = p.demultiply();
        out.extend([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("png decodes")
        .to_rgba8();
    (image.width(), image.height(), image.into_raw())
}

fn with_image() -> Vec<Tool> {
    TOOLS
        .iter()
        .copied()
        .filter(|t| *t != Tool::Select)
        .collect()
}

#[test]
#[ignore = "rewrites assets/cursors/*.png from design/brand/icons/cursors/*.svg"]
fn write_cursor_pngs() {
    std::fs::create_dir_all(crate_dir().join("assets/cursors")).unwrap();
    for tool in with_image() {
        let name = cursors::file_name(tool).unwrap();
        let rgba = render(name);
        let image = image::RgbaImage::from_raw(CURSOR_SIZE, CURSOR_SIZE, rgba).unwrap();
        image
            .save_with_format(png_path(name), image::ImageFormat::Png)
            .unwrap();
    }
    let rgba = render("eyedropper");
    let image = image::RgbaImage::from_raw(CURSOR_SIZE, CURSOR_SIZE, rgba).unwrap();
    image
        .save_with_format(png_path("eyedropper"), image::ImageFormat::Png)
        .unwrap();
}

#[test]
fn select_has_no_cursor_image() {
    assert_eq!(cursors::file_name(Tool::Select), None);
    assert!(cursors::image_png(Tool::Select).is_none());
    assert_eq!(cursors::hotspot(Tool::Select), None);
}

#[test]
fn every_other_toolbar_tool_has_an_image_and_a_hotspot() {
    assert_eq!(with_image().len(), 17);
    for tool in with_image() {
        let png = cursors::image_png(tool).unwrap_or_else(|| panic!("{tool:?} has no image"));
        let (w, h, _) = decode(png);
        assert_eq!((w, h), (CURSOR_SIZE, CURSOR_SIZE), "{tool:?}");
        let (x, y) = cursors::hotspot(tool).unwrap_or_else(|| panic!("{tool:?} has no hotspot"));
        assert!(
            x < w && y < h,
            "{tool:?} hotspot ({x},{y}) outside the image"
        );
    }
}

#[test]
fn file_names_are_the_icon_names_and_distinct() {
    let mut names: Vec<&str> = with_image()
        .into_iter()
        .map(|t| cursors::file_name(t).unwrap())
        .collect();
    for expected in [
        "line", "erase", "paint", "pushpull", "followme", "arc3", "zoom",
    ] {
        assert!(names.contains(&expected), "{expected}");
    }
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 17);
    for n in names {
        assert!(svg_path(n).exists(), "{n}.svg missing");
        assert!(png_path(n).exists(), "{n}.png missing");
    }
}

#[test]
fn hotspots_follow_the_milestone_table() {
    let table = [
        (Tool::Line, (25, 7)),
        (Tool::Rectangle, (25, 25)),
        (Tool::Circle, (16, 16)),
        (Tool::Arc, (16, 12)),
        (Tool::Arc3, (26, 22)),
        (Tool::Offset, (22, 22)),
        (Tool::FollowMe, (25, 7)),
        (Tool::Move, (16, 16)),
        (Tool::Rotate, (16, 16)),
        (Tool::Scale, (27, 5)),
        (Tool::PushPull, (16, 22)),
        (Tool::Orbit, (16, 16)),
        (Tool::Paint, (4, 22)),
        (Tool::Tape, (4, 16)),
        (Tool::Erase, (9, 24)),
        (Tool::Pan, (16, 18)),
        (Tool::Zoom, (13, 13)),
    ];
    assert_eq!(table.len(), 17);
    for (tool, want) in table {
        assert_eq!(cursors::hotspot(tool), Some(want), "{tool:?}");
    }
    // The Eyedropper (Paint with Alt) is not a toolbar tool; it has its own image and tip.
    assert_eq!(cursors::eyedropper_hotspot(), (4, 28));
    let (w, h, _) = decode(cursors::eyedropper_png());
    assert_eq!((w, h), (CURSOR_SIZE, CURSOR_SIZE));
}

/// Every image is white ink over a dark outline: dark opaque pixels and white opaque
/// pixels both exist, every opaque-ish pixel touching transparency on the outside is
/// dark, and the image is not mostly filled.
fn check_outline(name: &str, rgba: &[u8]) {
    let px = |x: i32, y: i32| -> Option<[u8; 4]> {
        if x < 0 || y < 0 || x >= 32 || y >= 32 {
            return None;
        }
        let i = ((y * 32 + x) * 4) as usize;
        Some([rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]])
    };
    let dark = |p: [u8; 4]| p[3] >= 200 && (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3 < 80;
    let white = |p: [u8; 4]| p[3] >= 200 && p[0] >= 230 && p[1] >= 230 && p[2] >= 230;
    let all: Vec<[u8; 4]> = (0..32 * 32).map(|i| px(i % 32, i / 32).unwrap()).collect();
    let n_dark = all.iter().filter(|p| dark(**p)).count();
    let n_white = all.iter().filter(|p| white(**p)).count();
    let n_opaque = all.iter().filter(|p| p[3] > 0).count();
    assert!(n_dark >= 20, "{name}: only {n_dark} dark outline pixels");
    assert!(n_white >= 10, "{name}: only {n_white} white ink pixels");
    assert!(
        n_opaque < 32 * 32 * 9 / 10,
        "{name}: image is mostly filled"
    );
    // Outline: no white pixel sits next to transparency (or the image edge is fine);
    // the dark ring must separate white ink from the background.
    let mut bare_white = 0;
    for y in 0..32 {
        for x in 0..32 {
            if !white(px(x, y).unwrap()) {
                continue;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if let Some(q) = px(x + dx, y + dy)
                    && q[3] < 40
                {
                    bare_white += 1;
                }
            }
        }
    }
    assert!(
        bare_white * 10 <= n_white,
        "{name}: white ink without outline ({bare_white})"
    );
}

#[test]
fn images_have_a_dark_outline_around_white_ink() {
    for tool in with_image() {
        let (_, _, rgba) = decode(cursors::image_png(tool).unwrap());
        check_outline(cursors::file_name(tool).unwrap(), &rgba);
    }
    let (_, _, rgba) = decode(cursors::eyedropper_png());
    check_outline("eyedropper", &rgba);
}

#[test]
fn the_ring_centre_is_not_covered_by_ink_for_ring_tools() {
    // The point you click is the middle of the ring: the hotspot pixel is see-through or
    // outline-free of white ink for the tools whose icon carries a ring.
    for tool in [
        Tool::Line,
        Tool::Rectangle,
        Tool::Circle,
        Tool::Arc,
        Tool::Arc3,
        Tool::Offset,
        Tool::FollowMe,
        Tool::Move,
        Tool::Rotate,
        Tool::Scale,
    ] {
        let (_, _, rgba) = decode(cursors::image_png(tool).unwrap());
        let (x, y) = cursors::hotspot(tool).unwrap();
        let i = ((y * 32 + x) * 4) as usize;
        let white = rgba[i + 3] >= 200 && rgba[i] >= 230;
        assert!(!white, "{tool:?}: white ink on the hotspot pixel");
    }
}

#[test]
fn committed_pngs_match_a_fresh_render_of_the_generated_svgs() {
    for tool in with_image() {
        let name = cursors::file_name(tool).unwrap();
        let (_, _, committed) = decode(&std::fs::read(png_path(name)).unwrap());
        assert_eq!(
            committed,
            render(name),
            "{name}.png differs from {name}.svg"
        );
        let (_, _, embedded) = decode(cursors::image_png(tool).unwrap());
        assert_eq!(
            embedded, committed,
            "{name}: embedded bytes differ from the file"
        );
    }
    let (_, _, committed) = decode(&std::fs::read(png_path("eyedropper")).unwrap());
    assert_eq!(committed, render("eyedropper"));
}

#[test]
fn the_paint_bucket_pours_to_the_left() {
    // Mirrored left-to-right: the drip hangs at the left, so the hotspot (the
    // bottom of the drip) is in the left third of the image.
    let (x, _) = cursors::hotspot(Tool::Paint).unwrap();
    assert!(x < 11);
}

#[test]
fn eyedropper_is_included_in_the_generated_set() {
    assert!(svg_path("eyedropper").exists());
    assert!(!svg_path("select").exists());
}
