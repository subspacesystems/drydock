// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m11-dialog-print-toolbars.md`: Print Setup and Customize
//! Toolbars are dialog windows (`dialog::dialog_window`). UI tests embed viewports, so the
//! window is drawn inline; what is checked is the behaviour the helper adds (Escape closes)
//! and that the contents still work inside it.

use drydock::input::Tool;
use drydock::print_setup::{
    Bed, PrintSetupEvent, PrintSetupTexts, PrintSetupView, print_setup_window,
};
use drydock::toolbars::{CustomizeState, ToolbarLayout, customize_window};
use eframe::egui::{self, Key, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const DRAWING: usize = 1;

struct Print {
    open: bool,
    texts: PrintSetupTexts,
    events: Vec<PrintSetupEvent>,
}

fn print_harness(open: bool) -> Harness<'static, Print> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 800.0))
        .build_ui_state(
            |ui, s: &mut Print| {
                let view = PrintSetupView {
                    bed: Bed {
                        x_mm: 220.0,
                        y_mm: 220.0,
                        z_mm: Some(250.0),
                    },
                    show_bed: false,
                    overhang_degrees: 45.0,
                    can_drop: false,
                    report: None,
                };
                let event = print_setup_window(ui.ctx(), &mut s.open, &mut s.texts, &view);
                if event != PrintSetupEvent::None {
                    s.events.push(event);
                }
            },
            Print {
                open,
                texts: PrintSetupTexts::default(),
                events: vec![],
            },
        );
    h.run();
    h
}

#[test]
fn print_setup_window_shows_its_contents_while_open() {
    let h = print_harness(true);
    h.get_by_label("Printer bed");
    h.get_by_label("Check fit");
}

#[test]
fn print_setup_window_draws_nothing_while_closed() {
    let h = print_harness(false);
    assert!(h.query_by_label("Printer bed").is_none());
    assert!(h.query_by_label("Check fit").is_none());
}

#[test]
fn print_setup_buttons_still_report_events_in_the_window() {
    let mut h = print_harness(true);
    h.get_by_label("Check fit").click();
    h.run();
    assert_eq!(h.state().events, vec![PrintSetupEvent::CheckFit]);
    assert!(h.state().open);
}

#[test]
fn escape_closes_print_setup() {
    let mut h = print_harness(true);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().open);
    assert!(h.query_by_label("Printer bed").is_none());
}

struct Custom {
    open: bool,
    layout: ToolbarLayout,
    state: CustomizeState,
    changed: bool,
}

fn custom_harness(open: bool) -> Harness<'static, Custom> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 800.0))
        .build_ui_state(
            |ui, s: &mut Custom| {
                s.changed |= customize_window(ui.ctx(), &mut s.open, &mut s.layout, &mut s.state);
            },
            Custom {
                open,
                layout: ToolbarLayout::default(),
                state: CustomizeState {
                    selected: DRAWING,
                    ..CustomizeState::default()
                },
                changed: false,
            },
        );
    h.run();
    h
}

fn centre(h: &Harness<'_, Custom>, label: &str) -> Pos2 {
    use egui_kittest::kittest::NodeT;
    h.get_by_label(label)
        .accesskit_node()
        .raw_bounds()
        .map_or_else(
            || panic!("no bounds for {label}"),
            |b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32),
        )
}

fn drag(h: &mut Harness<'_, Custom>, from: Pos2, to: Pos2) {
    let press = |h: &mut Harness<'_, Custom>, pos, pressed| {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    };
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

#[test]
fn customize_window_shows_its_lists_while_open_and_nothing_when_closed() {
    let h = custom_harness(true);
    h.get_by_label("On the toolbar");
    h.get_by_label("Available");
    let closed = custom_harness(false);
    assert!(closed.query_by_label("On the toolbar").is_none());
}

#[test]
fn dragging_a_tool_into_the_list_works_inside_the_window() {
    let mut h = custom_harness(true);
    let from = centre(&h, "Drag Move");
    let line = centre(&h, "Drag Line");
    let rect = centre(&h, "Drag Rectangle");
    drag(&mut h, from, pos2(rect.x, line.y + 2.0));
    assert_eq!(
        h.state().layout.toolbars()[DRAWING].tools[..3],
        [Tool::Line, Tool::Move, Tool::Rectangle]
    );
    assert!(h.state().changed);
    assert!(h.state().open);
}

#[test]
fn escape_closes_customize_toolbars() {
    let mut h = custom_harness(true);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().open);
    assert!(h.query_by_label("On the toolbar").is_none());
}
