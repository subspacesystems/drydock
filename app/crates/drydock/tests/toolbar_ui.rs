// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-ui.md`: the toolbar strip widget, its
//! tooltips and the drop-target rule, driven with egui_kittest.

use drydock::chrome::TOOL_BUTTONS;
use drydock::icons::BUTTON_SIZE;
use drydock::input::Tool;
use drydock::toolbars::{
    EDGE_ZONE, Edge, GRIP_LENGTH, Placement, Toolbar, ToolbarEvent, drop_target, tool_tooltip,
    toolbar_strip,
};
use eframe::egui::{self, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn label(tool: Tool) -> &'static str {
    TOOL_BUTTONS.iter().find(|(t, _)| *t == tool).unwrap().1
}

struct State {
    toolbar: Toolbar,
    current: Tool,
    vertical: bool,
    events: Vec<ToolbarEvent>,
}

fn drawing() -> Toolbar {
    Toolbar {
        name: "Drawing".to_string(),
        tools: vec![Tool::Line, Tool::Rectangle, Tool::Circle],
        visible: true,
        placement: Placement::Docked {
            edge: Edge::Top,
            slot: 1,
        },
    }
}

fn harness(toolbar: Toolbar, current: Tool, vertical: bool) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 400.0))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(event) =
                    toolbar_strip(ui, &state.toolbar, 7, state.current, state.vertical)
                {
                    state.events.push(event);
                }
            },
            State {
                toolbar,
                current,
                vertical,
                events: Vec::new(),
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

const GRIP: &str = "Move Drawing toolbar";

#[test]
fn tooltips_name_the_tool_and_its_key() {
    assert_eq!(tool_tooltip(Tool::Line), "Line (L)");
    assert_eq!(tool_tooltip(Tool::Arc3), "3-Point Arc (Shift+A)");
    assert_eq!(tool_tooltip(Tool::Select), "Select (Space)");
    assert_eq!(tool_tooltip(Tool::Paint), "Paint Bucket (B)");
}

#[test]
fn drop_target_picks_the_nearest_edge_in_range() {
    let window = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
    let z = EDGE_ZONE;
    let cases = [
        (pos2(400.0, 300.0), None),
        (pos2(400.0, 10.0), Some(Edge::Top)),
        (pos2(400.0, 590.0), Some(Edge::Bottom)),
        (pos2(10.0, 300.0), Some(Edge::Left)),
        (pos2(790.0, 300.0), Some(Edge::Right)),
        // On the edge itself, and past it.
        (pos2(400.0, 0.0), Some(Edge::Top)),
        (pos2(400.0, -50.0), Some(Edge::Top)),
        (pos2(900.0, 300.0), Some(Edge::Right)),
        // Exactly the zone counts; a hair beyond does not.
        (pos2(400.0, z), Some(Edge::Top)),
        (pos2(400.0, z + 0.5), None),
        (pos2(800.0 - z, 300.0), Some(Edge::Right)),
        (pos2(800.0 - z - 0.5, 300.0), None),
        // Near a corner: the closer edge wins.
        (pos2(5.0, 20.0), Some(Edge::Left)),
        (pos2(20.0, 5.0), Some(Edge::Top)),
        // A tie goes to the first of top, bottom, left, right.
        (pos2(10.0, 10.0), Some(Edge::Top)),
        (pos2(790.0, 590.0), Some(Edge::Bottom)),
        (pos2(10.0, 590.0), Some(Edge::Bottom)),
        (pos2(790.0, 10.0), Some(Edge::Top)),
    ];
    for (pos, want) in cases {
        assert_eq!(drop_target(pos, window), want, "{pos:?}");
    }
    // The window need not start at the origin.
    let offset = Rect::from_min_max(pos2(100.0, 50.0), pos2(500.0, 350.0));
    assert_eq!(drop_target(pos2(110.0, 200.0), offset), Some(Edge::Left));
    assert_eq!(drop_target(pos2(60.0, 200.0), offset), Some(Edge::Left));
    assert_eq!(drop_target(pos2(300.0, 200.0), offset), None);
}

#[test]
fn a_horizontal_strip_is_a_grip_then_the_tools_left_to_right() {
    let h = harness(drawing(), Tool::Select, false);
    let grip = h.get_by_label(GRIP).rect();
    assert!((grip.width() - GRIP_LENGTH).abs() < 0.5, "{grip:?}");
    assert!((grip.height() - BUTTON_SIZE).abs() < 0.5, "{grip:?}");
    let mut previous = grip;
    for tool in [Tool::Line, Tool::Rectangle, Tool::Circle] {
        let rect = h.get_by_label(label(tool)).rect();
        assert!(
            (rect.width() - BUTTON_SIZE).abs() < 0.5 && (rect.height() - BUTTON_SIZE).abs() < 0.5,
            "{tool:?}: {rect:?}"
        );
        assert!((rect.center().y - grip.center().y).abs() < 0.5, "one row");
        assert!(rect.left() >= previous.right(), "in order: {tool:?}");
        assert!(rect.left() - previous.right() < 10.0, "close together");
        previous = rect;
    }
    assert!(
        h.query_by_label("Select").is_none(),
        "only this toolbar's tools"
    );
}

#[test]
fn a_vertical_strip_runs_top_to_bottom() {
    let h = harness(drawing(), Tool::Select, true);
    let grip = h.get_by_label(GRIP).rect();
    assert!((grip.height() - GRIP_LENGTH).abs() < 0.5, "{grip:?}");
    assert!((grip.width() - BUTTON_SIZE).abs() < 0.5, "{grip:?}");
    let mut previous = grip;
    for tool in [Tool::Line, Tool::Rectangle, Tool::Circle] {
        let rect = h.get_by_label(label(tool)).rect();
        assert!(
            (rect.center().x - grip.center().x).abs() < 0.5,
            "one column"
        );
        assert!(rect.top() >= previous.bottom(), "in order: {tool:?}");
        assert!(rect.top() - previous.bottom() < 10.0, "close together");
        previous = rect;
    }
}

#[test]
fn the_tools_follow_the_toolbars_order() {
    let mut toolbar = drawing();
    toolbar.tools = vec![Tool::Circle, Tool::Zoom, Tool::Line];
    let h = harness(toolbar, Tool::Select, false);
    let x = |tool| h.get_by_label(label(tool)).rect().left();
    assert!(x(Tool::Circle) < x(Tool::Zoom) && x(Tool::Zoom) < x(Tool::Line));
    assert!(h.query_by_label("Rectangle").is_none());
}

#[test]
fn the_current_tool_is_drawn_selected() {
    use eframe::egui::accesskit::Toggled;
    let h = harness(drawing(), Tool::Rectangle, false);
    for (tool, want) in [
        (Tool::Line, false),
        (Tool::Rectangle, true),
        (Tool::Circle, false),
    ] {
        let node = h.get_by_label(label(tool)).accesskit_node();
        assert_eq!(node.toggled() == Some(Toggled::True), want, "{tool:?}");
    }
}

#[test]
fn hovering_a_tool_shows_its_tooltip() {
    let mut h = harness(drawing(), Tool::Select, false);
    h.get_by_label("Line").hover();
    for _ in 0..90 {
        h.step();
    }
    h.run();
    assert!(h.query_by_label("Line (L)").is_some());
}

#[test]
fn clicking_a_tool_selects_it() {
    let mut h = harness(drawing(), Tool::Select, false);
    h.get_by_label("Circle").click();
    h.run();
    assert_eq!(h.state().events, vec![ToolbarEvent::Select(Tool::Circle)]);
}

#[test]
fn letting_go_of_the_grip_reports_where() {
    let mut h = harness(drawing(), Tool::Select, false);
    let grip = h.get_by_label(GRIP).rect().center();
    let to = pos2(300.0, 250.0);
    drag(&mut h, grip, to);
    assert_eq!(
        h.state().events,
        vec![ToolbarEvent::Dropped { index: 7, pos: to }]
    );
}

#[test]
fn a_click_on_the_grip_is_not_a_drop() {
    let mut h = harness(drawing(), Tool::Select, false);
    h.get_by_label(GRIP).click();
    h.run();
    assert!(h.state().events.is_empty());
}

#[test]
fn dragging_a_tool_button_is_not_a_grip_drop() {
    let mut h = harness(drawing(), Tool::Select, false);
    let from = h.get_by_label("Line").rect().center();
    drag(&mut h, from, pos2(300.0, 250.0));
    assert!(
        !h.state()
            .events
            .iter()
            .any(|event| matches!(event, ToolbarEvent::Dropped { .. }))
    );
}

#[test]
fn an_empty_toolbar_is_just_its_grip() {
    let mut toolbar = drawing();
    toolbar.tools.clear();
    let h = harness(toolbar, Tool::Select, false);
    let grip = h.get_by_label(GRIP).rect();
    assert!((grip.width() - GRIP_LENGTH).abs() < 0.5);
    assert!(h.query_by_label("Line").is_none());
}
