// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-dock-ui.md`: the panel title row and the drop
//! rules, driven with egui_kittest.

use drydock::dock::{
    DEFAULT_WIDTH, DockLayout, DropTarget, HEADER_HEIGHT, PanelEvent, PanelId, Place, Side,
    apply_drop, drop_destination, panel_header,
};
use eframe::egui::{self, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    id: PanelId,
    collapsed: bool,
    docked: bool,
    events: Vec<PanelEvent>,
}

fn harness(id: PanelId, collapsed: bool, docked: bool) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 200.0))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(event) = panel_header(ui, state.id, state.collapsed, state.docked) {
                    state.events.push(event);
                }
            },
            State {
                id,
                collapsed,
                docked,
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

#[test]
fn the_header_has_a_handle_a_collapse_button_and_a_close_button() {
    let h = harness(PanelId::Materials, false, true);
    let handle = h.get_by_label("Move Materials panel").rect();
    let collapse = h.get_by_label("Collapse Materials").rect();
    let close = h.get_by_label("Close Materials").rect();
    assert!(handle.left() < collapse.left() && collapse.left() < close.left());
    assert!(handle.right() <= collapse.left() + 0.5);
    for rect in [handle, collapse, close] {
        assert!(rect.height() <= HEADER_HEIGHT + 0.5, "{rect:?}");
        assert!((rect.center().y - handle.center().y).abs() < 1.0, "one row");
    }
    assert!(handle.width() > 40.0, "the title has room: {handle:?}");
}

#[test]
fn a_collapsed_panel_offers_expand() {
    let h = harness(PanelId::Components, true, true);
    assert!(h.query_by_label("Expand Components").is_some());
    assert!(h.query_by_label("Collapse Components").is_none());
}

#[test]
fn a_floating_panel_has_no_collapse_button() {
    let h = harness(PanelId::Shadows, false, false);
    assert!(h.query_by_label("Collapse Shadows").is_none());
    assert!(h.query_by_label("Expand Shadows").is_none());
    assert!(h.query_by_label("Close Shadows").is_some());
    assert!(h.query_by_label("Move Shadows panel").is_some());
}

#[test]
fn clicking_the_buttons_reports_them() {
    let mut h = harness(PanelId::EntityInfo, false, true);
    h.get_by_label("Collapse Entity Info").click();
    h.run();
    h.get_by_label("Close Entity Info").click();
    h.run();
    assert_eq!(
        h.state().events,
        vec![
            PanelEvent::ToggleCollapsed(PanelId::EntityInfo),
            PanelEvent::Close(PanelId::EntityInfo)
        ]
    );
}

#[test]
fn dragging_the_title_reports_where_it_was_dropped() {
    let mut h = harness(PanelId::Materials, false, true);
    let from = h.get_by_label("Move Materials panel").rect().center();
    let to = pos2(150.0, 120.0);
    drag(&mut h, from, to);
    match h.state().events.as_slice() {
        [PanelEvent::Dropped { id, pos }] => {
            assert_eq!(*id, PanelId::Materials);
            assert!((*pos - to).length() < 1.0, "{pos:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_plain_click_on_the_title_reports_nothing() {
    let mut h = harness(PanelId::Materials, false, true);
    h.get_by_label("Move Materials panel").click();
    h.run();
    assert!(h.state().events.is_empty());
}

fn screen() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0))
}

#[test]
fn drop_destination_docks_near_a_side_and_floats_elsewhere() {
    let layout = DockLayout::default(); // Entity Info and Materials open, right column
    let middles = [100.0, 300.0];
    assert_eq!(
        drop_destination(&layout, screen(), pos2(900.0, 200.0), &middles),
        DropTarget::Dock {
            side: Side::Right,
            position: 1
        }
    );
    assert_eq!(
        drop_destination(&layout, screen(), pos2(900.0, 50.0), &middles),
        DropTarget::Dock {
            side: Side::Right,
            position: 0
        }
    );
    assert_eq!(
        drop_destination(&layout, screen(), pos2(900.0, 650.0), &middles),
        DropTarget::Dock {
            side: Side::Right,
            position: 2
        }
    );
    // The left column is empty, so only its edge strip counts.
    assert_eq!(
        drop_destination(&layout, screen(), pos2(10.0, 400.0), &[]),
        DropTarget::Dock {
            side: Side::Left,
            position: 0
        }
    );
    assert_eq!(
        drop_destination(&layout, screen(), pos2(400.0, 300.0), &middles),
        DropTarget::Float
    );
}

#[test]
fn applying_a_dock_drop_moves_the_panel_into_the_column() {
    let mut layout = DockLayout::default();
    apply_drop(
        &mut layout,
        PanelId::Shadows,
        DropTarget::Dock {
            side: Side::Left,
            position: 0,
        },
        pos2(10.0, 10.0),
    );
    assert_eq!(layout.docked(Side::Left), vec![PanelId::Shadows]);
    assert!(layout.is_open(PanelId::Shadows));
    // Reordering inside a column.
    apply_drop(
        &mut layout,
        PanelId::Materials,
        DropTarget::Dock {
            side: Side::Right,
            position: 0,
        },
        pos2(900.0, 10.0),
    );
    assert_eq!(
        layout.docked(Side::Right),
        vec![PanelId::Materials, PanelId::EntityInfo]
    );
}

#[test]
fn applying_a_float_drop_puts_the_corner_at_the_pointer() {
    let mut layout = DockLayout::default();
    apply_drop(
        &mut layout,
        PanelId::Materials,
        DropTarget::Float,
        pos2(400.0, 250.0),
    );
    assert_eq!(
        layout.panel(PanelId::Materials).place,
        Place::Floating {
            x: 400.0,
            y: 250.0,
            w: DEFAULT_WIDTH,
            h: 300.0
        }
    );
    assert_eq!(layout.floating(), vec![PanelId::Materials]);

    // A floating panel keeps its size when it floats again.
    layout.float(
        PanelId::Materials,
        Rect::from_min_size(pos2(0.0, 0.0), vec2(333.0, 444.0)),
    );
    apply_drop(
        &mut layout,
        PanelId::Materials,
        DropTarget::Float,
        pos2(50.0, 60.0),
    );
    assert_eq!(
        layout.panel(PanelId::Materials).place,
        Place::Floating {
            x: 50.0,
            y: 60.0,
            w: 333.0,
            h: 444.0
        }
    );
}
