// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-popup-wayland-on: pop-up windows on for Wayland. No compositor is available in CI, so
//! these cover the support rule and the position rule (content-relative on Wayland, screen
//! coordinates elsewhere). See docs/specs/m12-popup-wayland-on.md.

use drydock::chrome::{menu_popup_position, submenu_popup_position};
use drydock::context_menu::context_popup_position;
use drydock::popup::{PopupMode, popup_mode, popup_root_inner, popup_supported_on};
use eframe::egui::{Rect, pos2, vec2};

fn root() -> Rect {
    Rect::from_min_size(pos2(300.0, 200.0), vec2(800.0, 600.0))
}

#[test]
fn wayland_supports_popups() {
    assert!(popup_supported_on("linux", true));
    assert!(popup_supported_on("linux", false));
    assert!(popup_supported_on("windows", false));
    assert!(!popup_supported_on("macos", false));
}

#[test]
fn window_mode_is_chosen_on_wayland_when_not_embedded() {
    let supported = popup_supported_on("linux", true);
    assert_eq!(popup_mode(false, supported), PopupMode::Window);
    assert_eq!(popup_mode(true, supported), PopupMode::Inline, "UI tests");
}

#[test]
fn root_inner_is_dropped_on_wayland_only() {
    assert_eq!(popup_root_inner(Some(root()), true), None);
    assert_eq!(popup_root_inner(None, true), None);
    assert_eq!(popup_root_inner(Some(root()), false), Some(root()));
    assert_eq!(popup_root_inner(None, false), None);
}

#[test]
fn menu_position_is_content_relative_on_wayland() {
    let title = Rect::from_min_size(pos2(40.0, 2.0), vec2(50.0, 20.0));
    let wayland = menu_popup_position(popup_root_inner(Some(root()), true), title);
    assert_eq!(wayland, pos2(40.0, 22.0));
    let x11 = menu_popup_position(popup_root_inner(Some(root()), false), title);
    assert_eq!(x11, pos2(340.0, 222.0));
}

#[test]
fn submenu_position_is_content_relative_on_wayland() {
    let row = Rect::from_min_size(pos2(40.0, 100.0), vec2(120.0, 20.0));
    let size = vec2(150.0, 90.0);
    let wayland = submenu_popup_position(popup_root_inner(Some(root()), true), row, size, None);
    assert_eq!(wayland, pos2(160.0, 100.0));
    let x11 = submenu_popup_position(popup_root_inner(Some(root()), false), row, size, None);
    assert_eq!(x11, pos2(460.0, 300.0));
}

#[test]
fn context_position_is_content_relative_on_wayland() {
    let click = pos2(55.0, 66.0);
    assert_eq!(
        context_popup_position(popup_root_inner(Some(root()), true), click),
        click
    );
    assert_eq!(
        context_popup_position(popup_root_inner(Some(root()), false), click),
        pos2(355.0, 266.0)
    );
}
