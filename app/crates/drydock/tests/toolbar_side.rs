// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-side-width.md`: toolbars docked on the left
//! or right sit in a column exactly as wide as a toolbar, which can't be resized.

use drydock::icons::BUTTON_SIZE;
use drydock::input::Tool;
use drydock::toolbars::{Edge, Placement, SIDE_MARGIN, Toolbar, edge_panel, toolbar_strip};
use eframe::egui::{self, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const WINDOW: f32 = 600.0;

struct State {
    edge: Edge,
    toolbars: Vec<Toolbar>,
    panel: Rect,
}

fn toolbar(name: &str, tools: Vec<Tool>, edge: Edge, slot: u32) -> Toolbar {
    Toolbar {
        name: name.to_string(),
        tools,
        visible: true,
        placement: Placement::Docked { edge, slot },
    }
}

fn harness(edge: Edge, count: usize) -> Harness<'static, State> {
    let mut toolbars = vec![toolbar(
        "Drawing",
        vec![Tool::Line, Tool::Rectangle, Tool::Circle],
        edge,
        0,
    )];
    if count > 1 {
        toolbars.push(toolbar("Modify", vec![Tool::Move, Tool::Rotate], edge, 1));
    }
    let mut h = Harness::builder()
        .with_size(vec2(WINDOW, WINDOW))
        .build_ui_state(
            |ui, state: &mut State| {
                let vertical = matches!(state.edge, Edge::Left | Edge::Right);
                let toolbars = state.toolbars.clone();
                let response = edge_panel(state.edge, &ui.style().clone()).show(ui, |ui| {
                    let strips = |ui: &mut egui::Ui| {
                        for (index, toolbar) in toolbars.iter().enumerate() {
                            toolbar_strip(ui, toolbar, index, Tool::Select, vertical);
                        }
                    };
                    if vertical {
                        ui.vertical(strips);
                    } else {
                        ui.horizontal(strips);
                    }
                });
                state.panel = response.response.rect;
                ui.label("viewport");
            },
            State {
                edge,
                toolbars,
                panel: Rect::NOTHING,
            },
        );
    h.run();
    h.run();
    h
}

fn button_rect(h: &Harness<'_, State>, label: &str) -> Rect {
    h.get_by_label(label).rect()
}

fn press(h: &mut Harness<'_, State>, pos: Pos2, pressed: bool) {
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
}

fn drag(h: &mut Harness<'_, State>, from: Pos2, to: Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.run();
    press(h, from, true);
    h.run();
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        h.event(egui::Event::PointerMoved(from + (to - from) * t));
        h.run();
    }
    press(h, to, false);
    h.run();
    h.run();
}

fn assert_narrow(h: &Harness<'_, State>) {
    let panel = h.state().panel;
    assert!(
        panel.width() >= BUTTON_SIZE && panel.width() <= BUTTON_SIZE + 2.0 * SIDE_MARGIN + 1.0,
        "a side toolbar column should be about {} pt wide, got {panel:?}",
        BUTTON_SIZE + 2.0 * SIDE_MARGIN
    );
}

#[test]
fn a_left_toolbar_column_is_as_wide_as_the_toolbar() {
    let h = harness(Edge::Left, 1);
    assert_narrow(&h);
    let panel = h.state().panel;
    // The test harness draws inside its own 8 pt frame.
    assert!(panel.left() <= 9.0, "docked on the left edge: {panel:?}");
}

#[test]
fn a_right_toolbar_column_is_as_wide_as_the_toolbar() {
    let h = harness(Edge::Right, 1);
    assert_narrow(&h);
    let panel = h.state().panel;
    assert!(
        panel.right() >= WINDOW - 9.0,
        "docked on the right edge: {panel:?}"
    );
}

#[test]
fn the_buttons_sit_inside_the_column_and_centred_in_it() {
    for edge in [Edge::Left, Edge::Right] {
        let h = harness(edge, 1);
        let panel = h.state().panel;
        for label in ["Line", "Rectangle", "Circle"] {
            let rect = button_rect(&h, label);
            assert!(
                panel.expand(0.5).contains_rect(rect),
                "{edge:?}: {label} {rect:?} outside the column {panel:?}"
            );
            assert!(
                (rect.center().x - panel.center().x).abs() <= 1.0,
                "{edge:?}: {label} {rect:?} not centred in {panel:?}"
            );
        }
        let line = button_rect(&h, "Line");
        let circle = button_rect(&h, "Circle");
        assert!(
            circle.top() > line.bottom() - 0.5,
            "buttons run top to bottom"
        );
    }
}

#[test]
fn two_toolbars_on_one_side_share_the_same_narrow_column() {
    for edge in [Edge::Left, Edge::Right] {
        let h = harness(edge, 2);
        assert_narrow(&h);
        let panel = h.state().panel;
        let rotate = button_rect(&h, "Rotate");
        assert!(
            panel.expand(0.5).contains_rect(rotate),
            "{edge:?}: second toolbar {rotate:?} outside {panel:?}"
        );
    }
}

#[test]
fn dragging_the_columns_border_does_not_resize_it() {
    for (edge, outward) in [(Edge::Left, 1.0), (Edge::Right, -1.0)] {
        let mut h = harness(edge, 1);
        let before = h.state().panel;
        let border_x = if outward > 0.0 {
            before.right()
        } else {
            before.left()
        };
        for x in [border_x - 1.0, border_x, border_x + 1.0] {
            let from = pos2(x, WINDOW * 0.75);
            drag(&mut h, from, from + vec2(150.0 * outward, 0.0));
        }
        let after = h.state().panel;
        assert!(
            (after.width() - before.width()).abs() < 0.5,
            "{edge:?}: width changed from {before:?} to {after:?}"
        );
    }
}

#[test]
fn top_and_bottom_toolbars_still_run_along_the_edge() {
    for edge in [Edge::Top, Edge::Bottom] {
        let h = harness(edge, 2);
        let panel = h.state().panel;
        assert!(panel.width() >= WINDOW - 20.0, "{edge:?}: {panel:?}");
        assert!(
            panel.height() >= BUTTON_SIZE && panel.height() <= BUTTON_SIZE + 16.0,
            "{edge:?}: {panel:?}"
        );
        let line = button_rect(&h, "Line");
        let rotate = button_rect(&h, "Rotate");
        assert!(
            rotate.left() > line.right(),
            "{edge:?}: toolbars side by side"
        );
        assert!(panel.expand(0.5).contains_rect(rotate), "{edge:?}");
    }
}
