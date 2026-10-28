// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-customize-2.md`: the two lists and dragging
//! between them.

use eframe::egui::{self, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::input::Tool;
use modeler::toolbars::{
    CustomizeDrop, CustomizeState, ToolbarLayout, apply_customize_drop, available_tools,
    customize_panel,
};

// Toolbar 1 ("Drawing") holds Line, Rectangle, Circle, Arc, 3-Point Arc.
const DRAWING: usize = 1;

fn tools(layout: &ToolbarLayout) -> Vec<Tool> {
    layout.toolbars()[DRAWING].tools.clone()
}

#[test]
fn available_is_everything_else_in_button_order() {
    let layout = ToolbarLayout::default();
    let free = available_tools(&layout, DRAWING);
    assert_eq!(free.len(), 16 - 5);
    assert_eq!(free[0], Tool::Select);
    assert_eq!(free[1], Tool::PushPull);
    assert!(!free.contains(&Tool::Line));
    assert_eq!(available_tools(&layout, 99).len(), 16);
}

#[test]
fn drop_a_new_tool_inserts_it() {
    let mut layout = ToolbarLayout::default();
    assert!(apply_customize_drop(
        &mut layout,
        DRAWING,
        Tool::Move,
        CustomizeDrop::OnToolbar(2)
    ));
    assert_eq!(
        tools(&layout),
        vec![
            Tool::Line,
            Tool::Rectangle,
            Tool::Move,
            Tool::Circle,
            Tool::Arc,
            Tool::Arc3
        ]
    );
    assert!(apply_customize_drop(
        &mut layout,
        DRAWING,
        Tool::Zoom,
        CustomizeDrop::OnToolbar(99)
    ));
    assert_eq!(tools(&layout).last(), Some(&Tool::Zoom));
}

#[test]
fn drop_within_the_list_reorders() {
    let mut layout = ToolbarLayout::default();
    assert!(apply_customize_drop(
        &mut layout,
        DRAWING,
        Tool::Line,
        CustomizeDrop::OnToolbar(3)
    ));
    assert_eq!(
        tools(&layout),
        vec![
            Tool::Rectangle,
            Tool::Circle,
            Tool::Line,
            Tool::Arc,
            Tool::Arc3
        ]
    );
    assert!(apply_customize_drop(
        &mut layout,
        DRAWING,
        Tool::Arc3,
        CustomizeDrop::OnToolbar(0)
    ));
    assert_eq!(tools(&layout)[0], Tool::Arc3);
}

#[test]
fn drop_in_place_changes_nothing() {
    let mut layout = ToolbarLayout::default();
    for i in [1, 2] {
        // Rectangle is at 1: before itself (1) or just after itself (2).
        assert!(!apply_customize_drop(
            &mut layout,
            DRAWING,
            Tool::Rectangle,
            CustomizeDrop::OnToolbar(i)
        ));
    }
    assert_eq!(layout, ToolbarLayout::default());
}

#[test]
fn drop_on_available_removes_only_from_this_toolbar() {
    let mut layout = ToolbarLayout::default();
    assert!(apply_customize_drop(
        &mut layout,
        DRAWING,
        Tool::Circle,
        CustomizeDrop::Available
    ));
    assert_eq!(
        tools(&layout),
        vec![Tool::Line, Tool::Rectangle, Tool::Arc, Tool::Arc3]
    );
    assert!(!apply_customize_drop(
        &mut layout,
        DRAWING,
        Tool::Circle,
        CustomizeDrop::Available
    ));
    let mut layout = ToolbarLayout::default();
    assert!(apply_customize_drop(
        &mut layout,
        0,
        Tool::Select,
        CustomizeDrop::Available
    ));
    assert!(layout.toolbars()[0].tools.is_empty());
    assert_eq!(layout.toolbars()[1], ToolbarLayout::default().toolbars()[1]);
}

#[test]
fn out_of_range_toolbar_does_nothing() {
    let mut layout = ToolbarLayout::default();
    assert!(!apply_customize_drop(
        &mut layout,
        99,
        Tool::Line,
        CustomizeDrop::OnToolbar(0)
    ));
    assert_eq!(layout, ToolbarLayout::default());
}

struct State {
    layout: ToolbarLayout,
    custom: CustomizeState,
    changed: Vec<bool>,
}

fn panel() -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let changed = customize_panel(ui, &mut state.layout, &mut state.custom);
                state.changed.push(changed);
            },
            State {
                layout: ToolbarLayout::default(),
                custom: CustomizeState {
                    selected: DRAWING,
                    new_name: String::new(),
                    ..CustomizeState::default()
                },
                changed: Vec::new(),
            },
        );
    h.run();
    h
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
}

fn centre(h: &Harness<'_, State>, label: &str) -> Pos2 {
    use egui_kittest::kittest::NodeT;
    h.get_by_label(label)
        .accesskit_node()
        .raw_bounds()
        .map_or_else(
            || panic!("no bounds for {label}"),
            |b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32),
        )
}

#[test]
fn window_has_two_headed_lists_with_drag_handles() {
    let h = panel();
    h.get_by_label("On the toolbar");
    h.get_by_label("Available");
    h.get_by_label("Drag Line");
    h.get_by_label("Drag Move");
    // Existing controls stay.
    h.get_by_label("Remove Line");
    h.get_by_label("Add Move");
}

#[test]
fn dragging_an_available_tool_into_the_list_inserts_it() {
    let mut h = panel();
    let from = centre(&h, "Drag Move");
    let line = centre(&h, "Drag Line");
    let rect = centre(&h, "Drag Rectangle");
    // Just below Line's centre: after Line, before Rectangle.
    drag(&mut h, from, pos2(rect.x, line.y + 2.0));
    assert_eq!(
        tools(&h.state().layout)[..3],
        [Tool::Line, Tool::Move, Tool::Rectangle]
    );
    assert!(h.state().changed.iter().any(|&c| c));
    h.get_by_label("Remove Move");
    assert!(h.query_by_label("Add Move").is_none());
}

#[test]
fn dragging_a_row_down_reorders() {
    let mut h = panel();
    let from = centre(&h, "Drag Line");
    let circle = centre(&h, "Drag Circle");
    drag(&mut h, from, pos2(circle.x, circle.y + 2.0));
    assert_eq!(
        tools(&h.state().layout),
        vec![
            Tool::Rectangle,
            Tool::Circle,
            Tool::Line,
            Tool::Arc,
            Tool::Arc3
        ]
    );
}

#[test]
fn dragging_a_row_to_available_removes_it() {
    let mut h = panel();
    let from = centre(&h, "Drag Circle");
    let to = centre(&h, "Available");
    drag(&mut h, from, to);
    assert_eq!(
        tools(&h.state().layout),
        vec![Tool::Line, Tool::Rectangle, Tool::Arc, Tool::Arc3]
    );
    h.get_by_label("Add Circle");
}

#[test]
fn dropping_elsewhere_does_nothing() {
    let mut h = panel();
    let from = centre(&h, "Drag Line");
    drag(&mut h, from, pos2(2.0, 2.0));
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert!(!h.state().changed.iter().any(|&c| c));
}
