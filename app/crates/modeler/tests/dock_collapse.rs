// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-panel-collapse.md`: collapsing a docked panel keeps
//! the other panels' heights, so their title bars stay put and the free space opens at the
//! bottom of the column.

use modeler::dock::{HEADER_HEIGHT, MIN_BODY, StackRow, drag_splitter, stack_heights};

const MIN: f32 = HEADER_HEIGHT + MIN_BODY;

fn row(collapsed: bool, weight: f32) -> StackRow {
    StackRow { collapsed, weight }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// Top edge of each row, from the column's top.
fn tops(heights: &[f32]) -> Vec<f32> {
    let mut top = 0.0;
    heights
        .iter()
        .map(|height| {
            let this = top;
            top += height;
            this
        })
        .collect()
}

#[test]
fn collapsing_the_lower_panel_leaves_the_upper_one_and_both_titles_in_place() {
    let open = stack_heights(600.0, &[row(false, 1.0), row(false, 1.0)]);
    let folded = stack_heights(600.0, &[row(false, 1.0), row(true, 1.0)]);
    assert!(near(folded[0], open[0]), "{open:?} -> {folded:?}");
    assert!(near(folded[1], HEADER_HEIGHT), "{folded:?}");
    assert!(
        near(tops(&folded)[1], tops(&open)[1]),
        "{open:?} -> {folded:?}"
    );
    assert!(folded.iter().sum::<f32>() < 600.0 - 100.0, "{folded:?}");
}

#[test]
fn collapsing_a_middle_panel_only_moves_the_panels_below_it_up() {
    let open = stack_heights(600.0, &[row(false, 1.0), row(false, 1.0), row(false, 1.0)]);
    let folded = stack_heights(600.0, &[row(false, 1.0), row(true, 1.0), row(false, 1.0)]);
    assert!(
        near(folded[0], 200.0) && near(folded[2], 200.0),
        "{folded:?}"
    );
    assert!(near(folded[1], HEADER_HEIGHT), "{folded:?}");
    let (open_tops, folded_tops) = (tops(&open), tops(&folded));
    assert!(near(folded_tops[1], open_tops[1]), "{open:?} -> {folded:?}");
    assert!(
        near(folded_tops[2], open_tops[2] - (200.0 - HEADER_HEIGHT)),
        "{open:?} -> {folded:?}"
    );
    assert!(near(folded.iter().sum::<f32>(), 400.0 + HEADER_HEIGHT));
}

#[test]
fn every_expanded_panel_keeps_the_height_it_has_with_nothing_collapsed() {
    for weights in [[1.0, 2.0, 3.0], [1.0, 100.0, 1.0], [5.0, 1.0, 0.0]] {
        for available in [300.0, 900.0] {
            let all_open: Vec<StackRow> = weights.iter().map(|&w| row(false, w)).collect();
            let open = stack_heights(available, &all_open);
            for mask in 0..8u32 {
                let rows: Vec<StackRow> = weights
                    .iter()
                    .enumerate()
                    .map(|(index, &w)| row(mask & (1 << index) != 0, w))
                    .collect();
                let h = stack_heights(available, &rows);
                for index in 0..3 {
                    let expected = if rows[index].collapsed {
                        HEADER_HEIGHT
                    } else {
                        open[index]
                    };
                    assert!(
                        near(h[index], expected),
                        "weights {weights:?}, {available} pt, mask {mask:03b}: {h:?} vs {open:?}"
                    );
                }
                assert!(
                    h.iter().sum::<f32>() <= available + 0.01,
                    "weights {weights:?}, {available} pt, mask {mask:03b}: {h:?}"
                );
            }
        }
    }
}

#[test]
fn a_dragged_height_survives_collapsing_either_panel() {
    let mut rows = [row(false, 1.0), row(false, 1.0)];
    let heights = stack_heights(600.0, &rows);
    drag_splitter(&mut rows, &heights, 0, 100.0);
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], 400.0) && near(h[1], 200.0), "{h:?}");

    rows[0].collapsed = true;
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], HEADER_HEIGHT) && near(h[1], 200.0), "{h:?}");
    rows[0].collapsed = false;
    rows[1].collapsed = true;
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], 400.0) && near(h[1], HEADER_HEIGHT), "{h:?}");
    rows[1].collapsed = false;
    let h = stack_heights(600.0, &rows);
    assert!(near(h[0], 400.0) && near(h[1], 200.0), "{h:?}");
}

#[test]
fn expanding_a_panel_gives_back_the_height_it_had() {
    let mut rows = [row(false, 1.0), row(false, 1.0), row(true, 1.0)];
    let heights = stack_heights(900.0, &rows);
    assert!(
        near(heights[0], 300.0) && near(heights[1], 300.0),
        "{heights:?}"
    );
    drag_splitter(&mut rows, &heights, 0, 60.0);
    let h = stack_heights(900.0, &rows);
    assert!(near(h[0], 360.0) && near(h[1], 240.0), "{h:?}");
    assert!(near(h[2], HEADER_HEIGHT), "{h:?}");

    rows[2].collapsed = false;
    let h = stack_heights(900.0, &rows);
    assert!(
        near(h[0], 360.0) && near(h[1], 240.0) && near(h[2], 300.0),
        "{h:?}"
    );
}

#[test]
fn dragging_a_border_changes_only_its_two_panels_weights() {
    let mut rows = [
        row(false, 2.0),
        row(true, 5.0),
        row(false, 1.0),
        row(false, 1.0),
    ];
    let heights = stack_heights(1800.0, &rows);
    let before = rows;
    drag_splitter(&mut rows, &heights, 2, 30.0);
    assert_eq!(rows[0], before[0]);
    assert_eq!(rows[1], before[1]);
    assert!(
        near(
            rows[2].weight + rows[3].weight,
            before[2].weight + before[3].weight
        ),
        "{rows:?}"
    );
    let h = stack_heights(1800.0, &rows);
    assert!(near(h[0], heights[0]), "{heights:?} -> {h:?}");
    assert!(near(h[2], heights[2] + 30.0), "{heights:?} -> {h:?}");
    assert!(near(h[3], heights[3] - 30.0), "{heights:?} -> {h:?}");
}

#[test]
fn a_short_column_still_gives_each_expanded_panel_the_minimum() {
    let h = stack_heights(100.0, &[row(false, 1.0), row(true, 1.0), row(false, 1.0)]);
    assert!(near(h[0], MIN) && near(h[2], MIN), "{h:?}");
    assert!(near(h[1], HEADER_HEIGHT), "{h:?}");
}
