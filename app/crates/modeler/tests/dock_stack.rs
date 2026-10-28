// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-panel-stack.md`: stacking docked panels, the border
//! between two panels, revealing a panel, and the removed doubled titles.

use modeler::dock::{
    DockLayout, HEADER_HEIGHT, MIN_BODY, PanelId, Side, StackRow, drag_splitter, stack_heights,
};

const MIN: f32 = HEADER_HEIGHT + MIN_BODY;

fn row(collapsed: bool, weight: f32) -> StackRow {
    StackRow { collapsed, weight }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn equal_weights_share_the_column_equally() {
    let h = stack_heights(600.0, &[row(false, 1.0), row(false, 1.0)]);
    assert!(near(h[0], 300.0) && near(h[1], 300.0), "{h:?}");
}

#[test]
fn heights_follow_the_weights() {
    let h = stack_heights(600.0, &[row(false, 1.0), row(false, 3.0)]);
    assert!(near(h[0], 150.0) && near(h[1], 450.0), "{h:?}");
}

#[test]
fn a_collapsed_panel_is_just_its_header_and_the_others_keep_their_height() {
    // Changed by m6-panel-collapse: the others no longer take the collapsed panel's space.
    let h = stack_heights(600.0, &[row(false, 1.0), row(true, 1.0), row(false, 1.0)]);
    assert!(near(h[1], HEADER_HEIGHT), "{h:?}");
    assert!(near(h[0], 200.0) && near(h[2], 200.0), "{h:?}");
    assert!(h.iter().sum::<f32>() < 600.0, "{h:?}");
}

#[test]
fn a_panel_never_gets_less_than_a_header_and_the_minimum_body() {
    let h = stack_heights(600.0, &[row(false, 1.0), row(false, 100.0)]);
    assert!(near(h[0], MIN), "{h:?}");
    assert!(near(h.iter().sum::<f32>(), 600.0), "{h:?}");
}

#[test]
fn a_column_too_short_for_the_minimums_gives_each_the_minimum() {
    let h = stack_heights(100.0, &[row(false, 1.0), row(false, 1.0)]);
    assert!(near(h[0], MIN) && near(h[1], MIN), "{h:?}");
}

#[test]
fn nonpositive_weights_count_as_one() {
    let h = stack_heights(600.0, &[row(false, 0.0), row(false, -5.0)]);
    assert!(near(h[0], 300.0) && near(h[1], 300.0), "{h:?}");
}

#[test]
fn all_collapsed_and_empty_columns() {
    let h = stack_heights(600.0, &[row(true, 1.0), row(true, 1.0)]);
    assert!(
        near(h[0], HEADER_HEIGHT) && near(h[1], HEADER_HEIGHT),
        "{h:?}"
    );
    assert!(stack_heights(600.0, &[]).is_empty());
}

#[test]
fn dragging_a_border_moves_height_between_its_two_panels() {
    let mut rows = [row(false, 1.0), row(false, 1.0)];
    let heights = stack_heights(600.0, &rows);
    drag_splitter(&mut rows, &heights, 0, 50.0);
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], 350.0) && near(h[1], 250.0), "{h:?}");
    drag_splitter(&mut rows, &h, 0, -100.0);
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], 250.0) && near(h[1], 350.0), "{h:?}");
}

#[test]
fn dragging_a_border_stops_at_the_minimum() {
    let mut rows = [row(false, 1.0), row(false, 1.0)];
    let heights = stack_heights(600.0, &rows);
    drag_splitter(&mut rows, &heights, 0, 10_000.0);
    let h = stack_heights(600.0, &rows);
    assert!(near(h[1], MIN) && near(h[0], 600.0 - MIN), "{h:?}");
    let heights = h;
    drag_splitter(&mut rows, &heights, 0, -10_000.0);
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], MIN) && near(h[1], 600.0 - MIN), "{h:?}");
}

#[test]
fn dragging_a_border_leaves_the_other_panels_alone() {
    let mut rows = [row(false, 1.0), row(false, 1.0), row(false, 1.0)];
    let heights = stack_heights(900.0, &rows);
    drag_splitter(&mut rows, &heights, 1, 40.0);
    let h = stack_heights(900.0, &rows);
    assert!(near(h[0], 300.0), "{h:?}");
    assert!(near(h[1], 340.0) && near(h[2], 260.0), "{h:?}");
}

#[test]
fn a_border_next_to_a_collapsed_panel_or_past_the_end_does_nothing() {
    let mut rows = [row(false, 1.0), row(true, 1.0), row(false, 1.0)];
    let before = rows;
    let heights = stack_heights(600.0, &rows);
    drag_splitter(&mut rows, &heights, 0, 30.0);
    drag_splitter(&mut rows, &heights, 1, 30.0);
    drag_splitter(&mut rows, &heights, 2, 30.0);
    drag_splitter(&mut rows, &heights, 9, 30.0);
    assert_eq!(rows, before);
}

#[test]
fn reveal_opens_a_closed_panel_and_expands_a_collapsed_one() {
    let mut layout = DockLayout::default();
    assert!(!layout.is_open(PanelId::Components));
    layout.reveal(PanelId::Components);
    assert!(layout.is_open(PanelId::Components));
    layout.set_collapsed(PanelId::Materials, true);
    layout.reveal(PanelId::Materials);
    assert!(layout.is_open(PanelId::Materials));
    assert!(!layout.panel(PanelId::Materials).collapsed);
}

#[test]
fn reveal_never_closes_and_keeps_the_place() {
    let mut layout = DockLayout::default();
    layout.dock(PanelId::EntityInfo, Side::Left, 0);
    let before = layout.clone();
    layout.reveal(PanelId::EntityInfo);
    assert_eq!(layout, before);
    assert_eq!(layout.docked(Side::Left), vec![PanelId::EntityInfo]);
}
