// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Subspace Systems

//! m11-wayland-dialog-buttons: which title-bar buttons are shown. A button shows when the
//! layout has it, the compositor allows it and the window allows it; Close always shows.
//! See docs/specs/m11-wayland-dialog-buttons.md.

use sctk_adwaita::buttons::{ButtonKind, Buttons};
use sctk_adwaita::pointer::{Location, MouseState};
use smithay_client_toolkit::reexports::csd_frame::{
    FrameAction, WindowManagerCapabilities, WindowState,
};
use std::time::Duration;

use ButtonKind::{Close, Maximize, Minimize};

fn gnome() -> Buttons {
    Buttons::new(Some(("appmenu".into(), "minimize,maximize,close".into())))
}

fn layout(b: &Buttons) -> (Vec<ButtonKind>, Vec<ButtonKind>) {
    b.layout()
}

#[test]
fn all_allowed_shows_three_buttons_on_the_right() {
    let b = gnome();
    assert_eq!(layout(&b), (vec![], vec![Minimize, Maximize, Close]));
    assert!(b.allows_maximize());
}

#[test]
fn default_layout_without_config() {
    let b = Buttons::new(None);
    assert_eq!(layout(&b), (vec![], vec![Minimize, Maximize, Close]));
}

#[test]
fn minimize_and_maximize_disabled_leaves_close() {
    let mut b = gnome();
    b.set_enabled(false, false);
    assert_eq!(layout(&b), (vec![], vec![Close]));
    assert!(!b.allows_maximize());
}

#[test]
fn only_minimize_disabled() {
    let mut b = gnome();
    b.set_enabled(false, true);
    assert_eq!(layout(&b), (vec![], vec![Maximize, Close]));
}

#[test]
fn not_resizable_drops_maximize_only() {
    let mut b = gnome();
    b.set_resizable(false);
    assert_eq!(layout(&b), (vec![], vec![Minimize, Close]));
    assert!(!b.allows_maximize());
    b.set_resizable(true);
    assert_eq!(layout(&b), (vec![], vec![Minimize, Maximize, Close]));
}

#[test]
fn resizable_floating_panel_can_drop_minimize_and_keep_maximize() {
    let mut b = gnome();
    b.set_enabled(false, true);
    assert_eq!(layout(&b), (vec![], vec![Maximize, Close]));
    assert!(b.allows_maximize());
}

#[test]
fn resizable_and_enabled_are_independent_conditions() {
    let mut b = gnome();
    b.set_resizable(false);
    b.set_enabled(true, false);
    b.set_resizable(true);
    // Maximize was disabled by the window, so resizing again does not bring it back.
    assert_eq!(layout(&b), (vec![], vec![Minimize, Close]));
    b.set_enabled(true, true);
    assert_eq!(layout(&b), (vec![], vec![Minimize, Maximize, Close]));
}

#[test]
fn compositor_without_minimize_hides_it_whatever_the_window_allows() {
    let mut b = gnome();
    b.update_wm_capabilities(WindowManagerCapabilities::all() - WindowManagerCapabilities::MINIMIZE);
    b.set_enabled(true, true);
    assert_eq!(layout(&b), (vec![], vec![Maximize, Close]));
    // Setting the window flags afterwards keeps the compositor's limit.
    b.set_enabled(true, true);
    assert_eq!(layout(&b), (vec![], vec![Maximize, Close]));
}

#[test]
fn compositor_without_min_and_max_leaves_close() {
    let mut b = gnome();
    b.update_wm_capabilities(WindowManagerCapabilities::empty());
    assert_eq!(layout(&b), (vec![], vec![Close]));
    assert!(!b.allows_maximize());
    b.set_enabled(true, true);
    assert_eq!(layout(&b), (vec![], vec![Close]));
}

#[test]
fn window_limit_survives_a_capability_update() {
    let mut b = gnome();
    b.set_enabled(false, false);
    b.update_wm_capabilities(WindowManagerCapabilities::all());
    assert_eq!(layout(&b), (vec![], vec![Close]));
}

#[test]
fn order_and_side_are_kept() {
    let mut b = Buttons::new(Some(("close,minimize".into(), "maximize".into())));
    assert_eq!(layout(&b), (vec![Close, Minimize], vec![Maximize]));
    b.set_enabled(false, true);
    assert_eq!(layout(&b), (vec![Close], vec![Maximize]));
    b.set_enabled(true, false);
    assert_eq!(layout(&b), (vec![Close, Minimize], vec![]));
    b.set_enabled(true, true);
    assert_eq!(layout(&b), (vec![Close, Minimize], vec![Maximize]));
}

#[test]
fn buttons_are_placed_again_after_a_change() {
    let mut b = gnome();
    b.arrange(200, 0.0);
    let all = b.right_buttons_start_x().unwrap();
    b.set_enabled(false, false);
    // Close alone sits at the right edge: width - margin(5) - button(24).
    assert_eq!(b.right_buttons_start_x(), Some(171.0));
    assert!(all < 171.0);

    let mut l = Buttons::new(Some(("close,minimize".into(), "".into())));
    l.arrange(200, 0.0);
    l.set_enabled(false, false);
    // Close alone sits at the left margin: 5 + 24.
    assert_eq!(l.left_buttons_end_x(), Some(29.0));
}

#[test]
fn double_click_on_the_title_maximizes_only_when_allowed() {
    let click = |caps: WindowManagerCapabilities, t: u64| {
        let mut m = MouseState::default();
        m.moved(Location::Head, 50.0, 10.0, true);
        let first = m.click(Duration::from_millis(t), true, true, &WindowState::empty(), &caps);
        assert!(matches!(first, Some(FrameAction::Move)));
        m.click(Duration::from_millis(t + 100), true, true, &WindowState::empty(), &caps)
    };
    assert!(matches!(
        click(WindowManagerCapabilities::all(), 1000),
        Some(FrameAction::Maximize)
    ));
    // The frame passes capabilities without MAXIMIZE when the window cannot maximize.
    let no_max = WindowManagerCapabilities::all() - WindowManagerCapabilities::MAXIMIZE;
    assert!(matches!(click(no_max, 1000), Some(FrameAction::Move)));
}
