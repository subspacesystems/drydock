// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-wayland-dialog-buttons: the vendored winit and sctk-adwaita carry the patch that lets a
//! window ask for no minimize/maximize button on Wayland. The behaviour itself is tested in
//! `vendor/sctk-adwaita/tests/buttons.rs`; no Wayland compositor runs here, so these checks read
//! the sources. See docs/specs/m11-wayland-dialog-buttons.md.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const WL_WINDOW: &str = "vendor/winit/src/platform_impl/linux/wayland/window/mod.rs";
const WL_STATE: &str = "vendor/winit/src/platform_impl/linux/wayland/window/state.rs";

#[test]
fn winit_wayland_no_longer_ignores_enabled_buttons() {
    let w = read(WL_WINDOW);
    assert!(
        !w.contains("v5 of the xdg_shell allows that"),
        "set_enabled_buttons/enabled_buttons are still the TODO no-ops"
    );
    assert!(w.contains("Local patch (Drydock, PATCHES.md)"));
    assert!(
        w.contains("attributes.enabled_buttons"),
        "not applied at creation"
    );
}

#[test]
fn winit_wayland_state_stores_the_buttons_and_tells_the_frame() {
    let s = read(WL_STATE);
    assert!(
        s.contains("enabled_buttons"),
        "WindowState keeps no enabled buttons"
    );
    assert!(s.contains("Local patch (Drydock, PATCHES.md)"));
    assert!(
        s.contains("set_enabled"),
        "the frame is never told (AdwaitaFrame::set_enabled_buttons)"
    );
}

#[test]
fn the_frame_passes_the_window_buttons_to_its_button_set() {
    let l = read("vendor/sctk-adwaita/src/lib.rs");
    assert!(l.contains("fn set_enabled_buttons"));
    assert!(l.contains("Local patch (Drydock, PATCHES.md)"));
    assert!(l.contains("buttons.set_enabled("));
    assert!(
        l.contains("buttons.set_resizable("),
        "not-resizable must drop Maximize"
    );
    assert!(
        l.contains("allows_maximize"),
        "the title double-click must not maximize a window without Maximize"
    );
}

#[test]
fn both_patch_lists_name_the_change() {
    for rel in ["vendor/winit/PATCHES.md", "vendor/sctk-adwaita/PATCHES.md"] {
        let p = read(rel);
        assert!(
            p.contains("set_enabled_buttons"),
            "{rel} does not list the patch"
        );
    }
}

#[test]
fn dialogs_still_ask_for_close_only() {
    let d = read("crates/drydock/src/dialog.rs");
    assert!(d.contains("with_minimize_button(false)"));
    assert!(d.contains("with_maximize_button(false)"));
}
