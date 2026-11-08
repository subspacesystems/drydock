// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-keep-position: the position a dialog window opens at is worked out once and
//! kept, so moving the main window does not pull the dialog back to the centre.

use drydock::dialog::{
    centered_position, dialog_viewport_id, forget_opening_position, opening_position,
};
use eframe::egui::{self, Rect, Vec2, pos2, vec2};

const SIZE: Vec2 = vec2(200.0, 100.0);

fn root(min_x: f32, min_y: f32) -> Rect {
    Rect::from_min_size(pos2(min_x, min_y), vec2(800.0, 600.0))
}

#[test]
fn first_call_centres_over_the_main_window() {
    let ctx = egui::Context::default();
    let id = dialog_viewport_id("keep-a");
    let got = opening_position(&ctx, id, Some(root(100.0, 50.0)), SIZE);
    assert_eq!(got, Some(centered_position(root(100.0, 50.0), SIZE)));
    assert_eq!(got, Some(pos2(400.0, 300.0)));
}

#[test]
fn later_calls_keep_the_first_position_when_the_main_window_moves() {
    let ctx = egui::Context::default();
    let id = dialog_viewport_id("keep-b");
    let first = opening_position(&ctx, id, Some(root(100.0, 50.0)), SIZE);
    let later = opening_position(&ctx, id, Some(root(900.0, 400.0)), SIZE);
    assert_eq!(
        later, first,
        "moving the main window must not move the dialog"
    );
    let without = opening_position(&ctx, id, None, SIZE);
    assert_eq!(without, first, "an unknown main window keeps it too");
}

#[test]
fn unknown_main_window_gives_none_and_stores_nothing() {
    let ctx = egui::Context::default();
    let id = dialog_viewport_id("keep-c");
    assert_eq!(opening_position(&ctx, id, None, SIZE), None);
    // The first frame where the rectangle is known fixes the position.
    let got = opening_position(&ctx, id, Some(root(0.0, 0.0)), SIZE);
    assert_eq!(got, Some(pos2(300.0, 250.0)));
    assert_eq!(
        opening_position(&ctx, id, Some(root(50.0, 50.0)), SIZE),
        got
    );
}

#[test]
fn forgetting_makes_the_next_opening_centre_again() {
    let ctx = egui::Context::default();
    let id = dialog_viewport_id("keep-d");
    opening_position(&ctx, id, Some(root(100.0, 50.0)), SIZE);
    forget_opening_position(&ctx, id);
    let again = opening_position(&ctx, id, Some(root(900.0, 400.0)), SIZE);
    assert_eq!(again, Some(centered_position(root(900.0, 400.0), SIZE)));
    forget_opening_position(&ctx, dialog_viewport_id("never-opened"));
}

#[test]
fn each_dialog_keeps_its_own_position() {
    let ctx = egui::Context::default();
    let a = dialog_viewport_id("keep-e1");
    let b = dialog_viewport_id("keep-e2");
    let pa = opening_position(&ctx, a, Some(root(0.0, 0.0)), SIZE);
    let pb = opening_position(&ctx, b, Some(root(500.0, 500.0)), SIZE);
    assert_ne!(pa, pb);
    forget_opening_position(&ctx, a);
    assert_eq!(opening_position(&ctx, b, Some(root(0.0, 0.0)), SIZE), pb);
}
