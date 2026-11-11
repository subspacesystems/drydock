// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-wayland-popup-switch: a submenu's pop-up names its menu's pop-up as parent, and
//! switching menus destroys the old pop-up the frame before the new one is created
//! (`docs/specs/m12-wayland-popup-switch.md`). The Wayland path is forced with
//! `force_wayland_windows`; the viewports stay embedded.

use drydock::chrome::{
    MenuState, MenuSwitch, menu_bar_with_toolbars, menu_popup_title, menu_switch_step, open_menu,
    open_submenu, shown_menu, submenu_popup_title,
};
use drydock::popup::{force_wayland_windows, wayland_in};
use drydock::toolbars::ToolbarLayout;
use eframe::egui::{self, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

// ---- pure ---------------------------------------------------------------------------

#[test]
fn switching_menus_on_wayland_hides_one_frame() {
    assert_eq!(
        menu_switch_step(Some("File"), "Edit", true),
        MenuSwitch::HideThenShow
    );
}

#[test]
fn every_other_case_shows_at_once() {
    assert_eq!(menu_switch_step(None, "Edit", true), MenuSwitch::Show);
    assert_eq!(
        menu_switch_step(Some("Edit"), "Edit", true),
        MenuSwitch::Show
    );
    assert_eq!(
        menu_switch_step(Some("File"), "Edit", false),
        MenuSwitch::Show
    );
    assert_eq!(menu_switch_step(None, "Edit", false), MenuSwitch::Show);
}

// ---- kittest, Wayland path forced ----------------------------------------------------

#[derive(Default)]
struct App {
    layout: ToolbarLayout,
}

fn menus(forced: bool) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| {
                let state = MenuState::default();
                menu_bar_with_toolbars(ui, &state, &mut app.layout);
            },
            App::default(),
        );
    h.ctx.set_fonts(drydock::look::font_definitions());
    force_wayland_windows(&h.ctx, forced);
    h.run();
    h
}

#[test]
fn the_hook_turns_the_wayland_path_on() {
    let h = menus(true);
    assert!(wayland_in(&h.ctx));
}

/// Steps until `open_menu` is `want` (at most 10 frames), returning `shown_menu` after each.
fn frames_until_open(h: &mut Harness<'static, App>, want: &str) -> Vec<Option<String>> {
    let mut seen = Vec::new();
    for _ in 0..10 {
        h.step();
        seen.push(shown_menu(&h.ctx));
        if open_menu(&h.ctx).as_deref() == Some(want) && seen.last().unwrap().is_some() {
            break;
        }
    }
    seen
}

#[test]
fn a_hover_switch_shows_no_pop_up_for_a_frame_then_the_new_one() {
    let mut h = menus(true);
    h.get_by_label("File").click();
    h.run();
    h.run();
    assert_eq!(shown_menu(&h.ctx).as_deref(), Some("File"));
    h.get_by_label("Edit").hover();
    let seen = frames_until_open(&mut h, "Edit");
    let first_none = seen.iter().position(|s| s.is_none());
    assert!(first_none.is_some(), "a frame without a pop-up: {seen:?}");
    let first_none = first_none.unwrap();
    assert!(
        seen[..first_none]
            .iter()
            .all(|s| s.as_deref() == Some("File")),
        "{seen:?}"
    );
    assert!(
        seen[first_none..]
            .iter()
            .all(|s| s.is_none() || s.as_deref() == Some("Edit")),
        "File never comes back: {seen:?}"
    );
    assert_eq!(seen.last().unwrap().as_deref(), Some("Edit"), "{seen:?}");
}

#[test]
fn an_arrow_key_switch_hides_one_frame_too() {
    let mut h = menus(true);
    h.get_by_label("File").click();
    h.run();
    h.run();
    h.key_press(egui::Key::ArrowRight);
    let seen = frames_until_open(&mut h, "Edit");
    assert!(seen.iter().any(|s| s.is_none()), "{seen:?}");
    assert_eq!(seen.last().unwrap().as_deref(), Some("Edit"), "{seen:?}");
}

#[test]
fn without_the_hook_a_switch_has_no_hidden_frame() {
    let mut h = menus(false);
    h.get_by_label("File").click();
    h.run();
    h.run();
    h.get_by_label("Edit").hover();
    h.step();
    h.step();
    assert_eq!(open_menu(&h.ctx).as_deref(), Some("Edit"));
    assert!(h.query_by_label("Undo").is_some());
    assert!(h.query_by_label("New").is_none());
}

#[test]
fn a_switch_closes_the_submenu_before_the_menu_goes() {
    let mut h = menus(true);
    h.get_by_label("View").click();
    h.run();
    h.run();
    h.get_by_label("Toolbars").click();
    h.run();
    h.run();
    assert_eq!(open_submenu(&h.ctx).as_deref(), Some("Toolbars"));
    h.get_by_label("Edit").hover();
    h.step();
    assert_eq!(
        open_submenu(&h.ctx),
        None,
        "submenu cleared in the switch frame"
    );
    assert!(h.query_by_label("Reset Toolbars...").is_none());
}

#[test]
fn a_submenu_pop_up_names_its_menu_as_parent() {
    let mut h = menus(true);
    h.get_by_label("View").click();
    h.run();
    h.run();
    h.get_by_label("Toolbars").click();
    h.run();
    h.run();
    assert_eq!(
        egui_winit::popup_parent_title(&h.ctx, &submenu_popup_title("Toolbars")),
        Some(menu_popup_title("View"))
    );
    assert_eq!(
        egui_winit::popup_parent_title(&h.ctx, &menu_popup_title("View")),
        None,
        "a menu's parent is the main window"
    );
}
