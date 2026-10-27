// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-drag.md`: the insertion index and the
//! drop outline.

use eframe::egui::{Rect, pos2, vec2};
use modeler::toolbars::{Edge, drop_outline, insertion_index};

fn row() -> Vec<Rect> {
    vec![
        Rect::from_min_size(pos2(0.0, 20.0), vec2(100.0, 30.0)),
        Rect::from_min_size(pos2(104.0, 20.0), vec2(60.0, 30.0)),
        Rect::from_min_size(pos2(168.0, 20.0), vec2(80.0, 30.0)),
    ]
}

fn column() -> Vec<Rect> {
    vec![
        Rect::from_min_size(pos2(10.0, 40.0), vec2(30.0, 100.0)),
        Rect::from_min_size(pos2(10.0, 144.0), vec2(30.0, 60.0)),
    ]
}

fn window() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))
}

#[test]
fn index_in_a_row_counts_centres_before_the_pointer() {
    let r = row();
    // centres at x = 50, 134, 208
    assert_eq!(insertion_index(Edge::Top, &r, pos2(10.0, 30.0)), 0);
    assert_eq!(insertion_index(Edge::Top, &r, pos2(51.0, 30.0)), 1);
    assert_eq!(insertion_index(Edge::Top, &r, pos2(150.0, 0.0)), 2);
    assert_eq!(insertion_index(Edge::Bottom, &r, pos2(300.0, 500.0)), 3);
}

#[test]
fn index_on_a_centre_is_before_it() {
    let r = row();
    assert_eq!(insertion_index(Edge::Top, &r, pos2(50.0, 30.0)), 0);
    assert_eq!(insertion_index(Edge::Top, &r, pos2(134.0, 30.0)), 1);
}

#[test]
fn index_in_a_column_uses_y() {
    let c = column();
    // centres at y = 90, 174
    assert_eq!(insertion_index(Edge::Left, &c, pos2(500.0, 10.0)), 0);
    assert_eq!(insertion_index(Edge::Left, &c, pos2(0.0, 100.0)), 1);
    assert_eq!(insertion_index(Edge::Right, &c, pos2(0.0, 400.0)), 2);
}

#[test]
fn index_with_nothing_docked_is_zero() {
    assert_eq!(insertion_index(Edge::Top, &[], pos2(300.0, 10.0)), 0);
    assert_eq!(insertion_index(Edge::Left, &[], pos2(10.0, 300.0)), 0);
}

#[test]
fn outline_in_a_row_starts_at_the_toolbar_it_goes_before() {
    let r = row();
    let size = vec2(70.0, 28.0);
    let o = drop_outline(Edge::Top, &r, 1, window(), size);
    assert_eq!(o, Rect::from_min_size(pos2(104.0, 20.0), size));
    let first = drop_outline(Edge::Top, &r, 0, window(), size);
    assert_eq!(first.min, pos2(0.0, 20.0));
}

#[test]
fn outline_past_the_last_toolbar_starts_at_its_end() {
    let r = row();
    let size = vec2(70.0, 28.0);
    let o = drop_outline(Edge::Bottom, &r, 3, window(), size);
    assert_eq!(o, Rect::from_min_size(pos2(248.0, 20.0), size));
}

#[test]
fn outline_in_a_column() {
    let c = column();
    let size = vec2(26.0, 90.0);
    let o = drop_outline(Edge::Left, &c, 1, window(), size);
    assert_eq!(o, Rect::from_min_size(pos2(10.0, 144.0), size));
    let end = drop_outline(Edge::Right, &c, 2, window(), size);
    assert_eq!(end, Rect::from_min_size(pos2(10.0, 204.0), size));
}

#[test]
fn outline_with_nothing_docked_sits_against_the_edge() {
    let size = vec2(120.0, 30.0);
    let w = window();
    assert_eq!(
        drop_outline(Edge::Top, &[], 0, w, size),
        Rect::from_min_size(pos2(0.0, 0.0), size)
    );
    assert_eq!(
        drop_outline(Edge::Bottom, &[], 0, w, size),
        Rect::from_min_size(pos2(0.0, 570.0), size)
    );
    let tall = vec2(30.0, 120.0);
    assert_eq!(
        drop_outline(Edge::Left, &[], 0, w, tall),
        Rect::from_min_size(pos2(0.0, 0.0), tall)
    );
    assert_eq!(
        drop_outline(Edge::Right, &[], 0, w, tall),
        Rect::from_min_size(pos2(770.0, 0.0), tall)
    );
}
