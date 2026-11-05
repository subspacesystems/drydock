// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app icon files come from the brand logo (`design/brand/logo`), rendered with resvg:
//! `assets/drydock-256.png`, `assets/drydock.ico` (frames 16, 32, 48, 256) and the Linux
//! hicolor icons. The committed files must match a fresh render.
//!
//! To rewrite the files after the logo changes (this is how the task creates them):
//! `cargo test -p drydock --test app_icon -- --ignored write_icon_files`
//!
//! Behaviour: `docs/specs/m10-brand-app-icon.md`.

use std::path::{Path, PathBuf};

use drydock::desktop_entry::{self, ICON_SIZES};
use drydock::window_icon::{self, ICON_PNG};

const FRAMES: [(u32, bool); 4] = [(16, true), (32, true), (48, false), (256, false)];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn logo(name: &str) -> Vec<u8> {
    let path = crate_dir().join("../../../design/brand/logo").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `(rgba straight alpha, size)`: the logo rendered to a `size` x `size` image.
fn render(svg: &[u8], size: u32) -> Vec<u8> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).expect("logo svg parses");
    assert_eq!(tree.size().width(), 1000.0);
    assert_eq!(tree.size().height(), 1000.0);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let scale = size as f32 / 1000.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

/// The source for a frame: the favicon for 16 and 32 px, the app icon above that.
fn frame_svg(small: bool) -> Vec<u8> {
    logo(if small {
        "drydock-favicon.svg"
    } else {
        "drydock-app-icon.svg"
    })
}

fn encode_png(rgba: &[u8], size: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_raw(size, size, rgba.to_vec()).unwrap();
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("png decodes")
        .to_rgba8();
    (image.width(), image.height(), image.into_raw())
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

fn mean_diff(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(x.abs_diff(*y)))
        .sum::<f64>()
        / a.len() as f64
}

/// The PNG frames of an .ico: `(width, height, png bytes)` in file order.
fn ico_frames(data: &[u8]) -> Vec<(u32, u32, Vec<u8>)> {
    assert_eq!(&data[0..4], &[0, 0, 1, 0], "ico header: reserved 0, type 1");
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    (0..count)
        .map(|i| {
            let e = &data[6 + 16 * i..22 + 16 * i];
            // 0 in the width or height byte means 256.
            let w = if e[0] == 0 { 256 } else { u32::from(e[0]) };
            let h = if e[1] == 0 { 256 } else { u32::from(e[1]) };
            let len = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
            let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
            (w, h, data[off..off + len].to_vec())
        })
        .collect()
}

fn build_ico() -> Vec<u8> {
    let frames: Vec<(u32, Vec<u8>)> = FRAMES
        .iter()
        .map(|&(size, small)| (size, encode_png(&render(&frame_svg(small), size), size)))
        .collect();
    let mut out = vec![0, 0, 1, 0];
    out.extend((frames.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * frames.len();
    for (size, png) in &frames {
        let byte = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([byte, byte, 0, 0]);
        out.extend(1u16.to_le_bytes()); // planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((png.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in &frames {
        out.extend(png);
    }
    out
}

#[test]
#[ignore = "rewrites assets/drydock-256.png and assets/drydock.ico from design/brand/logo"]
fn write_icon_files() {
    let assets = crate_dir().join("assets");
    let rgba = render(&logo("drydock-app-icon.svg"), 256);
    std::fs::write(assets.join("drydock-256.png"), encode_png(&rgba, 256)).unwrap();
    std::fs::write(assets.join("drydock.ico"), build_ico()).unwrap();
}

#[test]
fn png_is_a_fresh_render_of_the_app_icon() {
    let (w, h, rgba) = decode(&std::fs::read(crate_dir().join("assets/drydock-256.png")).unwrap());
    assert_eq!((w, h), (256, 256));
    let fresh = render(&logo("drydock-app-icon.svg"), 256);
    assert!(
        max_diff(&rgba, &fresh) <= 2,
        "drydock-256.png differs from a fresh render"
    );
}

#[test]
fn png_has_transparent_rounded_corners_and_an_opaque_centre() {
    let (_, _, rgba) = decode(ICON_PNG);
    let alpha = |x: usize, y: usize| rgba[(y * 256 + x) * 4 + 3];
    assert_eq!(alpha(0, 0), 0);
    assert_eq!(alpha(255, 255), 0);
    assert_eq!(alpha(128, 2), 255, "the tile's top edge is opaque");
    assert_eq!(alpha(128, 128), 255);
}

#[test]
fn embedded_png_is_the_asset_and_the_window_icon_is_its_pixels() {
    assert_eq!(
        ICON_PNG,
        std::fs::read(crate_dir().join("assets/drydock-256.png")).unwrap()
    );
    let icon = window_icon::window_icon();
    let (w, h, rgba) = decode(ICON_PNG);
    assert_eq!((icon.width, icon.height), (w, h));
    assert_eq!(icon.rgba, rgba);
}

#[test]
fn ico_has_the_four_frames_in_order_each_a_fresh_render() {
    let data = std::fs::read(crate_dir().join("assets/drydock.ico")).unwrap();
    let frames = ico_frames(&data);
    assert_eq!(frames.len(), 4);
    for ((w, h, png), (size, small)) in frames.iter().zip(FRAMES) {
        assert_eq!((*w, *h), (size, size), "directory entry");
        let (pw, ph, rgba) = decode(png);
        assert_eq!((pw, ph), (size, size), "image size inside the frame");
        let fresh = render(&frame_svg(small), size);
        assert!(
            max_diff(&rgba, &fresh) <= 2,
            "{size} px frame differs from a fresh render of the {} logo",
            if small { "favicon" } else { "app icon" }
        );
    }
}

#[test]
fn linux_hicolor_icons_are_the_app_icon_at_each_size() {
    let dir = Path::new("/data");
    let files = desktop_entry::files_for(dir, Path::new("/x/drydock"));
    for size in ICON_SIZES {
        let suffix = format!(
            "icons/hicolor/{size}x{size}/apps/{}.png",
            window_icon::APP_ID
        );
        let file = files
            .iter()
            .find(|f| f.path.ends_with(&suffix))
            .unwrap_or_else(|| panic!("no hicolor file for {size}"));
        let (w, h, rgba) = decode(&file.bytes);
        assert_eq!((w, h), (size, size));
        if size == 256 {
            assert_eq!(file.bytes, ICON_PNG);
        }
        let fresh = render(&logo("drydock-app-icon.svg"), size);
        assert!(
            mean_diff(&rgba, &fresh) < 4.0,
            "{size} px hicolor icon is not the app icon"
        );
    }
}

#[test]
fn the_old_generator_is_gone_and_the_readme_names_the_logo() {
    let assets = crate_dir().join("assets");
    assert!(
        !assets.join("make-icon.py").exists(),
        "make-icon.py is replaced by the test"
    );
    let readme = std::fs::read_to_string(assets.join("README.md")).unwrap();
    assert!(readme.contains("design/brand/logo"));
    assert!(readme.contains("write_icon_files"));
    assert!(!readme.contains("make-icon"));
    assert!(!readme.contains("isometric cube on a navy tile"));
}
