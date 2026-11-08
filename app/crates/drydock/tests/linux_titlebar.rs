// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m9-linux-titlebar: the Linux window gets winit's GNOME-style title bar (sctk-adwaita),
//! an app id and our own icon. Pure; runs on any OS. See docs/specs/m9-linux-titlebar.md.

use std::path::PathBuf;

use drydock::window_icon::{APP_ID, ICON_PNG, window_icon};

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The `[[package]]` block of `name` in Cargo.lock, if any (all versions joined).
fn lock_blocks(lock: &str, name: &str) -> Vec<String> {
    lock.split("[[package]]")
        .filter(|block| {
            block
                .lines()
                .any(|l| l.trim() == format!("name = \"{name}\""))
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn lock_has_sctk_adwaita() {
    let lock = read("../../Cargo.lock");
    assert!(
        !lock_blocks(&lock, "sctk-adwaita").is_empty(),
        "Cargo.lock has no sctk-adwaita: the winit feature wayland-csd-adwaita is off"
    );
}

#[test]
fn the_winit_entry_depends_on_sctk_adwaita() {
    let lock = read("../../Cargo.lock");
    let winits = lock_blocks(&lock, "winit");
    assert_eq!(
        winits.len(),
        1,
        "exactly one winit in the lock (egui-winit's)"
    );
    assert!(
        winits[0].contains("\"sctk-adwaita\""),
        "the lock's winit entry must list sctk-adwaita:\n{}",
        winits[0]
    );
}

#[test]
fn the_crate_asks_for_the_feature_on_linux_only() {
    let toml = read("Cargo.toml");
    let section = toml
        .split("[target.'cfg(target_os = \"linux\")'.dependencies]")
        .nth(1)
        .expect("a Linux-only dependencies section in drydock's Cargo.toml");
    let section = section.split("\n[").next().unwrap();
    assert!(section.contains("winit"), "{section}");
    assert!(section.contains("wayland-csd-adwaita"), "{section}");
    assert!(
        section.contains("default-features = false"),
        "do not turn winit's default features on: {section}"
    );
}

#[test]
fn deny_toml_needs_no_license_outside_the_list() {
    // The new crates are MIT, Apache-2.0, BSD-2/3 only; the allow list must not grow.
    let deny = read("../../deny.toml");
    let allow = deny
        .split("allow = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the license allow list");
    for banned in ["MPL", "LGPL", "GPL", "OFL", "CC0", "Unlicense"] {
        assert!(
            !allow.contains(banned),
            "{banned} must not be in the allow list"
        );
    }
}

#[test]
fn app_id_is_a_reverse_dns_desktop_id() {
    let parts: Vec<&str> = APP_ID.split('.').collect();
    assert!(parts.len() >= 3, "{APP_ID}");
    for part in &parts {
        assert!(!part.is_empty(), "{APP_ID}");
        assert!(!part.starts_with(|c: char| c.is_ascii_digit()), "{APP_ID}");
        assert!(
            part.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "{APP_ID}"
        );
    }
    assert_eq!(APP_ID, "com.subspacesystems.Drydock");
}

#[test]
fn icon_is_a_square_rgba_image() {
    let icon = window_icon();
    assert_eq!(icon.width, icon.height);
    assert!((128..=512).contains(&icon.width), "size {}", icon.width);
    assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
}

#[test]
fn icon_is_our_artwork_not_a_blank_or_default() {
    let icon = window_icon();
    let w = icon.width as usize;
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * w + x) * 4;
        [
            icon.rgba[i],
            icon.rgba[i + 1],
            icon.rgba[i + 2],
            icon.rgba[i + 3],
        ]
    };
    // Rounded tile: the corner is transparent, the middle of the left edge is the navy tile.
    assert_eq!(px(0, 0)[3], 0, "corner is transparent");
    assert_eq!(px(w / 20, w / 2), [0x09, 0x11, 0x26, 255], "tile colour");
    // The cube's top face is near white.
    assert_eq!(
        px(w / 2, w * 33 / 100),
        [0xEE, 0xF7, 0xFF, 255],
        "cube top face"
    );
}

#[test]
fn icon_png_is_the_256_frame_of_the_ico() {
    // The PNG asset and the .ico's 256 px frame come from one script and must agree.
    assert!(ICON_PNG.starts_with(b"\x89PNG\r\n\x1a\n"));
    let ico = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/drydock.ico"))
        .unwrap();
    let found = ico.windows(ICON_PNG.len()).any(|w| w == ICON_PNG);
    assert!(
        found,
        "drydock-256.png differs from the .ico's 256 px frame: rerun write_icon_files"
    );
}

#[test]
fn the_window_is_built_with_the_app_id_and_our_icon() {
    let main = read("src/main.rs");
    assert!(
        main.contains("with_app_identity("),
        "main.rs uses the app identity helper"
    );
    let helper = read("src/window_icon.rs");
    assert!(
        helper.contains(".with_app_id(APP_ID)") && helper.contains(".with_icon(window_icon())"),
        "the helper sets the app id and our icon"
    );
}

#[test]
fn app_icon_writer_writes_the_png() {
    let writer = read("tests/app_icon.rs");
    assert!(writer.contains("drydock-256.png") && writer.contains("write_icon_files"));
}
