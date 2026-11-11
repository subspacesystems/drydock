// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Wayland pop-up stacking (M12 sign-off 4): the pure parts of the local winit and egui-winit
//! patches. GNOME only accepts a grabbing pop-up whose parent is the topmost pop-up, a pop-up
//! must be positioned relative to that parent, and a pop-up that changes size is repositioned.
//! There is no compositor in CI; the real check is the mutter run in the sign-off.
#![cfg(target_os = "linux")]

use egui_winit::{
    mark_popup_parent, mark_popup_title, popup_parent_title, popup_transient_parent,
    register_popup_window, root_window_id,
};
use winit::platform::wayland::{
    POPUP_REPOSITION_VERSION, PopupRect, PopupResize, PopupStack, popup_origin_in_content,
    popup_positioner, popup_positioner_under, popup_request_in_parent, popup_resize_action,
    popup_should_grab, resolve_popup_position,
};
use winit::window::WindowId;

// --- position conversion -------------------------------------------------------------------

#[test]
fn no_pop_up_in_the_chain_is_the_content_origin() {
    assert_eq!(popup_origin_in_content(&[], (0, 37)), (0, 0));
}

#[test]
fn a_first_level_pop_up_sits_below_the_title_bar() {
    // Configured at (40, 77) in the root's geometry; the content starts 37 px down.
    assert_eq!(popup_origin_in_content(&[(40, 77)], (0, 37)), (40, 40));
}

#[test]
fn nested_pop_ups_add_up() {
    assert_eq!(
        popup_origin_in_content(&[(40, 77), (150, 20)], (0, 37)),
        (190, 60)
    );
}

#[test]
fn no_title_bar_means_no_offset() {
    assert_eq!(popup_origin_in_content(&[(10, 30)], (0, 0)), (10, 30));
}

#[test]
fn a_request_is_made_relative_to_the_parent_pop_up() {
    assert_eq!(popup_request_in_parent((200, 100), (40, 40)), (160, 60));
}

#[test]
fn a_submenu_left_of_its_menu_has_a_negative_x() {
    assert_eq!(popup_request_in_parent((10, 100), (40, 40)), (-30, 60));
}

#[test]
fn under_the_root_it_is_the_plain_positioner() {
    assert_eq!(
        popup_positioner_under(None, (0, 37), (100, 20), (200, 300)),
        popup_positioner((0, 37), (100, 20), (200, 300))
    );
}

#[test]
fn under_a_pop_up_the_anchor_is_in_the_parent_and_ignores_the_title_bar() {
    let p = popup_positioner_under(Some((40, 40)), (0, 37), (200, 100), (150, 90));
    assert_eq!(p.size, (150, 90));
    assert_eq!(
        p.anchor_rect,
        PopupRect {
            x: 160,
            y: 60,
            width: 1,
            height: 1
        }
    );
    assert_eq!(p.offset, (0, 0));
    assert!(p.adjustment.slide_x && p.adjustment.slide_y);
    assert!(p.adjustment.flip_x && p.adjustment.flip_y);
}

#[test]
fn the_compositor_model_puts_a_submenu_where_it_was_asked() {
    // Menu at content (40, 40) = output (40, 77) when the root is at (0, 0) with a 37 px bar.
    let menu_origin_on_output = (40, 77);
    let p = popup_positioner_under(Some((40, 40)), (0, 37), (200, 100), (150, 90));
    let screen = PopupRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    // The parent pop-up has no frame: its geometry origin is the pop-up's own corner.
    let at = resolve_popup_position(&p, menu_origin_on_output, screen);
    assert_eq!(at, (200, 137));
}

// --- the stack and the grab rule -----------------------------------------------------------

#[test]
fn an_empty_stack_allows_a_grab_under_the_root_only() {
    let stack: PopupStack<u32> = PopupStack::new();
    assert_eq!(stack.topmost(), None);
    assert!(stack.may_grab_under(None));
    assert!(!stack.may_grab_under(Some(1)));
}

#[test]
fn only_the_topmost_pop_up_may_be_a_parent() {
    let mut stack = PopupStack::new();
    stack.push(1_u32);
    assert_eq!(stack.topmost(), Some(1));
    assert!(stack.may_grab_under(Some(1)));
    assert!(
        !stack.may_grab_under(None),
        "the root is not topmost any more"
    );
    stack.push(2);
    assert_eq!(stack.topmost(), Some(2));
    assert!(stack.may_grab_under(Some(2)));
    assert!(!stack.may_grab_under(Some(1)));
}

#[test]
fn removing_the_top_exposes_the_one_below() {
    let mut stack = PopupStack::new();
    stack.push(1_u32);
    stack.push(2);
    stack.remove(2);
    assert_eq!(stack.topmost(), Some(1));
    stack.remove(1);
    assert_eq!(stack.topmost(), None);
    assert!(stack.may_grab_under(None));
}

#[test]
fn removing_an_unknown_id_changes_nothing() {
    let mut stack = PopupStack::new();
    stack.push(1_u32);
    stack.remove(9);
    assert_eq!(stack.topmost(), Some(1));
}

#[test]
fn removing_a_middle_pop_up_keeps_the_order() {
    let mut stack = PopupStack::new();
    stack.push(1_u32);
    stack.push(2);
    stack.push(3);
    stack.remove(2);
    assert_eq!(stack.topmost(), Some(3));
    stack.remove(3);
    assert_eq!(stack.topmost(), Some(1));
}

#[test]
fn pushing_an_id_twice_keeps_one_copy_on_top() {
    let mut stack = PopupStack::new();
    stack.push(1_u32);
    stack.push(2);
    stack.push(1);
    assert_eq!(stack.topmost(), Some(1));
    stack.remove(1);
    assert_eq!(stack.topmost(), Some(2));
}

#[test]
fn the_switch_sequence_old_gone_before_new() {
    // Menu A is open, then hover switches to B: A is destroyed first, B grabs under the root.
    let mut stack = PopupStack::new();
    stack.push(10_u32);
    assert!(
        !stack.may_grab_under(None),
        "B while A is alive would be refused"
    );
    stack.remove(10);
    assert!(stack.may_grab_under(None));
    stack.push(11);
    assert_eq!(stack.topmost(), Some(11));
}

#[test]
fn a_grab_needs_a_serial_and_an_allowed_parent() {
    assert!(popup_should_grab(Some(7), true));
    assert!(!popup_should_grab(None, true));
    assert!(!popup_should_grab(Some(7), false));
    assert!(!popup_should_grab(None, false));
}

// --- reposition on a size change -----------------------------------------------------------

#[test]
fn the_reposition_request_needs_xdg_wm_base_version_3() {
    assert_eq!(POPUP_REPOSITION_VERSION, 3);
}

#[test]
fn a_new_size_is_repositioned() {
    assert_eq!(
        popup_resize_action(3, (120, 40), (210, 754)),
        PopupResize::Reposition
    );
    assert_eq!(
        popup_resize_action(6, (120, 40), (120, 41)),
        PopupResize::Reposition
    );
}

#[test]
fn the_same_size_is_left_alone() {
    assert_eq!(
        popup_resize_action(3, (210, 754), (210, 754)),
        PopupResize::Keep
    );
}

#[test]
fn an_old_compositor_cannot_reposition() {
    assert_eq!(
        popup_resize_action(2, (120, 40), (210, 754)),
        PopupResize::Keep
    );
    assert_eq!(
        popup_resize_action(1, (120, 40), (210, 754)),
        PopupResize::Keep
    );
}

#[test]
fn a_tall_menu_near_the_bottom_is_kept_on_screen_by_the_new_size() {
    // Created at 120x40 it fits at y = 1000 on a 1080 screen; at 754 the compositor flips it above the anchor (its bottom at y = 1001).
    let screen = PopupRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let small = popup_positioner((0, 0), (10, 1000), (120, 40));
    assert_eq!(resolve_popup_position(&small, (0, 0), screen).1, 1000);
    let tall = popup_positioner((0, 0), (10, 1000), (210, 754));
    assert_eq!(resolve_popup_position(&tall, (0, 0), screen).1, 1001 - 754);
}

// --- egui-winit: the parent mark -----------------------------------------------------------

fn id(n: u64) -> WindowId {
    WindowId::from(n)
}

#[test]
fn a_pop_up_without_a_parent_mark_has_none() {
    let ctx = eframe::egui::Context::default();
    mark_popup_title(&ctx, "menu");
    assert_eq!(popup_parent_title(&ctx, "menu"), None);
}

#[test]
fn the_parent_mark_round_trips() {
    let ctx = eframe::egui::Context::default();
    mark_popup_title(&ctx, "sub");
    mark_popup_parent(&ctx, "sub", "menu");
    assert_eq!(popup_parent_title(&ctx, "sub").as_deref(), Some("menu"));
    assert_eq!(popup_parent_title(&ctx, "menu"), None);
}

#[test]
fn a_registered_parent_becomes_the_transient_parent() {
    let ctx = eframe::egui::Context::default();
    mark_popup_parent(&ctx, "sub", "menu");
    register_popup_window(&ctx, "menu", id(5));
    assert_eq!(popup_transient_parent(&ctx, "sub"), Some(id(5)));
}

#[test]
fn an_unmarked_pop_up_falls_back_to_the_root() {
    let ctx = eframe::egui::Context::default();
    assert_eq!(popup_transient_parent(&ctx, "menu"), root_window_id(&ctx));
    ctx.data_mut(|data| data.insert_temp(eframe::egui::Id::new("egui_winit_root_window"), 3_u64));
    assert_eq!(popup_transient_parent(&ctx, "menu"), Some(id(3)));
}

#[test]
fn an_unregistered_parent_falls_back_to_the_root() {
    let ctx = eframe::egui::Context::default();
    ctx.data_mut(|data| data.insert_temp(eframe::egui::Id::new("egui_winit_root_window"), 3_u64));
    mark_popup_parent(&ctx, "sub", "menu");
    assert_eq!(popup_transient_parent(&ctx, "sub"), Some(id(3)));
}

#[test]
fn a_new_window_with_the_same_title_replaces_the_entry() {
    let ctx = eframe::egui::Context::default();
    mark_popup_parent(&ctx, "sub", "menu");
    register_popup_window(&ctx, "menu", id(5));
    register_popup_window(&ctx, "menu", id(8));
    assert_eq!(popup_transient_parent(&ctx, "sub"), Some(id(8)));
}
