// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Wayland pop-ups (M12): the placement math of the local winit patch.
//!
//! There is no Wayland compositor in CI, so the `xdg_popup` itself cannot be tested here; these
//! tests cover the pure functions `popup_positioner` (what is sent to `xdg_positioner`) and
//! `resolve_popup_position` (a model of the compositor's anchor / gravity / flip / slide rules).
#![cfg(target_os = "linux")]

use winit::platform::wayland::{
    PopupAdjustment, PopupPositioner, PopupRect, popup_positioner, resolve_popup_position,
};

fn rect(x: i32, y: i32, width: i32, height: i32) -> PopupRect {
    PopupRect {
        x,
        y,
        width,
        height,
    }
}

fn positioner(
    anchor: (i32, i32),
    size: (i32, i32),
    adjustment: PopupAdjustment,
) -> PopupPositioner {
    PopupPositioner {
        size,
        anchor_rect: rect(anchor.0, anchor.1, 1, 1),
        offset: (0, 0),
        adjustment,
    }
}

const NONE: PopupAdjustment = PopupAdjustment {
    slide_x: false,
    slide_y: false,
    flip_x: false,
    flip_y: false,
};
const ALL: PopupAdjustment = PopupAdjustment {
    slide_x: true,
    slide_y: true,
    flip_x: true,
    flip_y: true,
};
const SCREEN: PopupRect = PopupRect {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
};

#[test]
fn positioner_anchors_a_one_pixel_rect_below_the_title_bar() {
    let p = popup_positioner((0, 37), (100, 20), (200, 300));
    assert_eq!(p.size, (200, 300));
    assert_eq!(p.anchor_rect, rect(100, 57, 1, 1));
    assert_eq!(p.offset, (0, 0));
}

#[test]
fn positioner_without_a_title_bar_uses_the_request_as_is() {
    let p = popup_positioner((0, 0), (12, 34), (50, 60));
    assert_eq!(p.anchor_rect, rect(12, 34, 1, 1));
}

#[test]
fn positioner_keeps_negative_requests_past_the_parent_edge() {
    let p = popup_positioner((0, 37), (-50, -10), (50, 60));
    assert_eq!(p.anchor_rect, rect(-50, 27, 1, 1));
}

#[test]
fn positioner_asks_for_flip_and_slide_on_both_axes() {
    let p = popup_positioner((0, 0), (0, 0), (10, 10));
    assert_eq!(p.adjustment, ALL);
}

#[test]
fn room_to_spare_puts_the_top_left_corner_on_the_anchor() {
    let p = positioner((100, 200), (200, 300), ALL);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (100, 200));
}

#[test]
fn the_parent_origin_and_the_offset_shift_the_result() {
    let mut p = positioner((10, 10), (50, 50), NONE);
    p.offset = (3, 4);
    assert_eq!(resolve_popup_position(&p, (500, 100), SCREEN), (513, 114));
}

#[test]
fn a_pop_up_may_reach_past_the_parent_window() {
    // Parent 400 x 300 at (100, 100); the menu is requested at its lower right corner.
    let p = popup_positioner((0, 0), (350, 250), (200, 200));
    assert_eq!(resolve_popup_position(&p, (100, 100), SCREEN), (450, 350));
}

#[test]
fn full_request_to_final_position() {
    let p = popup_positioner((0, 37), (100, 20), (200, 300));
    assert_eq!(resolve_popup_position(&p, (50, 60), SCREEN), (150, 117));
}

#[test]
fn no_flags_leave_a_constrained_pop_up_where_it_is() {
    let p = positioner((1800, 900), (200, 300), NONE);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (1800, 900));
}

#[test]
fn flip_y_opens_upward_when_there_is_no_room_below() {
    let p = positioner((100, 900), (200, 300), ALL);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (100, 601));
}

#[test]
fn flip_x_opens_leftward_when_there_is_no_room_to_the_right() {
    let p = positioner((1800, 100), (200, 300), ALL);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (1601, 100));
}

#[test]
fn flip_is_used_only_when_it_fits() {
    // 400 high output: below does not fit, above (-99) does not fit either; slide wins.
    let bounds = rect(0, 0, 1920, 400);
    let p = positioner((100, 200), (200, 300), ALL);
    assert_eq!(resolve_popup_position(&p, (0, 0), bounds), (100, 100));
}

#[test]
fn slide_alone_moves_the_pop_up_back_inside() {
    let slide_x = PopupAdjustment {
        slide_x: true,
        ..NONE
    };
    let p = positioner((1800, 100), (200, 300), slide_x);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (1720, 100));
    let slide_y = PopupAdjustment {
        slide_y: true,
        ..NONE
    };
    let p = positioner((100, 900), (200, 300), slide_y);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (100, 780));
}

#[test]
fn flip_only_when_slide_is_off_keeps_a_too_big_pop_up_in_place() {
    // Neither side fits and sliding is not allowed: stays at the unflipped position.
    let bounds = rect(0, 0, 1920, 400);
    let flip_y = PopupAdjustment {
        flip_y: true,
        ..NONE
    };
    let p = positioner((100, 200), (200, 300), flip_y);
    assert_eq!(resolve_popup_position(&p, (0, 0), bounds), (100, 200));
}

#[test]
fn slide_on_a_second_monitor_stops_at_its_left_edge() {
    let bounds = rect(1920, 0, 1920, 1080);
    let slide_x = PopupAdjustment {
        slide_x: true,
        ..NONE
    };
    let p = positioner((100, 10), (200, 300), slide_x);
    assert_eq!(resolve_popup_position(&p, (1800, 0), bounds), (1920, 10));
}

#[test]
fn a_pop_up_bigger_than_the_bounds_keeps_its_top_left_corner_visible() {
    let bounds = rect(0, 0, 100, 100);
    let p = positioner((50, 50), (200, 150), ALL);
    assert_eq!(resolve_popup_position(&p, (0, 0), bounds), (0, 0));
}

#[test]
fn axes_are_adjusted_independently() {
    // x fits, y flips.
    let p = positioner((100, 1000), (200, 200), ALL);
    assert_eq!(resolve_popup_position(&p, (0, 0), SCREEN), (100, 801));
}
