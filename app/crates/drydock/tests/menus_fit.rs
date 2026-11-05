// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-menus-2: menus as wide as their content, an icon column on every row, check marks for
//! View's show/hide items, View regrouped, and "..." on items that open a window.
//! See docs/specs/m6-menus-2.md.

use drydock::chrome::{
    MenuState, menu_bar, menu_bar_with_toolbars, menu_check_item, menu_item, menu_item_layout,
};
use drydock::icons::Icon;
use drydock::input::{Action, Tool};
use drydock::look;
use drydock::toolbars::ToolbarLayout;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{self, Color32, Pos2, Rect, Shape, TextStyle, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

/// The app's fonts (the test build has no egui default fonts, so text would measure 0).
fn fonts<S>(h: &Harness<'_, S>) {
    h.ctx.set_fonts(look::font_definitions());
}

fn menus(state: MenuState<'static>, seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui(move |ui| {
            if let Some(action) = menu_bar(ui, &state) {
                seen.borrow_mut().push(action);
            }
        })
}

fn menus_with_toolbars(state: MenuState<'static>) -> Harness<'static> {
    let layout = Rc::new(RefCell::new(ToolbarLayout::default()));
    Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui(move |ui| {
            menu_bar_with_toolbars(ui, &state, &mut layout.borrow_mut());
        })
}

fn open(h: &mut Harness<'_>, menu: &str) {
    fonts(h);
    h.run();
    h.get_by_label(menu).click();
    h.run();
    h.run();
}

/// A menu row (button, check box or submenu row) whose label is exactly `label`.
fn row(h: &Harness<'_>, label: &str) -> Rect {
    let rows: Vec<Rect> = h
        .query_all_by_role(Role::Button)
        .chain(h.query_all_by_role(Role::CheckBox))
        .filter(|n| n.accesskit_node().label().as_deref() == Some(label))
        .map(|n| n.rect())
        .collect();
    assert_eq!(rows.len(), 1, "one row labelled {label:?}: {rows:?}");
    rows[0]
}

fn has_row(h: &Harness<'_>, label: &str) -> bool {
    h.query_all_by_role(Role::Button)
        .chain(h.query_all_by_role(Role::CheckBox))
        .any(|n| n.accesskit_node().label().as_deref() == Some(label))
}

/// Every row of the open menu (below the menu bar), with its label.
fn menu_rows(h: &Harness<'_>, bar_bottom: f32) -> Vec<(String, Rect)> {
    let mut rows: Vec<(String, Rect)> = h
        .query_all_by_role(Role::Button)
        .chain(h.query_all_by_role(Role::CheckBox))
        .filter(|n| n.rect().top() >= bar_bottom - 0.5)
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect()))
        .collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    rows
}

/// Widths of `texts` in the body font, measured inside a frame (fonts load there).
fn text_widths(texts: &[String]) -> Vec<f32> {
    let out = Rc::new(RefCell::new(Vec::new()));
    let o = out.clone();
    let texts = texts.to_vec();
    let mut h = Harness::new_ui(move |ui| {
        let font = TextStyle::Body.resolve(ui.style());
        *o.borrow_mut() = texts
            .iter()
            .map(|t| {
                ui.painter()
                    .layout_no_wrap(t.clone(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            })
            .collect();
    });
    fonts(&h);
    h.run_steps(2);
    out.take()
}

/// The labels (non-button text nodes) lying inside `rect`: a row's shortcut.
fn shortcut_in(h: &Harness<'_>, rect: Rect, label: &str) -> Option<String> {
    h.query_all_by_role(Role::Label)
        .filter(|n| rect.contains_rect(n.rect().shrink(0.5)))
        .filter_map(|n| {
            let node = n.accesskit_node();
            node.value().or_else(|| node.label())
        })
        .find(|text| text != label)
}

// ---- width ----

#[test]
fn each_menu_is_as_wide_as_its_widest_row_and_no_wider() {
    for menu in ["File", "Edit", "View", "Tools", "Help"] {
        let seen = Seen::default();
        let mut h = menus(MenuState::default(), &seen);
        h.run();
        let bar_bottom = h.get_by_label(menu).rect().bottom();
        open(&mut h, menu);
        let rows = menu_rows(&h, bar_bottom);
        assert!(!rows.is_empty(), "{menu}: rows {rows:?}");
        let mut widest_needed: f32 = 0.0;
        for (label, rect) in &rows {
            // icon column 4 + 16 + 6, label, gap 24, shortcut, right margin 6
            let shortcut = shortcut_in(&h, *rect, label);
            let mut texts = vec![label.clone()];
            texts.extend(shortcut.clone());
            let widths = text_widths(&texts);
            let mut needed = 26.0 + widths[0] + 6.0;
            if shortcut.is_some() {
                needed += 24.0 + widths[1];
            }
            widest_needed = widest_needed.max(needed);
        }
        let width = rows[0].1.width();
        for (label, rect) in &rows {
            assert!(
                (rect.width() - width).abs() < 0.5,
                "{menu}: every row spans the menu; {label:?} is {} not {width}",
                rect.width()
            );
            assert!(
                (rect.left() - rows[0].1.left()).abs() < 0.5,
                "{menu}: rows share a left edge"
            );
        }
        assert!(
            width >= widest_needed - 0.5,
            "{menu}: {width} cuts off a row needing {widest_needed}"
        );
        assert!(
            width <= widest_needed + 16.0,
            "{menu}: {width} is much wider than the widest row needs ({widest_needed})"
        );
    }
}

#[test]
fn a_long_undo_label_widens_the_edit_menu_and_a_short_one_narrows_it() {
    let seen = Seen::default();
    let mut h = menus(
        MenuState {
            undo: Some("Move a Very Long Selection Somewhere Else"),
            ..MenuState::default()
        },
        &seen,
    );
    open(&mut h, "Edit");
    let long = row(&h, "Undo Move a Very Long Selection Somewhere Else").width();
    let mut h = menus(MenuState::default(), &seen);
    open(&mut h, "Edit");
    let short = row(&h, "Undo").width();
    assert!(long > short + 100.0, "long {long}, short {short}");
}

// ---- icon column ----

#[test]
fn every_label_starts_after_the_icon_column() {
    let r = Rect::from_min_size(Pos2::new(10.0, 40.0), Vec2::new(200.0, 22.0));
    let with = menu_item_layout(r, true, 0.0);
    let without = menu_item_layout(r, false, 0.0);
    assert!(without.icon.is_none());
    assert!((without.label_left - (r.left() + 26.0)).abs() < 0.01);
    assert!((with.label_left - without.label_left).abs() < 0.01);
}

// ---- check marks ----

/// Shapes of the last frame, flattened, without text.
fn painted(h: &Harness<'_>) -> Vec<Shape> {
    fn flatten(shape: &Shape, out: &mut Vec<Shape>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
            Shape::Text(_) | Shape::Noop => {}
            other => out.push(other.clone()),
        }
    }
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut out);
    }
    out
}

/// Filled rectangles covering most of `row` (the harness's own background is one; a row
/// highlight would add another).
fn highlights(shapes: &[Shape], row: Rect) -> usize {
    shapes
        .iter()
        .filter(|shape| match shape {
            Shape::Rect(r) => {
                r.fill != Color32::TRANSPARENT
                    && r.rect.width() >= row.width() * 0.8
                    && r.rect.height() >= row.height() * 0.8
                    && r.rect.intersects(row)
            }
            _ => false,
        })
        .count()
}

fn ink_in(shapes: &[Shape], area: Rect) -> bool {
    shapes
        .iter()
        .any(|shape| area.contains_rect(shape.visual_bounding_rect().shrink(0.01)))
}

fn check_harness(checked: bool) -> Harness<'static> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(300.0, 100.0))
        .build_ui(move |ui| {
            ui.set_width(200.0);
            menu_check_item(ui, "Shown", checked);
        });
    h.run();
    h
}

#[test]
fn a_check_item_is_a_check_box_with_its_state() {
    for checked in [true, false] {
        let h = check_harness(checked);
        let node = h.get_by_label("Shown");
        assert_eq!(node.accesskit_node().role(), Role::CheckBox);
        let want = if checked {
            Toggled::True
        } else {
            Toggled::False
        };
        assert_eq!(node.accesskit_node().toggled(), Some(want));
    }
}

#[test]
fn a_checked_item_shows_a_mark_in_the_icon_column_and_no_row_highlight() {
    let h = check_harness(true);
    let r = h.get_by_label("Shown").rect();
    let column = menu_item_layout(r, true, 0.0).icon.expect("icon column");
    let shapes = painted(&h);
    let baseline = highlights(&painted(&check_harness(false)), r);
    assert_eq!(
        highlights(&shapes, r),
        baseline,
        "checked rows are not highlighted: {shapes:?}"
    );
    assert!(
        ink_in(&shapes, column.expand(1.0)),
        "a check mark in the icon column"
    );
    let h = check_harness(false);
    let shapes = painted(&h);
    assert!(
        !ink_in(&shapes, column.expand(1.0)),
        "no mark when unchecked"
    );
}

#[test]
fn a_check_item_clicks() {
    let clicked = Rc::new(RefCell::new(0));
    let c = clicked.clone();
    let mut h = Harness::new_ui(move |ui| {
        if menu_check_item(ui, "Shown", false).clicked() {
            *c.borrow_mut() += 1;
        }
    });
    h.run();
    h.get_by_label("Shown").click();
    h.run();
    assert_eq!(*clicked.borrow(), 1);
}

#[test]
fn the_current_tool_frames_its_icon_instead_of_highlighting_the_row() {
    let tool_row = |selected: bool| {
        let mut h = Harness::builder()
            .with_size(egui::vec2(300.0, 100.0))
            .build_ui(move |ui| {
                ui.set_width(200.0);
                menu_item(ui, Some(Icon::Tool(Tool::Line)), "Line", "L", selected);
            });
        h.run();
        h
    };
    let h = tool_row(true);
    let r = row(&h, "Line");
    let icon = menu_item_layout(r, true, 0.0).icon.expect("icon");
    let shapes = painted(&h);
    let baseline = highlights(&painted(&tool_row(false)), r);
    assert_eq!(
        highlights(&shapes, r),
        baseline,
        "no row highlight: {shapes:?}"
    );
    let framed = shapes.iter().any(|shape| match shape {
        Shape::Rect(f) => {
            f.rect.contains_rect(icon)
                && f.rect.width() <= icon.width() + 8.0
                && (f.fill != Color32::TRANSPARENT || f.stroke.width > 0.0)
        }
        _ => false,
    });
    assert!(framed, "a box around the icon: {shapes:?}");
}

#[test]
fn view_show_hide_items_are_check_items() {
    let seen = Seen::default();
    let mut h = menus(
        MenuState {
            axes: true,
            materials: true,
            ..MenuState::default()
        },
        &seen,
    );
    open(&mut h, "View");
    for (label, on) in [
        ("Axes", true),
        ("Entity Info", false),
        ("Materials", true),
        ("Components", false),
        ("Shadows", false),
        ("Texture Position", false),
    ] {
        let node = h
            .query_all_by_role(Role::CheckBox)
            .find(|n| n.accesskit_node().label().as_deref() == Some(label))
            .unwrap_or_else(|| panic!("{label} is a check item"));
        let want = if on { Toggled::True } else { Toggled::False };
        assert_eq!(node.accesskit_node().toggled(), Some(want), "{label}");
    }
    h.query_all_by_role(Role::CheckBox)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Components"))
        .expect("Components")
        .click();
    h.run();
    assert_eq!(
        *seen.borrow(),
        vec![Action::TogglePanel(drydock::dock::PanelId::Components)]
    );
}

#[test]
fn toolbars_submenu_uses_check_items_too() {
    let mut h = menus_with_toolbars(MenuState::default());
    open(&mut h, "View");
    h.get_by_label("Toolbars").click();
    h.run();
    let names: Vec<String> = ToolbarLayout::default()
        .toolbars()
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert!(!names.is_empty());
    for name in names {
        let node = h
            .query_all_by_role(Role::CheckBox)
            .find(|n| n.accesskit_node().label().as_deref() == Some(name.as_str()))
            .unwrap_or_else(|| panic!("toolbar {name} is a check item"));
        assert_eq!(node.accesskit_node().toggled(), Some(Toggled::True));
    }
    assert!(has_row(&h, "Customize..."));
    assert!(has_row(&h, "Reset Toolbars..."));
}

// ---- View grouping ----

#[test]
fn view_groups_axes_panels_and_layout_items_apart() {
    let mut h = menus_with_toolbars(MenuState::default());
    open(&mut h, "View");
    let order = [
        "Axes",
        "Entity Info",
        "Materials",
        "Components",
        "Shadows",
        "Texture Position",
        "Toolbars",
        "Reset Panels...",
        "Model Info...",
    ];
    let rects: Vec<Rect> = order.iter().map(|l| row(&h, l)).collect();
    for (pair, labels) in rects.windows(2).zip(order.windows(2)) {
        assert!(pair[0].top() < pair[1].top(), "{labels:?} out of order");
    }
    let gap = |a: usize| rects[a + 1].top() - rects[a].bottom();
    let normal = gap(1); // Entity Info -> Materials
    for inside in [1, 2, 3, 4, 6] {
        assert!(
            (gap(inside) - normal).abs() < 0.5,
            "{} -> {} should be in one group",
            order[inside],
            order[inside + 1]
        );
    }
    for split in [0, 5, 7] {
        assert!(
            gap(split) >= normal + 3.0,
            "a separator between {} and {} (gap {}, normal {normal})",
            order[split],
            order[split + 1],
            gap(split)
        );
    }
}

// ---- ellipses ----

#[test]
fn items_that_open_a_window_end_with_an_ellipsis() {
    let seen = Seen::default();
    for (menu, label) in [
        ("File", "Open..."),
        ("File", "Save As..."),
        ("View", "Model Info..."),
        ("Help", "About Drydock..."),
    ] {
        let mut h = menus(MenuState::default(), &seen);
        open(&mut h, menu);
        assert!(has_row(&h, label), "{menu} > {label}");
    }
    let mut h = menus(
        MenuState {
            has_selection: true,
            ..MenuState::default()
        },
        &seen,
    );
    open(&mut h, "Edit");
    assert!(has_row(&h, "Make Component..."));
    assert!(has_row(&h, "Make Group..."));
    for (menu, bare) in [("View", "Model Info"), ("Help", "About Drydock")] {
        let mut h = menus(MenuState::default(), &seen);
        open(&mut h, menu);
        assert!(!has_row(&h, bare), "{menu} > {bare} has no ellipsis");
    }
}

#[test]
fn renamed_items_still_act() {
    for (menu, label, action) in [
        ("View", "Model Info...", Action::ModelInfo),
        ("Help", "About Drydock...", Action::About),
    ] {
        let seen = Seen::default();
        let mut h = menus(MenuState::default(), &seen);
        open(&mut h, menu);
        h.query_all_by_role(Role::Button)
            .find(|n| n.accesskit_node().label().as_deref() == Some(label))
            .expect(label)
            .click();
        h.run();
        assert_eq!(*seen.borrow(), vec![action], "{label}");
    }
}
